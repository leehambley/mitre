use crate::{Direction, DriverResult, Error, Migration};

pub struct Driver {}

impl crate::NamedDriver for Driver {
    fn name() -> &'static str {
        "noop"
    }
}

impl crate::Driver for Driver {
    fn apply(&mut self, m: &Migration) -> Result<DriverResult, Error> {
        match (m.steps.get(&Direction::Up), m.steps.get(&Direction::Change)) {
            (Some(_), Some(_)) => Err(Error::MalformedMigration),
            _ => Ok(DriverResult::NothingToDo),
        }
    }
    fn unapply(&mut self, _: &Migration) -> Result<DriverResult, Error> {
        Ok(DriverResult::NothingToDo)
    }
}
