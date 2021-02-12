pub mod mariadb;
use crate::mitre::config::RunnerConfiguration;

pub trait Runner {
    type Error;
    type Migration;

    type MigrationStateTuple;
    // type Iter = Iterator<Item = Self::MigrationStateTuple>;

    fn new(config: &RunnerConfiguration) -> Result<Self, Self::Error>
    where
        Self: Sized;

    fn bootstrap(&mut self) -> Result<(), Self::Error>
    where
        Self: Sized;

    fn diff(
        &mut self,
        _: impl Iterator<Item = Self::Migration>,
    ) -> Result<Vec<Self::MigrationStateTuple>, Self::Error>
    where
        Self: Sized;
}
