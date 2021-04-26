use crate::migrations::{Direction, Migration, MigrationStep};
use crate::runner::{BoxedRunner, Error as RunnerError, MigrationResult, MigrationState};
use crate::{config::Configuration, runner::RunnersHashMap};

#[derive(Debug)]
pub enum Error {
    MariaDb(mysql::Error),

    /// The configuration did not contain a `mitre: ...` block
    NoMitreConfigProvided,

    /// If a mitre: config is provided the database name is required
    /// even though the type is Option<String>.
    NoStateStoreDatabaseNameProvided,

    /// No supported state store in mitre entry of the configuration
    UnsupportedRunnerSpecified,

    /// Could not record success
    CouldNotRecordSuccess {
        reason: String,
    },

    /// An attempt was made to instantiate a runner or state store
    /// with a runner name that did not match the implementation's expected name.
    /// e.g starting a PostgreSQL state store with a value of "curl" in the runner name.
    /// Error contains the expected and actual names.
    RunnerNameMismatch {
        expected: String,
        found: String,
    },

    /// Error reading migration state from store, such as not being able
    /// to run the diff query for some reason. (different from an empty result)
    ErrorReadingMigrationState,

    /// Some kind of error, most likely bad config, or lost connection, usually
    RunnerError {
        reason: Box<RunnerError>,
    },

    /// This meand the runner look-up failed and is very serious, not the same as a regular RunnerError
    CouldNotFindOrCreateRunner,
}

impl From<mysql::Error> for Error {
    fn from(err: mysql::Error) -> Error {
        Error::MariaDb(err)
    }
}

impl From<RunnerError> for Error {
    fn from(err: RunnerError) -> Error {
        Error::RunnerError {
            reason: Box::new(err),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "State Store Error {:?}", self)
    }
}

pub type MigrationStateTuple = (MigrationState, Migration);
pub type MigrationResultTuple = (MigrationResult, Migration);

// pub trait StateStore {
//     #[cfg(test)] // testing helper, not thrilled about having this on the trait, but works for now.
//     fn reset(&mut self) -> Result<(), Error>
//     where
//         Self: Sized;

//     fn new_state_store(config: &Configuration) -> Result<Self, Error>
//     where
//         Self: Sized;

//     fn get_runner(&mut self, _: &Migration) -> Result<&mut crate::runner::BoxedRunner, Error>;

//     fn up(
//         &mut self,
//         _: Vec<Migration>,
//         _: Option<chrono::NaiveDateTime>,
//     ) -> Result<Vec<MigrationResultTuple>, Error>;

//     fn down(
//         &mut self,
//         _: Vec<Migration>,
//         _: Option<chrono::NaiveDateTime>,
//     ) -> Result<Vec<MigrationResultTuple>, Error>;

//     fn diff(&mut self, _: Vec<Migration>) -> Result<Vec<MigrationStateTuple>, Error>;
// }

pub trait StateStoreAdapter {
    fn new(config: &Configuration) -> Result<Self, Error>
    where
        Self: Sized;

    #[cfg(test)] // testing helper, not thrilled about having this on the trait, but works for now.
    fn reset(&mut self) -> Result<(), Error>;

    // TODO: rethink interface
    fn diff(&mut self, _: Vec<Migration>) -> Result<Vec<MigrationStateTuple>, Error>;

    fn record_success(&mut self, m: &Migration, d: std::time::Duration) -> Result<(), Error>;
    fn remove_success_record(
        &mut self,
        m: &Migration,
        _ms: &MigrationStep,
        _: std::time::Duration,
    ) -> Result<(), Error>;
}

pub struct StateStore {
    adapter: Box<dyn StateStoreAdapter>,
    runners: RunnersHashMap,
    config: Configuration,
}

