pub mod mariadb;
use crate::mitre::config::RunnerConfiguration;

pub trait Runner {
    type Error;

    fn new(config: &RunnerConfiguration) -> Result<Self, Self::Error>
    where
        Self: Sized;

    fn bootstrap(&mut self) -> Result<(), Self::Error>
    where
        Self: Sized;
}
