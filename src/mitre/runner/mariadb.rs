use crate::mitre::config::RunnerConfiguration;
use crate::mitre::migrations::{Direction, Migration, MigrationStep};
use mustache::MapBuilder;
use mysql::prelude::Queryable;
use mysql::{Conn, OptsBuilder};
use std::convert::From;
use std::io::prelude::*;
use std::io::Write;

const MARIADB_MIGRATION_STATE_TABLE_NAME: &'static str = "mitre_migration_state";

#[derive(Debug)]
pub struct MariaDB {
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
    Success,
    Failure,
}

#[derive(Debug)]
pub enum Error {
    MariaDB(mysql::Error),
    PingFailed(),

    ErrorRunningQuery,
    MigrationStateSchemaDoesNotExist,
    MigrationStateTableDoesNotExist,

    NoStateStoreDatabaseNameProvided,

    // (reason, the template)
    TemplateError(String, mustache::Template),

    ErrorRunningMigration(String),

    AnyError, // placeholder remove me
}

impl From<mysql::Error> for Error {
    fn from(err: mysql::Error) -> Error {
        Error::MariaDB(err)
    }
}

// non-public helper method
fn ensure_connectivity(db: &mut MariaDB) -> Result<(), Error> {
    return match db.conn.ping() {
        true => Ok(()),
        false => Err(Error::PingFailed()),
    };
}

/// Helper methods for MariaDB (non-public) used in the context
/// of fulfilling the implementation of the runner::Runner trait.
impl MariaDB {
    /// We do not record failures,
    fn record_success(
        &mut self,
        m: &Migration,
        ms: &MigrationStep,
        d: std::time::Duration,
    ) -> Result<(), Error> {
        match self.conn.prep(format!("INSERT INTO {} (`version`, `up`, `down`, `change`, `applied_at_utc`, `apply_time_sec`, `built_in`, `environment`) VALUES (?, ?, ?, ?, ?, ?, ?, ?);", MARIADB_MIGRATION_STATE_TABLE_NAME)) {
          Ok(stmt) => match self.conn.exec_first::<String, _, _>(stmt, (
              m.date_time,
              m.steps.get(&Direction::Up).map(|ms| format!("{:?}", ms.content )),
              m.steps.get(&Direction::Down).map(|ms| format!("{:?}", ms.content )),
              m.steps.get(&Direction::Change).map(|ms| format!("{:?}", ms.content )),
              chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
              d.as_secs(),
              m.built_in,
              "NOT IMPLEMENTED"
            )) {
            Ok(reply) => match reply {
              Some(r) => {println!("db says: {}", r); Ok(())},
              None => panic!("affected no rows"), 
            },
            Err(e) => panic!("error recording success {:?}", e),
          },
          Err(e) => panic!("coult not prepare statement {:?}", e)
        }
    }
}

impl crate::mitre::runner::Runner for MariaDB {
    type Error = Error;
    type Migration = Migration;
    type MigrationStep = MigrationStep;

    type MigrationStateTuple = (MigrationState, Migration);
    type MigrationResultTuple = (MigrationResult, Migration);

    fn new(config: &RunnerConfiguration) -> Result<MariaDB, Error> {
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
        return Ok(MariaDB {
            conn: Conn::new(opts)?,
            config: config.to_owned(),
        });
    }

    // https://docs.rs/mysql/20.1.0/mysql/struct.Conn.html
    //
    // TODO: bootstrap can probably go away, diff() handles missing stuff
    // gracefully, and "up" will err right away I guess.
    fn bootstrap(&mut self) -> Result<(), Error> {
        ensure_connectivity(self)
    }

    // Applies a single migration (each runner needs something like this)
    fn apply<'a>(&mut self, ms: &Self::MigrationStep) -> Result<(), Error> {
        let template_ctx = MapBuilder::new()
            .insert_str(
                "mariadb_migration_state_table_name",
                MARIADB_MIGRATION_STATE_TABLE_NAME,
            )
            .insert_str(
                "mariadb_migration_state_databaes_name",
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

    fn up<'a>(
        &mut self,
        migrations: impl Iterator<Item = Self::Migration> + 'a,
    ) -> Result<Box<dyn Iterator<Item = Self::MigrationResultTuple> + 'a>, Error> {
        let mut results = vec![];

        // let mut tx = self.conn.start_transaction(TxOpts::default())?;
        // tx.query_drop("CREATE TEMPORARY TABLE tmp (TEXT a)")?;
        // tx.exec_drop("INSERT INTO tmp (a) VALUES (?)", ("foo",))?;
        // let val: Option<String> = tx.query_first("SELECT a from tmp")?;
        // assert_eq!(val.unwrap(), "foo");
        // // Note, that transaction will be rolled back implicitly on Drop, if not committed.
        // tx.rollback();

