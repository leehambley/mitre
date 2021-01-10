pub mod mariadb;
use crate::config::Configuration;

pub trait Runner {
    type Errorrr;

    fn new(config: &Configuration) -> Result<Self, Self::Errorrr>
    where
        Self: Sized;

    fn bootstrap(&mut self) -> Result<(), Self::Errorrr>
    where
        Self: Sized;
}
