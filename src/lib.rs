#[macro_use]
extern crate log; // TODO: replace this with a use() statement?

pub mod config;
pub mod exit_code;
pub mod ffi;
pub mod migrations;
pub mod reserved;
pub mod runner;
pub mod state_store;
pub mod ui;

use migrations::Migration;
use runner::{MigrationResult, MigrationState};
use std::vec::IntoIter;

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

pub type MigrationStateTuple = (MigrationState, Migration);
pub type MigrationResultTuple = (MigrationResult, Migration);

struct InMemoryMigrations {
    m: Vec<Migration>,
}

impl InMemoryMigrations {
    fn new(m: Vec<Migration>) -> Self {
        InMemoryMigrations { m }
    }
}

impl MigrationList for InMemoryMigrations {
    fn all(&self) -> Result<IntoIter<Migration>, Error> {
        Ok(self.m.clone().into_iter())
    }
}

impl MigrationStorage for InMemoryMigrations {
    fn add(&mut self, _: Migration) -> Result<(), Error> {
        Ok(())
    }
    fn remove(&mut self, _: Migration) -> Result<(), Error> {
        Ok(())
    }
}

// https://doc.rust-lang.org/std/iter/trait.IntoIterator.html
pub trait MigrationList {
    // TODO: investigate how to make this automatically into_iter
    fn all(&self) -> Result<IntoIter<Migration>, Error>;
}

pub trait MigrationStorage: MigrationList {
    fn add(&mut self, _: Migration) -> Result<(), Error>;
    fn remove(&mut self, _: Migration) -> Result<(), Error>;
}

pub trait Engine {
    // formerly state store
    fn diff(
        src: impl MigrationList,
        dest: impl MigrationStorage,
    ) -> Result<IntoIter<MigrationStateTuple>, Error>;

    // TODO: dry-run?

    fn apply(
        // TODO: This should maybe get a _filtered_ list, or some query plan?
        src: impl MigrationList,
        dest: impl MigrationStorage,
    ) -> Result<IntoIter<MigrationStateTuple>, Error>;
}

#[cfg(test)]
mod migration_list_tests {

    #![feature(concat_idents)]

    extern crate rand;
    extern crate tempdir;

    use super::*;

    fn test_migration_list_is_empty_when_new(x: &dyn MigrationList) -> Result<(), String> {
        Ok(())
    }

    macro_rules! test {
        // If we ever get better support for concat_idents, you could get rid of
        // the test_name and gen it from the trait/type combo.
        // See https://github.com/rust-lang/rust/issues/29599.
        ($test_name:ident, $ml:expr) => {
            #[test]
            pub fn $test_name() -> Result<(), String> {
                let x: &dyn MigrationList = $ml;
                test_migration_list_is_empty_when_new(x)
            }
        };
    }

    test!(
        in_memory_migrations_empty_when_new,
        &InMemoryMigrations::new(vec![])
    );

    #[test]
    fn test_listing_migrations() -> Result<(), Error> {
        Ok(())
    }
}
