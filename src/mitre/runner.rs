pub mod mariadb;
use crate::mitre::config::Configuration;

pub trait Runner {
    type Error;

    fn new(config: &Configuration) -> Result<Self, Self::Error>
    where
        Self: Sized;

    fn bootstrap(&mut self) -> Result<(), Self::Error>
    where
        Self: Sized;
}
