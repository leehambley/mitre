use crate::config::RunnerConfiguration;
use crate::migrations::Migration;
use crate::migrations::MigrationStep;
use std::collections::HashMap;

pub mod mariadb;
pub mod postgresql;

#[derive(Debug)]
pub enum Error {
    MariaDb(mysql::Error),
    PostgreSQL(postgres::error::Error),

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

    /// Template error such as a syntax error.
    TemplateError {
        reason: String,
        template: mustache::Template,
    },

    /// TODO: Describe these
    ErrorRunningMigration {
        cause: mysql::Error,
    },

    MigrationHasFailed(String, Migration),
}

impl From<mysql::Error> for Error {
    fn from(err: mysql::Error) -> Error {
        Error::MariaDb(err)
    }
}

impl From<postgres::error::Error> for Error {
    fn from(err: postgres::error::Error) -> Error {
        Error::PostgreSQL(err)
    }
}

pub type BoxedRunner = Box<dyn Runner>;
pub type RunnersHashMap = HashMap<crate::reserved::Runner, BoxedRunner>;

#[derive(PartialEq, Debug)]
pub enum MigrationState {
    Pending,
    Applied,
    // TODO: Orphaned (switched branch?)
}

#[derive(PartialEq, Debug)]
pub enum MigrationResult {
    AlreadyApplied,
    Success,
    Failure(String),
    NothingToDo,
}

pub trait Runner {
    fn new_runner(config: RunnerConfiguration) -> Result<Self, Error>
    where
        Self: Sized;

    fn apply(&mut self, _: &MigrationStep) -> Result<(), Error>;
}
