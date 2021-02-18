use crate::config::RunnerConfiguration;
use crate::migrations::{Direction, Migration, MigrationStep};
use mustache::MapBuilder;
use mysql::prelude::Queryable;
use mysql::{Conn, OptsBuilder};
use std::convert::From;

const MARIADB_MIGRATION_STATE_TABLE_NAME: &str = "mitre_migration_state";

#[derive(Debug)]
pub struct MariaDb {
    conn: Conn,
    config: RunnerConfiguration,
}

#[derive(PartialEq, Debug)]
pub enum MigrationState {
    Pending,
    Applied,
}

#[derive(PartialEq, Debug)]
pub enum MigrationResult {
    AlreadyApplied,
    Success,
    Failure(String),
    NothingToDo,
}

#[derive(Debug)]
pub enum Error {
    MariaDb(mysql::Error),
    ErrorRunningQuery,
    CouldNotSelectDatabase,
    NoStateStoreDatabaseNameProvided,
    // (reason, the template)
    TemplateError(String, mustache::Template),
    ErrorRunningMigration(String),
    MigrationHasFailed(String, Migration),
}

impl From<mysql::Error> for Error {
    fn from(err: mysql::Error) -> Error {
        Error::MariaDb(err)
    }
}

/// Helper methods for MariaDb (non-public) used in the context
/// of fulfilling the implementation of the runner::Runner trait.
impl MariaDb {
    /// We do not record failures,
    fn record_success(
        &mut self,
        m: &Migration,
        _ms: &MigrationStep,
        d: std::time::Duration,
    ) -> Result<(), Error> {
        match self.conn.prep(format!("INSERT INTO {} (`version`, `up`, `down`, `change`, `applied_at_utc`, `apply_time_ms`, `built_in`, `environment`) VALUES (?, ?, ?, ?, ?, ?, ?, ?);", MARIADB_MIGRATION_STATE_TABLE_NAME)) {
          Ok(stmt) => match self.conn.exec_iter(stmt, (
              m.date_time,
              m.steps.get(&Direction::Up).map(|ms| format!("{:?}", ms.source )),
              m.steps.get(&Direction::Down).map(|ms| format!("{:?}", ms.source )),
              m.steps.get(&Direction::Change).map(|ms| format!("{:?}", ms.source )),
              chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
              d.as_millis(),
              m.built_in,
              "NOT IMPLEMENTED"
            )) {
            Ok(query_results) => match query_results.affected_rows() { // TODO: this also contains warnings, could be cool
              1 => Ok(()),
              _ => panic!("error recording success, expected to affect exactly one row")
            },
            Err(e) => panic!("error running query {:?}", e),
          },
          Err(e) => panic!("coult not prepare statement {:?}", e)
        }
    }
}

impl crate::runner::Runner for MariaDb {
    type Error = Error;
    type Migration = Migration;
    type MigrationStep = MigrationStep;

    type MigrationStateTuple = (MigrationState, Migration);
    type MigrationResultTuple = (MigrationResult, Migration);

    fn new(config: &RunnerConfiguration) -> Result<MariaDb, Error> {
        let opts = mysql::Opts::from(
            OptsBuilder::new()
                .ip_or_hostname(config.ip_or_hostname.clone())
                .user(config.username.clone())
                // NOTE: Do not specify database name here, otherwise we cannot
                // connect until the database exists. Makes it difficult to
                // bootstrap.
                // .db_name(config.database.clone())
                .pass(config.password.clone()),
        );
        Ok(MariaDb {
            conn: Conn::new(opts)?,
            config: config.to_owned(),
        })
    }

    // Applies a single migration (each runner needs something like this)
    fn apply(&mut self, ms: &Self::MigrationStep) -> Result<(), Error> {
        let template_ctx = MapBuilder::new()
            .insert_str(
                "mariadb_migration_state_table_name",
                MARIADB_MIGRATION_STATE_TABLE_NAME,
            )
            .insert_str(
                "mariadb_migration_state_database_name",
                self.config.database.as_ref().unwrap(),
            )
            .build();

        println!("Template CTX is {:?}", template_ctx);

        let parsed = match ms.content.render_data_to_string(&template_ctx) {
            Ok(str) => Ok(str),
            Err(e) => Err(Error::TemplateError(e.to_string(), ms.content.clone())),
        }?;

        match self.conn.query::<String, _>(parsed) {
            Ok(res) => {
                println!("Result Is: {:#?}", res);
                Ok(())
            }
            Err(e) => Err(Error::ErrorRunningMigration(e.to_string())),
        }
    }

