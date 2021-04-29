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

mod engine;
mod in_memory_migrations;
mod migration_list;
mod migration_storage;

pub use engine::Engine;
pub use in_memory_migrations::InMemoryMigrations;
pub use migration_list::MigrationList;
pub use migration_storage::MigrationStorage;

pub type MigrationStateTuple = (MigrationState, Migration);
pub type MigrationResultTuple = (MigrationResult, Migration);
#[derive(Debug)]
pub enum Error {}

#[cfg(test)]
#[ctor::ctor]
fn init() {
    env_logger::Builder::new()
        .filter(None, log::LevelFilter::Off)
        .parse_env("MITRE_TEST_LOG")
        .init();
}