        for migration in migrations {
            // TODO: blow-up (mayne not here?) if we have up+change, only up+down makes sense.migration
            match migration.steps.get(&Direction::Change) {
                // TODO: also Up.
                Some(up_step) => {
                    let start = std::time::Instant::now();
                    // have an up migration, let's run it ...
                    match self.apply(up_step) {
                        Ok(res) => {
                            match self.record_success(&migration, up_step, start.elapsed()) {
                                Ok(_) => results.push((MigrationResult::Success, migration)),
                                Err(e) => {
                                    println!("There was an error recording success, but the migration ran, probably {:?}", e);
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            println!("There was an error {:?}", e);
                            break;
                        }
                    };
                }
                None => {} // TODO: warn if a migration has no up-step?
            }
        }
        Ok(Box::new(results.into_iter()))
    }

    fn diff<'a>(
        &mut self,
        migrations: impl Iterator<Item = Self::Migration> + 'a,
    ) -> Result<Box<dyn Iterator<Item = Self::MigrationStateTuple> + 'a>, Error> {
        let database = match &self.config.database {
            Some(database) => Ok(database),
            None => Err(Error::NoStateStoreDatabaseNameProvided),
        }?;

        // Database doesn't exist, then obviously nothing ran... (or we have no permission)
        let schema_exists = self.conn.exec_first::<bool, _, _>(
          "SELECT EXISTS(SELECT SCHEMA_NAME FROM INFORMATION_SCHEMA.SCHEMATA WHERE SCHEMA_NAME = ?)",
          (database,) //trailing comma makes this a tuple
        )?;
        match schema_exists {
            Some(schema_exists) => {
                println!("schema exists?: {}", schema_exists);
                if !schema_exists {
                    // println!("about to create {}", database);
                    // let stmt_create_db = self.conn.prep(format!("CREATE DATABASE {}", database))?;
                    // self.conn.exec::<bool, _, _>(stmt_create_db, ())?;
                    return Ok(Box::new(
                        migrations.map(|m| (MigrationState::Pending, m)).into_iter(),
                    ));
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
            // let iter = migrations.map(|m| (false, m)).collect::<Vec<Self::MigrationStateTuple>>().into_iter();
            // let iter = migrations.map(|m| (false, m)).into_iter();
            return Ok(Box::new(migrations.map(|m| (MigrationState::Pending, m)).into_iter()));
            } else {

              // TODO check what migrations did run, and
              // Thinking something like a SELECT * FROM <migration schema table> WHERE timestamp NOT IN <1,2,3,4,5,6>
              // (the migrations we know about) .. theory being we send our list, they send back a list, maybe
              // that list isn't enormous, or we check out cursor based

              panic!("this road isn't finished yet, turn around!");

            }
          },
          None => return Err(Error::ErrorRunningQuery)
        }
    }
}

#[cfg(test)]
mod tests {

    extern crate rand;
    extern crate tempdir;

    use super::*;
    use crate::mitre::migrations::migrations;
    use crate::mitre::runner::Runner;
    use rand::Rng;
    use tempdir::TempDir;

    const TEST_DB_IP: &'static str = "127.0.0.1";
    const TEST_DB_PORT: u16 = 3306;
    const TEST_DB_USER: &'static str = "root";
    const TEST_DB_PASSWORD: &'static str = "example";

    fn helper_create_runner_config() -> RunnerConfiguration {
        let random_database_name = format!("mitre_test_{}", rand::thread_rng().gen::<u32>());
        RunnerConfiguration {
            _runner: Some(String::from("mariadb")),
            database: Some(String::from(random_database_name)),
            ip_or_hostname: Some(String::from(TEST_DB_IP)),
            port: Some(TEST_DB_PORT),
            username: Some(String::from(TEST_DB_USER)),
            password: Some(String::from(TEST_DB_PASSWORD)),
            database_number: None,
            index: None,
        }
    }

    fn helper_create_test_db() -> Result<(mysql::Conn, RunnerConfiguration), &'static str> {
        let runner_config = helper_create_runner_config();

        let mut conn = helper_db_conn();

        match &runner_config.database {
            Some(dbname) => {
                let stmt_create_db = conn
                    .prep(format!("CREATE DATABASE {}", dbname))
                    .expect("boom");
                match conn.exec::<bool, _, _>(stmt_create_db, ()) {
                    Err(e) => panic!(e),
                    _ => {}
                }
            }
            None => panic!(),
        }

