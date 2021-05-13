use super::{Direction, Error, Migration};

pub enum DriverResult {
    // Sucessfully applied the migration
    Success,
    // There's nothing to do at all
    NothingToDo,
    // This driver does not know how to run migrations of type X
    MigrationRunnerMismatch,
}

// A Driver that has no work to do must report NothingToDo (e.g trying
// to unapply an irreversible migration, because this is a successful
// no-op)
pub trait Driver {
    // Apply will take a Migration and run any
    fn apply(&mut self, _: &Migration) -> Result<DriverResult, Error>;
    fn unapply(&mut self, _: &Migration) -> Result<DriverResult, Error>;
}

pub struct NoopDriver {}

impl Driver for NoopDriver {
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

pub struct SucceedOrFailDriver {}
impl Driver for SucceedOrFailDriver {
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

// Test that all drivers raise malformed migration when the migration
// has up, and change parts.
#[cfg(test)]
mod test {

    use super::super::{Direction, Error, Migration, MigrationStep, TIMESTAMP_FORMAT_STR};
    use super::*;
    use concat_idents::concat_idents;
    use std::path::PathBuf;

    #[cfg(test)]
    fn malformed_migration() -> Migration {
        Migration {
            date_time: chrono::NaiveDateTime::parse_from_str(
                "20210512201455",
                TIMESTAMP_FORMAT_STR,
            )
            .unwrap(),
            steps: std::array::IntoIter::new([
                (
                    Direction::Up,
                    MigrationStep {
                        path: PathBuf::from("built/in/migration"),
                        source: String::from("Success"),
                    },
                ),
                (
                    Direction::Change,
                    MigrationStep {
                        path: PathBuf::from("built/in/migration"),
                        source: String::from("Success"),
                    },
                ),
                (
                    Direction::Down,
                    MigrationStep {
                        path: PathBuf::from("built/in/migration"),
                        source: String::from("Success"),
                    },
                ),
            ])
            .collect(),
            flags: vec![],
            built_in: false,
            configuration_name: String::from("anything"),
        }
    }

    macro_rules! test_driver {
        ($driver_name:ident, $setup:expr) => {
            concat_idents!(
                test_name = "test_",
                $driver_name,
                "_driver_raises_malformed_error_when_migration_has_both_change_and_up_steps",
                {
                    #[test]
                    fn test_name() -> Result<(), String> {
                        let mut driver = $setup;
                        match driver.apply(&malformed_migration()) {
                            Ok(_) => Err(format!("engine did not report malformed error",)),
                            Err(e) => match e {
                                Error::MalformedMigration => Ok(()),
                                _ => Err(format!("Engine returned {:?}, unexpected", e)),
                            },
                        }?;
                        Ok(())
                    }
                }
            );
        };
    }

    // rust-analyzer bug here (?)
    // https://github.com/rust-analyzer/rust-analyzer/issues/6747
    test_driver!(noop_driver, { NoopDriver {} });
    test_driver!(succeed_or_fail_driver, { SucceedOrFailDriver {} });

    // test that apply tries "up", and falls-back to "change" on apply

    // test that unapply runs the "down" step, or returns nothing to do if there's no
    // down step.
}