    fn up(
        &mut self,
        migrations: Vec<Self::Migration>,
    ) -> Result<Vec<Self::MigrationResultTuple>, Error> {
        Ok(self
            .diff(migrations)?
            .into_iter()
            .map(|(_migration_state, migration)| {
                // TODO: check mgiation_state is pending
                // TODO: blow-up (mayne not here?) if we have up+change, only up+down makes sense.migration
                match migration.steps.get(&Direction::Change) {
                    Some(change_step) => {
                        let start = std::time::Instant::now();
                        match self.apply(change_step) {
                            Ok(_) => {
                                match self.record_success(&migration, change_step, start.elapsed())
                                {
                                    Ok(_) => (MigrationResult::Success, migration),
                                    Err(e) => {
                                        (MigrationResult::Failure(format!("{:?}", e)), migration)
                                    }
                                }
                            }
                            Err(e) => (MigrationResult::Failure(format!("{:?}", e)), migration),
                        }
                    }
                    None => (MigrationResult::NothingToDo, migration),
                }
            })
            .collect())
    }

    fn diff(
        &mut self,
        migrations: Vec<Self::Migration>,
    ) -> Result<Vec<Self::MigrationStateTuple>, Error> {
        let database = match &self.config.database {
            Some(database) => Ok(database),
            None => Err(Error::NoStateStoreDatabaseNameProvided),
        }?;

        let schema_exists = self.conn.exec_first::<bool, _, _>(
          "SELECT EXISTS(SELECT SCHEMA_NAME FROM INFORMATION_SCHEMA.SCHEMATA WHERE SCHEMA_NAME = ?)",
          (database,) //trailing comma makes this a tuple
        )?;

        match schema_exists {
            Some(schema_exists) => {
                println!("schema exists?: {}", schema_exists);
                if !schema_exists {
                    return Ok(migrations
                        .into_iter()
                        .map(|m| (MigrationState::Pending, m))
                        .collect());
                } else {
                    match &self.conn.select_db(&database) {
                        true => {}
                        false => return Err(Error::CouldNotSelectDatabase),
                    }
                }
            }
            None => {
                return Err(Error::ErrorRunningQuery);
            }
        }

        // Same story for the table when diffing, we don't want to run any migrations, so
        // we simply say, if the table doesn't exist, then we answer that all migrations (incl. built-in)
        // _must_ be un-run as far as we know.
        match self.conn.exec_first::<bool, _, _>(
          "SELECT EXISTS( SELECT * FROM information_schema.tables WHERE table_schema = ? AND table_name = ? );",
            (database, MARIADB_MIGRATION_STATE_TABLE_NAME), //trailing comma makes this a tuple
          )? {
          Some(table_exists) => {
            if !table_exists {
                Ok(migrations.into_iter().map(|m| (MigrationState::Pending, m)).collect())
            } else {
              // TODO check what migrations did run, and
              // Thinking something like a SELECT * FROM <migration schema table> WHERE timestamp NOT IN <1,2,3,4,5,6>
              // (the migrations we know about) .. theory being we send our list, they send back a list, maybe
              // that list isn't enormous, or we check out cursor based

              // while let Some(result_set) = result.next_set() {
              //   let result_set = result_set?; //TODO: check result_set validity here and skip it if it's an Err
              //   sets += 1;
              //   println!("Result set columns: {:?}", result_set.columns());

              // Let's select payments from database. Type inference should do the trick here.
              match self.conn.query_map::<_,_,_,String>(format!("SELECT version FROM {} ORDER BY version ASC;", MARIADB_MIGRATION_STATE_TABLE_NAME), |version| version) {
                Ok(stored_migration_versions) => {
                   Ok(migrations.into_iter().map(move |m| {
                    let migration_version = format!("{}", m.date_time.format(crate::migrations::FORMAT_STR));
                    match stored_migration_versions.clone().into_iter().find(|stored_m| &migration_version == stored_m ) {
                        Some(_) =>(MigrationState::Applied, m),
                        None => (MigrationState::Pending, m)
                    }
                  }).collect())
                },
                Err(e) => Err(Error::MariaDb(e))
              }
            }
          },
          None => Err(Error::ErrorRunningQuery)
        }
    }
}

