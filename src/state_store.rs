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

pub trait StateStore {
    #[cfg(test)] // testing helper, not thrilled about having this on the trait, but works for now.
    fn reset_state_store(config: &Configuration) -> Result<(), Error>
    where
        Self: Sized;

    fn new_state_store(config: &Configuration) -> Result<Self, Error>
    where
        Self: Sized;

    fn get_runner(&mut self, _: &Migration) -> Result<&mut crate::runner::BoxedRunner, Error>;

    fn up(&mut self, _: Vec<Migration>) -> Result<Vec<MigrationResultTuple>, Error>;

    fn down(&mut self, _: Vec<Migration>) -> Result<Vec<MigrationResultTuple>, Error>;

    fn diff(&mut self, _: Vec<Migration>) -> Result<Vec<MigrationStateTuple>, Error>;
}
