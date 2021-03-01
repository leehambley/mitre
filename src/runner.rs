pub mod mariadb;
use crate::config::{Configuration, RunnerConfiguration};

pub trait StateStore<'a> {
    type Error;
    type Migration;
    type MigrationStateTuple;
    type MigrationResultTuple;

    fn new_state_store(config: &Configuration) -> Result<Self, Self::Error>
    where
        Self: Sized;

    fn up(
        &mut self,
        _: Vec<Self::Migration>,
    ) -> Result<Vec<Self::MigrationResultTuple>, Self::Error>;

    fn diff(
        &mut self,
        _: Vec<Self::Migration>,
    ) -> Result<Vec<Self::MigrationStateTuple>, Self::Error>;
}

pub trait Runner {
    type Error;
    type Migration;
    type MigrationStep;

    fn new_runner(config: RunnerConfiguration) -> Result<Self, Self::Error>
    where
        Self: Sized;

    fn apply(&mut self, _: &Self::MigrationStep) -> Result<(), Self::Error>;
}
