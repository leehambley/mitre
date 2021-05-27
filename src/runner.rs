use crate::config::RunnerConfiguration;
use crate::migrations::Migration;
use crate::migrations::MigrationStep;
use log::trace;
use std::collections::HashMap;

pub mod mariadb;
pub mod postgresql;

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
    Orphaned,

    FilteredOut,
}

impl std::fmt::Display for MigrationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            MigrationState::Pending => write!(f, "Pending"),
            MigrationState::Applied => write!(f, "Applied"),
            MigrationState::Orphaned => write!(f, "Orphaned"),
            MigrationState::FilteredOut => write!(f, "Filtered Out"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum MigrationResult {
    AlreadyApplied,
    Success,
    Failure { reason: String },
    NothingToDo,
    IrreversibleMigration, // migration contains no "down" part.
    SkippedDueToEarlierError,
}

/// Analog the `from_config` in StateStora trait, which however does
/// not box the StateStore result.
pub fn from_config(rc: &RunnerConfiguration) -> Result<BoxedRunner, Error> {
    trace!("Getting runner from config {:?}", rc);
    if rc._runner.to_lowercase() == crate::reserved::MARIA_DB.to_lowercase() {
        return Ok(Box::new(mariadb::runner::MariaDb::new_runner(rc.clone())?));
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
