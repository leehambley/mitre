pub mod config;
pub mod ffi;
pub mod migrations;
pub mod reserved;
pub mod ui;

mod driver;
mod engine;
mod in_memory_migrations;
mod migration_list;
mod migration_storage;
mod mysql;
mod runner;

// Public reuse defines the public API so that all other
// modules can simply reuse these types without knowing
// where they come from. The concept of Driver, DriverResult,
// Migration, MigrationStateTuple, etc all belong here.
pub use self::mysql::MySQL; // self:: required due to name conflict with MySQL crate.
pub use config::{Configuration, ConfigurationName, RunnerConfiguration};
pub use driver::{Driver, DriverResult, NamedDriver, StepDriver};
pub use engine::Engine;
pub use in_memory_migrations::InMemoryMigrations;
pub use migration_list::{from_disk as migration_list_from_disk, MigrationList};
pub use migration_storage::{from_config as migration_storage_from_config, MigrationStorage};
pub use migrations::{
    Direction, Migration, MigrationStep, MigrationSteps, FORMAT_STR as TIMESTAMP_FORMAT_STR,
};
pub use reserved::{Flag, Runner};
pub use runner::from_config as runner_from_config;
pub use runner::{MigrationResult, MigrationState};

pub type MigrationStateTuple = (MigrationState, Migration);
pub type MigrationResultTuple = (MigrationResult, Migration);

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),

    // Configuration is missing an optional (i.e syntactically)
    // but required option, such as when the MySQL database
    // name is not provided.
    ConfigurationIncomplete,

    // An error was encountered running some query in a database
    // or something.
    QueryFailed {
        reason: Option<::mysql::Error>,
        msg: String,
    },

    ConnectionError {
        msg: String,
    },

    // Migration probably contains Up+Change or some other illegal
    // combination of steps.
    MalformedMigration,

    // No mitre config provided, so we cannot initialize anything
    NoMitreConfigProvided,

    // UnsupportedRunnerSpecified
    // mitre config is correct, but the _runner field is set to a value
    // we do not support.
    UnsupportedRunnerSpecified,
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::Io(err)
    }
}

#[cfg(test)]
#[ctor::ctor]
fn init() {
    env_logger::Builder::new()
        .filter(None, log::LevelFilter::Off)
        .parse_env("MITRE_TEST_LOG")
        .init();
}
