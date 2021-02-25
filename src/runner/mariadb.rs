use crate::config::{Configuration, RunnerConfiguration};
use crate::migrations::{Direction, Migration, MigrationStep};
use crate::reserved::Runner as RunnerReservedWord;
use crate::runner::Runner;
use mustache::MapBuilder;
use mysql::prelude::Queryable;
use mysql::{Conn, OptsBuilder};
use std::collections::HashMap;
use std::convert::From;

const MARIADB_MIGRATION_STATE_TABLE_NAME: &str = "mitre_migration_state";

type BoxedRunner =
    Box<dyn Runner<Error = Error, Migration = Migration, MigrationStep = MigrationStep>>;
type RunnersHashMap = HashMap<RunnerReservedWord, BoxedRunner>;

/// MariaDb is both a StateStore and a runner. The bootstrapping phase
/// means that when no migrations have yet been run, the StateStore may
/// attempt to connect to the database server when no database, or a
/// database with no tables exists. When bootstrapping the first connections
/// may swallow errors, the `diff()` method of StateStore may simply
/// return that all migrations are unapplied. Once the bootstrap migration
/// has run, it should be possible for the state store behaviour to
/// properly store results.
pub struct MariaDb {
    conn: Conn,

    // All configurations because as a state-store MariaDb also
    // has to be able to look-up the correct implementation for.
    // Option<T> because we may simply be a runner (if someone uses
    // another store, and MariaDb is only used for running)
    config: Option<Configuration>,

    // Configuration in case we are a runner not a state store
    runner_config: RunnerConfiguration,

    // Runners in a muxed'ed hashmap. This hashmap is keyed by [`crate::reserved::Runner`]
    runners: RunnersHashMap,
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
    /// Shadowing the errors from the mysql crate to have them type-safely in our scope.
    MariaDb(mysql::Error),
    /// The configuration did not contain a `mitre: ...` block
    NoMitreConfigProvided,
    /// An attempt was made to instantiate a runner or state store
    /// with a runner name that did not match the implementation's expected name.
    /// e.g starting a PostgreSQL state store with a value of "curl" in the runner name.
    /// Error contains the expected and actual names.
    RunnerNameMismatch {
        expected: String,
        found: String,
    },
    /// No runner name
    NoMariaDbConfigProvided,
    CouldNotSelectDatabase,
    NoStateStoreDatabaseNameProvided,
    // (reason, the template)
    TemplateError(String, mustache::Template),
    ErrorRunningMigration(String),
    MigrationHasFailed(String, Migration),

    /// When in StateStore mode, we need to instantiate runners
    /// from configurations, if we fail to do that, we'll bubble
    /// up this error with the runner configuration.
    ///
    /// TODO: Instances of this should have good errors
    CannotCreateRunnerFor,
    ErrorRunningQuery,
}

impl From<mysql::Error> for Error {
    fn from(err: mysql::Error) -> Error {
        Error::MariaDb(err)
    }
}

/// Helper methods for MariaDb (non-public) used in the context
/// of fulfilling the implementation of the runner::Runner trait.
impl MariaDb {
    /// Given the set of runner configs on config, this will
    /// try to create a
    fn get_runner(&mut self, ms: &MigrationStep) -> Result<&mut BoxedRunner, Error> {
        // If we have a cached runner miss, let's
        trace!("looking up runner for {:?}", ms);
        if self.runners.get(&ms.runner).is_none() {
            trace!("none found, will create");
            warn!("!! hard-coded only to create MariaDb runners !!");
            match self.runners.insert(
                ms.runner.clone(),
                Box::new(MariaDb::new_runner(self.runner_config.clone())?),
            ) {
                None => trace!(
                    "clean insert of {} into runners map, no old value",
                    ms.runner.name
                ),
                Some(_) => warn!(
                    "insert of {} into runners map overwrote a previous value, race condition?",
                    ms.runner.name
                ),
            };
        }

        trace!("returning from get_runner with runner");
        match self.runners.get_mut(&ms.runner) {
            Some(r) => Ok(r),
            None => Err(Error::CannotCreateRunnerFor),
        }
    }

