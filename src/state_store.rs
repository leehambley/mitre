use crate::config::Configuration;
use crate::migrations::Migration;
use crate::runner::{Error as RunnerError, MigrationResult, MigrationState};

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

/// Takes a `crate::config::Configuration` and restores a
//
// Please make sure to add any new implementations to both places if the runner
// is both a state store and a runner!
pub fn from_config(c: &Configuration) -> Result<impl StateStore, Error> {
    match c.get("mitre") {
        Some(mc) => {
            if mc._runner.to_lowercase() == crate::reserved::MARIA_DB.to_lowercase() {
                Ok(crate::runner::mariadb::state_store::MariaDb::new_state_store(&c.clone())?)
            } else {
                Err(Error::UnsupportedRunnerSpecified)
            }
        }
        None => Err(Error::NoMitreConfigProvided),
    }
}

pub trait StateStore {
    #[cfg(test)] // testing helper, not thrilled about having this on the trait, but works for now.
    fn reset(&mut self) -> Result<(), Error>
    where
        Self: Sized;

    fn new_state_store(config: &Configuration) -> Result<Self, Error>
    where
        Self: Sized;

    fn get_runner(&mut self, _: &Migration) -> Result<&mut crate::runner::BoxedRunner, Error>;

    fn up(
        &mut self,
        _: Vec<Migration>,
        _: Option<chrono::NaiveDateTime>,
    ) -> Result<Vec<MigrationResultTuple>, Error>;

    fn down(
        &mut self,
        _: Vec<Migration>,
        _: Option<chrono::NaiveDateTime>,
    ) -> Result<Vec<MigrationResultTuple>, Error>;

    fn diff(&mut self, _: Vec<Migration>) -> Result<Vec<MigrationStateTuple>, Error>;
}

#[cfg(test)]
mod tests {

    // extern crate rand;
    // extern crate tempdir;

    // use super::*;
    // use crate::migrations::migrations;
    // use crate::runner::MigrationState;
    // use indoc::indoc;
    // use maplit::hashmap;
    // use mysql::OptsBuilder;
    // use rand::Rng;
    // use std::path::PathBuf;
    // use tempdir::TempDir;
    use super::*;
    use crate::config::Configuration;
    use crate::migrations::migrations;
    use serial_test::serial;
    use std::path::PathBuf;

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

        let mut state_store =
            from_config(&config).map_err(|e| format!("Could not create state store {:?}", e))?;

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

        let mut state_store =
            from_config(&config).map_err(|e| format!("Could not create state store {:?}", e))?;

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

        let mut main_state_store = from_config(&main_config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;

        let alt_path = PathBuf::from("./test/fixtures/example-4-orphaned-migrations/mitre-alt.yml");
        let alt_config = match Configuration::from_file(&alt_path) {
            Ok(config) => config,
            Err(e) => Err(format!("couldn't make config {}", e))?,
        };

        let mut alt_state_store = from_config(&alt_config)
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
}
