pub mod mariadb;
use crate::mitre::config::RunnerConfiguration;

pub trait Runner {
    type Error;
    type Migration;

    type MigrationStateTuple;

    fn new(config: &RunnerConfiguration) -> Result<Self, Self::Error>
    where
        Self: Sized;

    fn bootstrap(&mut self) -> Result<(), Self::Error>
    where
        Self: Sized;

    fn diff(
        &mut self,
        _: impl Iterator<Item = Self::Migration>,
    ) -> Result<Box<dyn Iterator<Item = Self::MigrationStateTuple>>, Self::Error>;
}