impl StateStore {
    /// Takes a `crate::config::Configuration` and restores a
    //
    // Please make sure to add any new implementations to both places if the runner
    // is both a state store and a runner!
    pub fn from_config<'a>(c: &Configuration) -> Result<Self, Error> {
        match c.get("mitre") {
            Some(mc) => {
                if mc._runner.to_lowercase() == crate::reserved::MARIA_DB.to_lowercase() {
                    let adapter = crate::runner::mariadb::state_store::MariaDb::new(&c.clone())?;
                    let b: Box<dyn StateStoreAdapter> = Box::new(adapter);

                    Ok(StateStore {
                        adapter: b,
                        runners: RunnersHashMap::new(),
                        config: c.clone(),
                    })
                } else {
                    Err(Error::UnsupportedRunnerSpecified)
                }
            }
            None => Err(Error::NoMitreConfigProvided),
        }
    }

    #[cfg(test)] // testing helper, not thrilled about having this on the trait, but works for now.
    fn reset(&mut self) -> Result<(), Error> {
        self.adapter.reset()
    }

    pub fn up(
        &mut self,
        migrations: Vec<Migration>,
        dt: Option<chrono::NaiveDateTime>,
    ) -> Result<Vec<MigrationResultTuple>, Error> {
        let _apply_until = dt.unwrap_or_else(|| chrono::Utc::now().naive_utc());
        let mut stop_applying = false;
        Ok(self
            .diff(migrations)?
            .into_iter()
            .map(|(migration_state, migration)| match migration_state {
                MigrationState::Pending => match (
                    stop_applying,
                    migration.steps.get(&Direction::Change),
                    migration.steps.get(&Direction::Up),
                ) {
                    (true, _, _) => (MigrationResult::SkippedDueToEarlierError, migration),
                    (false, Some(_up_step), Some(_change_step)) => (
                        MigrationResult::Failure {
                            reason: String::from("contains both up and down parts"),
                        },
                        migration,
                    ),
                    (false, Some(up_step), None) => {
                        let (migration_result, migration) =
                            self.apply_migration_step(migration.clone(), up_step);
                        match migration_result {
                            MigrationResult::Failure { reason: _ } => {
                                warn!("migration {:?} failed, will stop applying", migration);
                                stop_applying = true;
                            }
                            _ => {}
                        }
                        (migration_result, migration)
                    }
                    (false, None, Some(change_step)) => {
                        let (migration_result, migration) =
                            self.apply_migration_step(migration.clone(), change_step);
                        match migration_result {
                            MigrationResult::Failure { reason: _ } => {
                                warn!("migration {:?} failed, will stop applying", migration);
                                stop_applying = true;
                            }
                            _ => {}
                        }
                        (migration_result, migration)
                    }
                    (false, None, None) => (MigrationResult::NothingToDo, migration),
                },
                MigrationState::Orphaned => match migration.steps.get(&Direction::Down) {
                    Some(down_step) => self.unapply_migration_step(migration.clone(), down_step),
                    None => (MigrationResult::IrreversibleMigration, migration),
                },
                MigrationState::Applied => (MigrationResult::AlreadyApplied, migration),
            })
            .collect())
    }

    fn unapply_migration_step(
        &mut self,
        m: Migration,
        ms: &MigrationStep,
    ) -> (MigrationResult, Migration) {
        let start = std::time::Instant::now();

        match self.get_runner(&m) {
            Err(e) => (
                MigrationResult::Failure {
                    reason: format!("{:?}", e),
                },
                m,
            ),
            Ok(runner) => match runner.apply(ms) {
                Ok(_) => match self.adapter.remove_success_record(&m, ms, start.elapsed()) {
                    Ok(_) => (MigrationResult::Success, m),
                    Err(e) => (
                        MigrationResult::Failure {
                            reason: e.to_string(),
                        },
                        m,
                    ),
                },
                Err(e) => (
                    MigrationResult::Failure {
                        reason: e.to_string(),
                    },
                    m,
                ),
            },
        }
    }

    fn apply_migration_step(
        &mut self,
        m: Migration,
        ms: &MigrationStep,
    ) -> (MigrationResult, Migration) {
        let start = std::time::Instant::now();

        match self.get_runner(&m) {
            Err(e) => (
                MigrationResult::Failure {
                    reason: format!("{:?}", e),
                },
                m,
            ),
            Ok(runner) => match runner.apply(ms) {
                Ok(_) => match self.adapter.record_success(&m, start.elapsed()) {
                    Ok(_) => (MigrationResult::Success, m),
                    Err(e) => (
                        MigrationResult::Failure {
                            reason: e.to_string(),
                        },
                        m,
                    ),
                },
                Err(e) => (
                    MigrationResult::Failure {
                        reason: e.to_string(),
                    },
                    m,
                ),
            },
        }
    }

    /// Given the set of runner configs on config, this will
    /// try to create a
    fn get_runner(&mut self, m: &Migration) -> Result<&mut BoxedRunner, Error> {
        // If we have a cached runner miss, let's
        trace!(
            "looking up runner for {} in MariaDB StateStore",
            m.configuration_name
        );
        if self.runners.get(&m.configuration_name).is_none() {
            let runner_config = match self.config.get(&m.configuration_name) {
                Some(rc) => rc,
                None => return Err(Error::CouldNotFindOrCreateRunner),
            };

            let new_runner = crate::runner::from_config(&runner_config)?;

            match self
                .runners
                .insert(m.configuration_name.clone(), new_runner)
            {
                None => trace!(
                    "clean insert of {} ({}) into runners map, no old value",
                    m.configuration_name,
                    runner_config._runner
                ),
                Some(_) => warn!(
                    "insert of {} ({}) into runners map overwrote a previous value, race condition?",
                    m.configuration_name,
                    runner_config._runner
                ),
            };
        }

        match self.runners.get_mut(&m.configuration_name) {
            Some(r) => Ok(r),
            None => Err(Error::CouldNotFindOrCreateRunner),
        }
    }

    pub fn down(
        &mut self,
        migrations: Vec<Migration>,
        dt: Option<chrono::NaiveDateTime>,
    ) -> Result<Vec<MigrationResultTuple>, Error> {
        let _unapply_after = dt.unwrap_or_else(|| chrono::Utc::now().naive_utc());
        Ok(self
            .diff(migrations)?
            .into_iter()
            .rev()
            .map(
                |(migration_state, migration)| -> (MigrationResult, Migration) {
                    match migration_state {
                        MigrationState::Applied | MigrationState::Orphaned => {
                            match migration.steps.get(&Direction::Down) {
                                Some(down_step) => {
                                    self.unapply_migration_step(migration.clone(), down_step)
                                }
                                None => (MigrationResult::IrreversibleMigration, migration),
                            }
                        }
                        MigrationState::Pending => (MigrationResult::NothingToDo, migration),
                    }
                },
            )
            .collect())
    }

    pub fn diff(&mut self, m: Vec<Migration>) -> Result<Vec<MigrationStateTuple>, Error> {
        self.adapter.diff(m)
    }
}

