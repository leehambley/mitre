use crate::{Direction, DriverResult, Error, Migration, MigrationStep};

pub struct Driver {}

impl crate::NamedDriver for Driver {
    fn name() -> &'static str {
        "succeed_or_fail"
    }
}

impl crate::Driver for Driver {
    // If the migration step source is SUCCEED it returns Success
    // else it returns
    fn apply(&mut self, m: &Migration) -> Result<DriverResult, Error> {
        match (m.steps.get(&Direction::Up), m.steps.get(&Direction::Change)) {
            (Some(_), Some(_)) => Err(Error::MalformedMigration),
            _ => Ok(DriverResult::NothingToDo),
        }?;
        Ok(DriverResult::NothingToDo)
    }
    fn unapply(&mut self, _: &Migration) -> Result<DriverResult, Error> {
        Ok(DriverResult::NothingToDo)
    }
}

impl crate::StepDriver for Driver {
    fn run(&mut self, ms: &MigrationStep) -> Result<DriverResult, Error> {
        match ms.source.as_str() {
            "NOTHING_TO_DO" => Ok(DriverResult::NothingToDo),
            "SUCCESS" => Ok(DriverResult::Success),
            "MIGRATION_RUNNER_MISMATCH" => Ok(DriverResult::MigrationRunnerMismatch),
            _ => panic!(
                "succeed or fail driver can't handle the source {}",
                ms.source
            ),
        }
    }
}