    fn select_db(&mut self) {
        match &self.runner_config.database {
            Some(database) => {
                trace!("select_db database name is {}", database);
                match &self.conn.select_db(&database) {
                    true => trace!("select_db successfully using {}", database),
                    false => trace!("could not switch to {} (may not exist yet?)", database),
                }
            }
            None => trace!("select_db no database name provided"),
        }
    }

    /// We do not record failures,
    fn record_success(
        &mut self,
        m: &Migration,
        _ms: &MigrationStep,
        d: std::time::Duration,
    ) -> Result<(), Error> {
        self.select_db(); // TODO: maybe move select_db inside .conn -> .conn()
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

impl crate::runner::StateStore for MariaDb {
    type Error = Error;
    type Migration = Migration;
    type MigrationStateTuple = (MigrationState, Migration);
    type MigrationResultTuple = (MigrationResult, Migration);

    fn new_state_store(config: Configuration) -> Result<MariaDb, Error> {
        let runner_name = String::from(crate::reserved::MARIA_DB).to_lowercase();
        let mariadb_config = match config.get("mitre") {
            None => {
                debug!("no config entry `mitre' found, please check the docs");
                return Err(Error::NoMitreConfigProvided);
            }
            Some(c) => {
                if c._runner.to_lowercase() == runner_name {
                    c.clone()
                } else {
                    warn!("runner name mismatch, please check the docs and your config");
                    return Err(Error::RunnerNameMismatch {
                        expected: runner_name,
                        found: c._runner.to_lowercase(),
                    });
                }
            }
        };

        let opts = mysql::Opts::from(
            OptsBuilder::new()
                .ip_or_hostname(mariadb_config.ip_or_hostname.clone())
                .user(mariadb_config.username.clone())
                // NOTE: Do not specify database name here, otherwise we cannot
                // connect until the database exists. Makes it difficult to
                // bootstrap.
                // .db_name(mariadb_config.database.clone())
                .pass(mariadb_config.password.clone()),
        );
        Ok(MariaDb {
            conn: Conn::new(opts)?,
            config: None {}, // we are a runner
            runner_config: mariadb_config,
            runners: RunnersHashMap::new(),
        })
    }

    fn up(
        &mut self,
        migrations: Vec<Self::Migration>,
    ) -> Result<Vec<Self::MigrationResultTuple>, Error> {
        Ok(self
            .diff(migrations)?
            .into_iter()
            .map(|(migration_state, migration)| {
                match migration_state {
                    // TODO: check mgiation_state is pending
                    // TODO: blow-up (mayne not here?) if we have up+change, only up+down makes sense.migration
                    MigrationState::Pending => match migration.steps.get(&Direction::Change) {
                        Some(change_step) => {
                            let start = std::time::Instant::now();
                            trace!("starting apply");

                            match self.get_runner(change_step) {
                                Err(e) => (
                                    MigrationResult::Failure(format!(
                                        "error setting up runner for change step{:?}",
                                        e
                                    )),
                                    migration,
                                ),
                                Ok(runner) => match runner.apply(change_step) {
                                    Ok(_) => {
                                        trace!("apply ok");
                                        match self.record_success(
                                            &migration,
                                            change_step,
                                            start.elapsed(),
                                        ) {
                                            Ok(_) => (MigrationResult::Success, migration),
                                            Err(e) => (
                                                MigrationResult::Failure(format!("{:?}", e)),
                                                migration,
                                            ),
                                        }
                                    }
                                    Err(e) => {
                                        trace!("apply not ok");
                                        (MigrationResult::Failure(format!("{:?}", e)), migration)
                                    }
                                },
                            }
                        }
                        None => (MigrationResult::NothingToDo, migration),
                    },
                    MigrationState::Applied => {
                        info!("already applied {}", migration.date_time);

                        (MigrationResult::AlreadyApplied, migration)
                    }
                }
            })
            .collect())
    }

    fn diff(
        &mut self,
        migrations: Vec<Self::Migration>,
    ) -> Result<Vec<Self::MigrationStateTuple>, Error> {
        self.select_db();

        let database = match &self.runner_config.database {
            Some(database) => Ok(database),
            None => Err(Error::NoStateStoreDatabaseNameProvided),
        }?;

        let schema_exists = self.conn.exec_first::<bool, _, _>(
      "SELECT EXISTS(SELECT SCHEMA_NAME FROM INFORMATION_SCHEMA.SCHEMATA WHERE SCHEMA_NAME = ?)",
      (database,)
    )?;

        match schema_exists {
            Some(schema_exists) => {
                println!("schema exists?: {}", schema_exists);
                if !schema_exists {
                    return Ok(migrations
                        .into_iter()
                        .map(|m| (MigrationState::Pending, m))
                        .collect());
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
            match self.conn.query_map::<_,_,_,String>(format!("SELECT `version` FROM `{}` ORDER BY `version` ASC;", MARIADB_MIGRATION_STATE_TABLE_NAME), |version| version) {
            Ok(stored_migration_versions) =>
               Ok(migrations.into_iter().map(move |m| {
                let migration_version = format!("{}", m.date_time.format(crate::migrations::FORMAT_STR));
                trace!("applied migrations {:?}", stored_migration_versions );
                match stored_migration_versions.clone().into_iter().find(|stored_m| &migration_version == stored_m ) {
                    Some(_) => { trace!("found applied"); (MigrationState::Applied, m)},
                    None => { trace!("found pending"); (MigrationState::Pending, m)}
                }
              }).collect()),
            Err(e) => {
              warn!("could not check for migrations {:?}", e);
              Err(Error::MariaDb(e))
            }
          }
        }
      },
      None => Err(Error::ErrorRunningQuery)
    }
    }
}

impl crate::runner::Runner for MariaDb {
    type Error = Error;
    type Migration = Migration;
    type MigrationStep = MigrationStep;

    fn new_runner(config: RunnerConfiguration) -> Result<MariaDb, Error> {
        let runner_name = String::from(crate::reserved::MARIA_DB).to_lowercase();
        let mariadb_config = if config._runner.to_lowercase() == runner_name {
            config.clone()
        } else {
            return Err(Error::RunnerNameMismatch {
                expected: runner_name,
                found: config._runner,
            });
        };

        let opts = mysql::Opts::from(
            OptsBuilder::new()
                .ip_or_hostname(mariadb_config.ip_or_hostname.clone())
                .user(mariadb_config.username.clone())
                // NOTE: Do not specify database name here, otherwise we cannot
                // connect until the database exists. Makes it difficult to
                // bootstrap.
                // .db_name(mariadb_config.database.clone())
                .pass(mariadb_config.password.clone()),
        );
        Ok(MariaDb {
            conn: Conn::new(opts)?,
            config: None {}, // we are a runner
            runner_config: mariadb_config,
            runners: RunnersHashMap::new(),
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
                self.runner_config.database.as_ref().unwrap(),
            )
            .build();

        trace!("rendering template to string");
        let parsed = match ms.content.render_data_to_string(&template_ctx) {
            Ok(str) => Ok(str),
            Err(e) => Err(Error::TemplateError(e.to_string(), ms.content.clone())),
        }?;
        trace!("template rendered to string successfully: {:#?}", parsed);

        debug!("executing query");
        match self.conn.query_iter(parsed) {
            Ok(mut res) => {
                // TODO: do something more with QueryResult
                trace!(
                    "Had {} warnings and this info: {}",
                    res.warnings(),
                    res.info_str()
                );
                trace!("applying parsed query success {:?}", res);
                while let Some(result_set) = res.next_set() {
                    let result_set = result_set.expect("boom");
                    info!(
                        "Result set meta: rows {}, last insert id {:?}, warnings {} info_str {}",
                        result_set.affected_rows(),
                        result_set.last_insert_id(),
                        result_set.warnings(),
                        result_set.info_str(),
                    );
                }
                Ok(())
            }
            Err(e) => {
                trace!("applying parsed query failed {:?}", e);
                Err(Error::ErrorRunningMigration(e.to_string()))
            }
        }
    }
}

#[cfg(test)]
#[ctor::ctor]
fn init() {
    env_logger::init();
}

#[cfg(test)]
mod tests {

    extern crate rand;
    extern crate tempdir;

    use super::*;
    use crate::migrations::migrations;
    use crate::runner::StateStore;
    use maplit::hashmap;
    use rand::Rng;
    use tempdir::TempDir;

    const TEST_DB_IP: &'static str = "127.0.0.1";
    const TEST_DB_PORT: u16 = 3306;
    const TEST_DB_USER: &'static str = "root";
    const TEST_DB_PASSWORD: &'static str = "example";

    struct TestDB {
        conn: mysql::Conn,
        // Config for impl Runner compatibility,
        // mariadb_config duplicated for convenience
        mariadb_config: RunnerConfiguration,
        config: Configuration,
    }

    impl Drop for TestDB {
        fn drop(&mut self) {
            println!("Dropping DB Conn");
            match helper_delete_test_db(&mut self.conn, &self.mariadb_config) {
                Ok(_) => println!("success"),
                Err(_) => println!("error, there may be some clean-up to do"),
            };
        }
    }

    fn helper_create_runner_config(dbname: Option<&str>) -> Configuration {
        // None means really none, but Some("") indicates that we should
        // generate a random one. A non-empty string will be used.
        let dbname = match dbname {
            Some(dbname) => Some(match dbname {
                "" => format!("mitre_test_{}", rand::thread_rng().gen::<u32>()),
                _ => dbname.to_string(),
            }),
            None => None,
        };
        Configuration {
            configured_runners: hashmap! {
                String::from("mariadb") => RunnerConfiguration {
                  _runner: String::from(crate::reserved::MARIA_DB).to_lowercase(),
                  database_number: None,
                  database: Some(format!("mitre_other_test_db_{}", rand::thread_rng().gen::<u32>()),),
                  index: None,
                  ip_or_hostname: Some(String::from(TEST_DB_IP)),
                  password: Some(String::from(TEST_DB_PASSWORD)),
                  port: Some(TEST_DB_PORT),
                  username: Some(String::from(TEST_DB_USER)),
              },
              String::from("mitre") => RunnerConfiguration {
                _runner: String::from(crate::reserved::MARIA_DB).to_lowercase(),
                database_number: None,
                database: dbname, // the one we want to bootstrap
                index: None,
                ip_or_hostname: Some(String::from(TEST_DB_IP)),
                password: Some(String::from(TEST_DB_PASSWORD)),
                port: Some(TEST_DB_PORT),
                username: Some(String::from(TEST_DB_USER)),
            }
            },
        }
    }

    fn helper_create_test_db() -> Result<TestDB, String> {
        let config = helper_create_runner_config(Some(""));
        let mariadb_config = config
            .configured_runners
            .get("mariadb")
            .ok_or_else(|| "no config")?;
        let mut conn = helper_db_conn()?;

        trace!("helper_create_test_db: creating database");
        match &mariadb_config.database {
            Some(dbname) => {
                let stmt_create_db = conn
                    .prep(format!("CREATE DATABASE `{}`", dbname))
                    .expect("could not prepare db create statement");
                match conn.exec::<bool, _, _>(stmt_create_db, ()) {
                    Err(e) => Err(format!("error creating test db {:?}", e)),
                    Ok(_) => Ok(TestDB {
                        conn,
                        config: config.clone(),
                        mariadb_config: mariadb_config.clone(),
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
        let config = helper_create_runner_config(None {});
        let mut runner = MariaDb::new_state_store(config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;
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
        let config = helper_create_runner_config(Some(""));
        let mut runner = MariaDb::new_state_store(config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;
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

        let mut runner = MariaDb::new_state_store(test_db.config.clone())
            .map_err(|e| format!("Could not create state store {:?}", e))?;
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

        let mut runner = MariaDb::new_state_store(test_db.config.clone())
            .map_err(|e| format!("Could not create state store {:?}", e))?;
        let migrations =
            migrations(tmp_dir.as_ref()).expect("should make at least default migrations");

        // Arrange: Run up (only built-in, because tmp dir
        match runner.up(migrations.clone()) {
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
        match runner.diff(migrations.clone()) {
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
        match runner.up(migrations) {
            Ok(migration_results) => {
                let diff_pending: Vec<(MigrationResult, Migration)> = migration_results
                    .into_iter()
                    .filter(|mr| mr.0 == MigrationResult::AlreadyApplied)
                    .collect();

                assert_eq!(1, diff_pending.len());
            }
            Err(e) => return Err(format!("did not expect error running up again {:?}", e)),
        };

        Ok(())
    }

    #[test]
    fn checks_the_diff_in_run_migrations() {}
}