        Ok((conn, runner_config))
    }

    fn helper_db_conn() -> mysql::Conn {
        let opts = mysql::Opts::from(
            OptsBuilder::new()
                .ip_or_hostname(Some(TEST_DB_IP))
                .user(Some(TEST_DB_USER))
                .pass(Some(TEST_DB_PASSWORD)),
        );
        Conn::new(opts.clone())
            .expect(format!("cannot connect to test db with {:?}", opts).as_str())
    }

    fn helper_delete_test_db(
        mut conn: mysql::Conn,
        config: &RunnerConfiguration,
    ) -> Result<(), &'static str> {
        let stmt_create_db = conn
            .prep(format!(
                "DROP DATABASE {}",
                config.database.as_ref().unwrap()
            ))
            .expect("could not prepare statement");
        match conn.exec::<bool, _, _>(stmt_create_db, ()) {
            Err(e) => panic!(e),
            _ => {}
        }
        Ok(())
    }

    #[test]
    fn it_requires_a_config_with_a_table_name() -> Result<(), String> {
        let mut config = helper_create_runner_config();
        config.database = None {};
        let mut runner =
            MariaDB::new(&config).map_err(|e| format!("Could not create runner {:?}", e))?;
        let migrations = std::iter::empty::<Migration>();

        match runner.diff(migrations) {
            Ok(_) => Err(String::from("expected an error about missing dbname")),
            Err(e) => match e {
                Error::NoStateStoreDatabaseNameProvided => Ok(()),
                _ => Err(format!("did not expect error {:?}", e)),
            },
        }
    }

    #[test]
    fn it_returns_all_migrations_pending_if_db_does_not_exist() -> Result<(), String> {
        let tmp_dir = TempDir::new("example").expect("gen tmpdir");
        let config = helper_create_runner_config();
        let mut runner =
            MariaDB::new(&config).map_err(|e| format!("Could not create runner {:?}", e))?;
        let migrations =
            migrations(tmp_dir.as_ref()).expect("should make at least default migrations");

        match runner.diff(migrations) {
            Ok(mut pending_migrations) => {
                match pending_migrations.all(|pm| pm.0 == MigrationState::Pending) {
                    true => Ok(()),
                    false => Err(String::from("expected all migrations to be pending")),
                }
            }
            Err(e) => Err(format!("did not expect error {:?}", e)),
        }
    }

    #[test]
    fn it_returns_all_migrations_pending_if_migrations_table_does_not_exist() -> Result<(), String>
    {
        let tmp_dir = TempDir::new("example").expect("gen tmpdir");
        let mut res: Result<(), String> = Ok(());
        match helper_create_test_db() {
            Ok((conn, config)) => {
                let mut runner = MariaDB::new(&config).map_err(|e| "Could not create runner")?;
                let migrations =
                    migrations(tmp_dir.as_ref()).expect("should make at least default migrations");

                res = match runner.diff(migrations) {
                    Ok(mut pending_migrations) => {
                        match pending_migrations.all(|pm| pm.0 == MigrationState::Pending) {
                            true => Ok(()),
                            false => Err(String::from("expected all migrations to be pending")),
                        }
                    }
                    Err(e) => Err(format!("did not expect error {:?}", e)),
                };
                helper_delete_test_db(conn, &config)?;
            }
            Err(e) => return Err(format!("test set-up err {:?}", e)),
        };
        res
    }

    #[test]
    fn migrating_up_just_the_built_in_migrations() -> Result<(), String> {
        let tmp_dir = TempDir::new("example").expect("gen tmpdir");
        let mut res: Result<(), String> = Ok(());
        match helper_create_test_db() {
            Ok((conn, config)) => {
                let mut runner = MariaDB::new(&config).map_err(|e| "Could not create runner")?;
                let migrations =
                    migrations(tmp_dir.as_ref()).expect("should make at least default migrations");

                res = match runner.up(migrations) {
                    Ok(mut migration_results) => {
                        match migration_results.all(|pm| pm.0 == MigrationResult::Success) {
                            true => Ok(()),
                            false => Err(String::from("expected all migrations to be pending")),
                        }
                    }
                    Err(e) => Err(format!("did not expect error {:?}", e)),
                };

                // helper_delete_test_db(conn, &config)?;
            }
            Err(e) => return Err(format!("test set-up err {:?}", e)),
        };
        res
    }

    #[test]
    fn checks_the_diff_in_run_migrations() {}
}