#[cfg(test)]
mod tests {

    extern crate rand;
    extern crate tempdir;

    use super::*;

    use crate::config::{Configuration, RunnerConfiguration};
    use crate::migrations::{migrations, Migration};
    use crate::runner::{MigrationResult, MigrationState};
    use crate::state_store::Error as StateStoreError;
    use indoc::indoc;
    use maplit::hashmap;
    use mysql::prelude::Queryable;
    use mysql::{Conn, OptsBuilder};
    use rand::Rng;
    use serial_test::serial;
    use std::path::PathBuf;
    use tempdir::TempDir;

    const TEST_DB_IP: &'static str = "127.0.0.1";
    const TEST_DB_PORT: u16 = 3306;
    const TEST_DB_USER: &'static str = "root";
    const TEST_DB_PASSWORD: &'static str = "example";

    struct TestDB {
        conn: mysql::Conn,
        config: Configuration,
    }

    impl Drop for TestDB {
        fn drop(&mut self) {
            for (_, rc) in &self.config.configured_runners {
                match helper_delete_test_db(&mut self.conn, &rc) {
                    Ok(_) => debug!("success cleaning up db {:?} text database", rc.database),
                    Err(e) => info!(
                        "error, there may be some clean-up to do for {:?}: {:?}",
                        rc, e
                    ),
                };
            }
        }
    }