#[cfg(test)]
mod tests {

    extern crate rand;
    extern crate tempdir;

    use super::*;
    use crate::migrations::migrations;
    use crate::runner::Runner;
    use rand::Rng;
    use tempdir::TempDir;

    const TEST_DB_IP: &'static str = "127.0.0.1";
    const TEST_DB_PORT: u16 = 3306;
    const TEST_DB_USER: &'static str = "root";
    const TEST_DB_PASSWORD: &'static str = "example";

    struct TestDB {
        conn: mysql::Conn,
        runner_config: RunnerConfiguration,
    }

    impl Drop for TestDB {
        fn drop(&mut self) {
            println!("Dropping DB Conn");
            match helper_delete_test_db(&mut self.conn, &self.runner_config) {
                Ok(_) => println!("success"),
                Err(_) => println!("error, there may be some clean-up to do"),
            };
        }
    }

    fn helper_create_runner_config() -> RunnerConfiguration {
        let random_database_name = format!("mitre_test_{}", rand::thread_rng().gen::<u32>());
        RunnerConfiguration {
            _runner: Some(String::from("mariadb")),
            database_number: None,
            database: Some(String::from(random_database_name)),
            index: None,
            ip_or_hostname: Some(String::from(TEST_DB_IP)),
            password: Some(String::from(TEST_DB_PASSWORD)),
            port: Some(TEST_DB_PORT),
            username: Some(String::from(TEST_DB_USER)),
        }
    }

    fn helper_create_test_db() -> Result<TestDB, String> {
        let runner_config = helper_create_runner_config();
        let mut conn = helper_db_conn()?;

        match &runner_config.database {
            Some(dbname) => {
                let stmt_create_db = conn
                    .prep(format!("CREATE DATABASE {}", dbname))
                    .expect("could not prepare db create statement");
                match conn.exec::<bool, _, _>(stmt_create_db, ()) {
                    Err(e) => Err(format!("error creating test db {:?}", e)),
                    Ok(_) => Ok(TestDB {
                        conn,
                        runner_config,
                    }),
                }
            }
            None => Err(String::from(
                "no dbname provided in config, test set-up error",
            )),
        }
    }

    fn helper_db_conn() -> Result<mysql::Conn, String> {
        let opts = mysql::Opts::from(
            OptsBuilder::new()
                .ip_or_hostname(Some(TEST_DB_IP))
                .user(Some(TEST_DB_USER))
                .pass(Some(TEST_DB_PASSWORD)),
        );
        match Conn::new(opts.clone()) {
            Ok(conn) => Ok(conn),
            Err(e) => Err(format!(
                "cannot connect to test db with {:?}: {:?}",
                opts, e
            )),
        }
    }

    fn helper_delete_test_db(
        conn: &mut mysql::Conn,
        config: &RunnerConfiguration,
    ) -> Result<(), String> {
        match &config.database {
            Some(dbname) => {
                let stmt_create_db = conn
                    .prep(format!("DROP DATABASE {}", dbname))
                    .expect("could not prepare statement");
                match conn.exec::<bool, _, _>(stmt_create_db, ()) {
                    Err(e) => Err(format!("error dropping test db {:?}", e)),
                    Ok(_) => Ok(()),
                }
            }
            None => Err(String::from(
                "no dbname provided in config, test set-up error",
            )),
        }
    }

    #[test]
    fn it_requires_a_config_with_a_table_name() -> Result<(), String> {
        let mut config = helper_create_runner_config();
        config.database = None {};
        let mut runner =
            MariaDb::new(&config).map_err(|e| format!("Could not create runner {:?}", e))?;
        let migrations = vec![];

        let x = match runner.diff(migrations) {
            Ok(_) => Err(String::from("expected an error about missing dbname")),
            Err(e) => match e {
                Error::NoStateStoreDatabaseNameProvided => Ok(()),
                _ => Err(format!("did not expect error {:?}", e)),
            },
        };
        x
    }

