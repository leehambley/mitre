pub mod mariadb;
use crate::mitre::config::RunnerConfiguration;

pub trait Runner {
    type Error;
    type Migration;
    type MigrationStep;

    type MigrationStateTuple;
    type MigrationResultTuple;

    fn new(config: &RunnerConfiguration) -> Result<Self, Self::Error>
    where
        Self: Sized;

    fn apply(&mut self, _: &Self::MigrationStep) -> Result<(), Self::Error>;

    fn up(
        &mut self,
        _: Vec<Self::Migration>,
    ) -> Result<Vec<Self::MigrationResultTuple>, Self::Error>;

    fn diff(
        &mut self,
        _: Vec<Self::Migration>,
    ) -> Result<Vec<Self::MigrationStateTuple>, Self::Error>;
}