    #[test]
    #[serial]
    fn fixture_two_stops_executing_after_the_first_failure() -> Result<(), String> {
        let path = PathBuf::from(
            "./test/fixtures/example-2-the-second-of-three-migrations-fails/mitre.yml",
        );
        let config = match Configuration::from_file(&path) {
            Ok(config) => config,
            Err(e) => Err(format!("couldn't make config {}", e))?,
        };

        let mut state_store = StateStore::from_config(&config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;

        match state_store.reset() {
            Ok(_) => {}
            Err(e) => return Err(format!("{:?}", e)),
        }

        let migrations = migrations(&config).expect("should make at least default migrations");

        // Arrange: Run up (only built-in, because tmp dir)
        match state_store.up(migrations.clone(), None) {
            Ok(migration_results) => {
                // Built-in plus three in the fixture
                assert_eq!(4, migration_results.len());

                assert_eq!(MigrationResult::Success, migration_results[0].0);
                assert_eq!(MigrationResult::Success, migration_results[1].0);
                match migration_results[2].0 {
                    MigrationResult::Failure { reason: _ } => {}
                    _ => return Err(format!("expected results[1].0 to be Failure")),
                }
                assert_eq!(
                    MigrationResult::SkippedDueToEarlierError,
                    migration_results[3].0
                );

                Ok(())
            }
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    #[test]
    #[serial]
    fn test_down_migration() -> Result<(), String> {
        let path = PathBuf::from("./test/fixtures/example-3-all-migrations-succeed/mitre.yml");

        let config = match Configuration::from_file(&path) {
            Ok(config) => config,
            Err(e) => Err(format!("couldn't make config {}", e))?,
        };

        let mut state_store = StateStore::from_config(&config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;

        match state_store.reset() {
            Ok(_) => {}
            Err(e) => return Err(format!("{:?}", e)),
        }

        let migrations = migrations(&config).expect("should make at least default migrations");

        // Arrange: Run up (only built-in, because tmp dir)
        match state_store.up(migrations.clone(), None) {
            Ok(migration_results) => {
                // Built-in plus three in the fixture
                assert_eq!(2, migration_results.len());
                assert_eq!(MigrationResult::Success, migration_results[0].0); // built-in
                assert_eq!(MigrationResult::Success, migration_results[1].0);
            }
            Err(e) => return Err(format!("{:?}", e)),
        }

        // Act: Run down
        match state_store.down(migrations.clone(), None) {
            Ok(migration_results) => {
                // Built-in plus three in the fixture
                assert_eq!(2, migration_results.len());

                // NOTE: results are reversed when dealing with down()
                assert_eq!(MigrationResult::Success, migration_results[0].0);
                assert_eq!(
                    MigrationResult::IrreversibleMigration,
                    migration_results[1].0
                ); // built-in

                Ok(())
            }
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    #[test]
    #[serial]
    fn test_diff_detects_orphaned_migrations() -> Result<(), String> {
        let main_path =
            PathBuf::from("./test/fixtures/example-4-orphaned-migrations/mitre-main.yml");
        let main_config = match Configuration::from_file(&main_path) {
            Ok(config) => config,
            Err(e) => Err(format!("couldn't make config {}", e))?,
        };

        let mut main_state_store = StateStore::from_config(&main_config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;

        let alt_path = PathBuf::from("./test/fixtures/example-4-orphaned-migrations/mitre-alt.yml");
        let alt_config = match Configuration::from_file(&alt_path) {
            Ok(config) => config,
            Err(e) => Err(format!("couldn't make config {}", e))?,
        };

        let mut alt_state_store = StateStore::from_config(&alt_config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;

        let alt_migrations =
            migrations(&alt_config).expect("should make at least default migrations");

        match alt_state_store.reset() {
            Ok(_) => {}
            Err(e) => return Err(format!("{:?}", e)),
        }
        match main_state_store.reset() {
            Ok(_) => {}
            Err(e) => return Err(format!("{:?}", e)),
        }

        match alt_state_store.up(alt_migrations.clone(), None) {
            Err(e) => panic!("error running up {:?}", e),
            _ => {}
        }
        info!("alt migrations {:#?}", alt_migrations);

        let main_migrations =
            migrations(&main_config).expect("should make at least default migrations");

        match main_state_store.diff(main_migrations.clone()) {
            Err(e) => panic!("error running diff {:?}", e),
            Ok(result) => {
                trace!("result is {:#?}", result);
                assert_eq!(
                    result.len(),
                    alt_migrations.len(),
                    "diff result should be the length of the alt (longer than main) results"
                );

                let orphaned_migrations: Vec<MigrationStateTuple> = result
                    .into_iter()
                    .filter(|k| k.0 == MigrationState::Orphaned)
                    .collect();

                assert_eq!(
                    1,
                    orphaned_migrations.len(),
                    "one orphaned migration from alt"
                );
            }
        }

        Ok(())
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
            migrations_directory: PathBuf::from(
                TempDir::new("helper_create_runner_config")
                    .expect("could not make tmpdir")
                    .into_path(),
            ),
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
    #[serial]
    fn it_requires_a_config_with_a_table_name() -> Result<(), String> {
        let config = helper_create_runner_config(None {});
        let mut runner = StateStore::from_config(&config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;
        let migrations = vec![];

        let x = match runner.diff(migrations) {
            Ok(_) => Err(String::from("expected an error about missing dbname")),
            Err(e) => match e {
                StateStoreError::NoStateStoreDatabaseNameProvided => Ok(()),
                _ => Err(format!("did not expect error {:?}", e)),
            },
        };
        x
    }

    #[test]
    #[serial]
    fn it_returns_all_migrations_pending_if_db_does_not_exist() -> Result<(), String> {
        let config = helper_create_runner_config(Some(""));
        let mut runner = StateStore::from_config(&config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;
        let migrations = migrations(&config).expect("should make at least default migrations");

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
    #[serial]
    fn it_returns_all_migrations_pending_if_migrations_table_does_not_exist() -> Result<(), String>
    {
        let test_db = helper_create_test_db()?;
        let config = match Configuration::load_from_str(indoc!(
            r"
          ---
          mitre:
            _runner: mariadb
        "
        )) {
            Ok(c) => c,
            Err(e) => Err(format!("error generating config: {}", e))?,
        };

        let mut runner = StateStore::from_config(&test_db.config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;
        let migrations = migrations(&config).expect("should make at least default migrations");

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
    #[serial]
    fn migrating_up_just_the_built_in_migrations() -> Result<(), String> {
        let config = helper_create_runner_config(Some(""));

        let mut runner = StateStore::from_config(&config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;
        let migrations = migrations(&config).expect("should make at least default migrations");

        // Arrange: Run up (only built-in, because tmp dir)
        match runner.up(migrations.clone(), None) {
            Ok(migration_results) => {
                print!("{:#?}", migration_results);

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
        match runner.up(migrations, None) {
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
