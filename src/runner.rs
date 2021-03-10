use crate::config::RunnerConfiguration;

pub mod mariadb;
pub mod postgresql;
// mod redis;
// mod http;

#[derive(PartialEq, Debug)]
pub enum MigrationState {
    Pending,
    Applied,
    // Orphaned (switched branch?)
}

#[derive(PartialEq, Debug)]
pub enum MigrationResult {
    AlreadyApplied,
    Success,
    Failure(String),
    NothingToDo,
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