    #[test]
    fn it_returns_all_migrations_pending_if_db_does_not_exist() -> Result<(), String> {
        let tmp_dir = TempDir::new("example").expect("gen tmpdir");
        let config = helper_create_runner_config();
        let mut runner =
            MariaDb::new(&config).map_err(|e| format!("Could not create runner {:?}", e))?;
        let migrations =
            migrations(tmp_dir.as_ref()).expect("should make at least default migrations");

        let x = match runner.diff(migrations) {
            Ok(pending_migrations) => {
                match pending_migrations
                    .iter()
                    .all(|pm| pm.0 == MigrationState::Pending)
                {
                    true => Ok(()),
                    false => Err(String::from("expected all migrations to be pending")),
                }
            }
            Err(e) => Err(format!("did not expect error {:?}", e)),
        };
        x
    }

    #[test]
    fn it_returns_all_migrations_pending_if_migrations_table_does_not_exist() -> Result<(), String>
    {
        let tmp_dir = TempDir::new("example").expect("gen tmpdir");
        let test_db = helper_create_test_db()?;

        let mut runner = MariaDb::new(&test_db.runner_config)
            .map_err(|e| format!("Could not create runner {:?}", e))?;
        let migrations =
            migrations(tmp_dir.as_ref()).expect("should make at least default migrations");

        match runner.diff(migrations) {
            Ok(pending_migrations) => {
                match pending_migrations
                    .iter()
                    .all(|pm| pm.0 == MigrationState::Pending)
                {
                    true => Ok(()),
                    false => Err(String::from("expected all migrations to be pending")),
                }
            }
            Err(e) => Err(format!("did not expect error {:?}", e)),
        }
    }

    #[test]
    fn migrating_up_just_the_built_in_migrations() -> Result<(), String> {
        let tmp_dir = TempDir::new("example").expect("gen tmpdir");
        let test_db = helper_create_test_db()?;

        let mut runner = MariaDb::new(&test_db.runner_config)
            .map_err(|e| format!("Could not create runner {:?}", e))?;
        let migs = migrations(tmp_dir.as_ref()).expect("should make at least default migrations");

        let migrations_again =
            migrations(tmp_dir.as_ref()).expect("should make at least default migrations");

        let migrations_thrice =
            migrations(tmp_dir.as_ref()).expect("should make at least default migrations");

        // Arrange: Run up (only built-in, because tmp dir)
        match runner.up(migs) {
            Ok(migration_results) => {
                let v = migration_results;
                assert_eq!(1, v.len());

                let v_success: Vec<&(MigrationResult, Migration)> = v
                    .iter()
                    .filter(|mr| mr.0 == MigrationResult::Success)
                    .collect();
                assert_eq!(1, v_success.len());
            }
            Err(e) => return Err(format!("did not expect error {:?}", e)),
        };

        // Assert that diff thinks all is clear
        match runner.diff(migrations_again) {
            Err(e) => return Err(format!("didn't expect err from diff {:?}", e)),
            Ok(diff_result) => {
                let diff_pending: Vec<(MigrationState, Migration)> = diff_result
                    .into_iter()
                    .filter(|mr| mr.0 == MigrationState::Pending)
                    .collect();
                assert_eq!(0, diff_pending.len());
            }
        };

        // Assert up is a noop
        match runner.up(migrations_thrice) {
            Ok(migration_results) => {
                let v = migration_results;
                assert_eq!(1, v.len());

                let v_success: Vec<&(MigrationResult, Migration)> = v
                    .iter()
                    .filter(|mr| mr.0 == MigrationResult::AlreadyApplied)
                    .collect();
                assert_eq!(1, v_success.len());
            }
            Err(e) => return Err(format!("did not expect error running up again {:?}", e)),
        };

        Ok(())
    }

    #[test]
    fn checks_the_diff_in_run_migrations() {}
}
