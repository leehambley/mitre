use ::mysql::Error as MySQLError;
use migrations::Migration;
use runner::{MigrationResult, MigrationState};

pub mod config;
pub mod exit_code;
pub mod ffi;
pub mod migrations;
pub mod reserved;
pub mod runner;
pub mod state_store;
pub mod ui;

mod mysql;

mod engine;
mod in_memory_migrations;
pub mod migration_list;
mod migration_storage;

pub use engine::Engine;
pub use in_memory_migrations::InMemoryMigrations;
pub use migration_list::{IntoIter, MigrationList};
pub use migration_storage::MigrationStorage;

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
        reason: Option<MySQLError>,
        msg: String,
    },
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
