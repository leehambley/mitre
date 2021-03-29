use crate::config::RunnerConfiguration;
use crate::migrations::Migration;
use crate::migrations::MigrationStep;
use colored::*;
use std::collections::HashMap;

pub mod mariadb;
pub mod postgresql;
// pub mod redis;

#[derive(Debug)]
pub enum Error {
    MariaDb(mysql::Error),
    PostgreSql(postgres::error::Error),

    /// No configuration provided for the runner, which is a problem
    NoConfigForRunner {
        name: String,
    },

    // Attempted to use the wrong runner/config combo
    RunnerNameMismatch {
        expected: String,
        found: String,
    },

    /// Some runners need a database name to be provided (typically RDBMS) for flexibility
    /// including the ability to create databases in migrations, that database is tentatively
    /// selected and we won't fail until the very last moment that we need to select the database
    /// but cannot.
    CouldNotSelectDatabase,

    /// Could not get a runner
    CouldNotGetRunner {
        reason: String,
    },

    /// Template error such as a syntax error.
    TemplateError {
        reason: String,
        template: mustache::Template,
    },

    /// TODO: Describe these
    ErrorRunningMigration {
        cause: String,
    },

    /// We successfully ran the migration, but we didn't succeed in
    /// recording the status
    ErrorRecordingMigrationResult {
        cause: String,
    },

    // Couldn't make a runner from the config
    CouldNotFindOrCreateRunner,

    /// Migrations may not contain both "up" and "change"
    MigrationContainsBothUpAndChange(Migration),

    MigrationHasFailed(String, Migration),
}

impl From<mysql::Error> for Error {
    fn from(err: mysql::Error) -> Error {
        Error::MariaDb(err)
    }
}

impl From<postgres::error::Error> for Error {
    fn from(err: postgres::error::Error) -> Error {
        Error::PostgreSql(err)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Runner Error {:?}", self)
    }
}

pub type BoxedRunner = Box<dyn Runner>;
pub type RunnersHashMap = HashMap<crate::config::ConfigurationName, BoxedRunner>;

#[derive(PartialEq, Debug)]
pub enum MigrationState {
    Pending,
    Applied,
    // TODO: Orphaned (switched branch?)
}

impl std::fmt::Display for MigrationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            MigrationState::Pending => write!(f, "{}", "Pending".yellow()),
            MigrationState::Applied => write!(f, "{}", "Applied".green()),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum MigrationResult {
    AlreadyApplied,
    Success,
    Failure { reason: String },
    NothingToDo,
    SkippedDueToEarlierError, // not implemented yet, should be
}

pub fn from_config(rc: &RunnerConfiguration) -> Result<BoxedRunner, Error> {
    trace!("Getting runner from config {:?}", rc);
    if rc._runner.to_lowercase() == crate::reserved::MARIA_DB.to_lowercase() {
        return Ok(Box::new(mariadb::MariaDb::new_runner(rc.clone())?));
    }
    if rc._runner.to_lowercase() == crate::reserved::POSTGRESQL.to_lowercase() {
        return Ok(Box::new(postgresql::PostgreSql::new_runner(rc.clone())?));
    }
    Err(Error::CouldNotFindOrCreateRunner)
}

pub type MigrationTemplate = &'static str;
pub type MigrationFileExtension = &'static str;

pub trait Runner {
    fn new_runner(config: RunnerConfiguration) -> Result<Self, Error>
    where
        Self: Sized;

    fn apply(&mut self, _: &MigrationStep) -> Result<(), Error>;

    /// Returns tuple with up, down and file extension for the migration
    fn migration_template(&self) -> (MigrationTemplate, MigrationTemplate, MigrationFileExtension);
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

    use super::mariadb::MariaDb;
    use super::MigrationResult;
    use crate::config::Configuration;
    use crate::migrations::migrations;
    use crate::state_store::StateStore;
    use std::path::PathBuf;

    #[test]
    fn fixture_two_stops_executing_after_the_first_failure() -> Result<(), String> {
        let path = PathBuf::from(
            "./test/fixtures/example-2-the-second-of-three-migrations-fails/mitre.yml",
        );
        let config = match Configuration::from_file(&path) {
            Ok(config) => config,
            Err(e) => Err(format!("couldn't make config {}", e))?,
        };

        match MariaDb::reset_state_store(&config) {
            Ok(_) => {}
            Err(e) => return Err(format!("{:?}", e)),
        }

        let mut runner = MariaDb::new_state_store(&config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;

        let migrations = migrations(&config).expect("should make at least default migrations");

        // Arrange: Run up (only built-in, because tmp dir)
        match runner.up(migrations.clone()) {
            Ok(migration_results) => {
                // Built-in plus three in the fixture
                assert_eq!(4, migration_results.len());

                assert_eq!(MigrationResult::Success, migration_results[0].0);
                assert_eq!(MigrationResult::Success, migration_results[1].0);
                // assert_eq!(MigrationResult::SkippedDueToEarlierError, migration_results[2].0);
                assert_eq!(
                    MigrationResult::SkippedDueToEarlierError,
                    migration_results[3].0
                );

                let result_results: Vec<MigrationResult> =
                    migration_results.into_iter().map(|mr| mr.0).collect();
                print!("{:#?}", result_results);

                Ok(())
            }
            Err(e) => Err(format!("{:?}", e)),
        }
    }
}
