use crate::config::Configuration;

pub trait StateStore {
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
