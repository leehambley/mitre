pub mod mariadb;
use crate::mitre::config::RunnerConfiguration;

pub trait Runner<'a> {
    type Error;
    type Migration;
    type MigrationStep;

    type MigrationStateTuple;
    type MigrationResultTuple;

    fn new(config: &RunnerConfiguration) -> Result<Self, Self::Error>
    where
        Self: Sized;

    fn apply(&'a mut self, _: &Self::MigrationStep) -> Result<(), Self::Error>;

    fn up(
        &'a mut self,
        _: impl Iterator<Item = Self::Migration> + 'a,
    ) -> Result<Box<dyn Iterator<Item = Self::MigrationResultTuple> + 'a>, Self::Error>;

    fn diff(
        &'a mut self,
        _: impl Iterator<Item = Self::Migration> + 'a,
    ) -> Result<Box<dyn Iterator<Item = Self::MigrationStateTuple> + 'a>, Self::Error>;
}
