use super::{Direction, Error, Migration, MigrationStep};

pub enum DriverResult {
    // Sucessfully applied the migration
    Success,
    // There's nothing to do at all
    NothingToDo,
    // This driver does not know how to run migrations of type X
    MigrationRunnerMismatch,
}

// All drivers are required to define a name, and
// it must be matched in the configuration
pub trait NamedDriver {
    fn name() -> &'static str;
}
// A Driver that has no work to do must report NothingToDo (e.g trying
// to unapply an irreversible migration, because this is a successful
// no-op)
pub trait Driver: NamedDriver {
    // Apply will take a Migration and run any
    fn apply(&mut self, _: &Migration) -> Result<DriverResult, Error>;
    fn unapply(&mut self, _: &Migration) -> Result<DriverResult, Error>;
}

// Subtrait for convenience about a driver that only runs a single step.
// mostly used to keep implementations tidy and reusable.
// The trait doesn't make sense alone, so only allow this to be a convenience
// for existing drivers. (e.g NoopDriver doesn't need this)
pub trait StepDriver: Driver {
    fn run(&mut self, _: &MigrationStep) -> Result<DriverResult, Error>;
}

pub struct NoopDriver {}

impl NamedDriver for NoopDriver {
    fn name() -> &'static str {
        "noop"
    }
}

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

impl NamedDriver for SucceedOrFailDriver {
    fn name() -> &'static str {
        "succeed_or_fail"
    }
}

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

impl StepDriver for SucceedOrFailDriver {
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

// Test that all drivers raise malformed migration when the migration
// has up, and change parts.
#[cfg(test)]
mod test {

    use super::super::{Direction, Error, Migration, MigrationStep, TIMESTAMP_FORMAT_STR};
    use super::*;
    use std::path::PathBuf;

    use concat_idents::concat_idents as concat_idents_from_crate;
    use std::stringify;

    #[cfg(test)]
    fn malformed_migration(driver_name: &str) -> Migration {
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
                        source: String::from("SUCCESS"),
                    },
                ),
                (
                    Direction::Change,
                    MigrationStep {
                        path: PathBuf::from("built/in/migration"),
                        source: String::from("SUCCESS"),
                    },
                ),
                (
                    Direction::Down,
                    MigrationStep {
                        path: PathBuf::from("built/in/migration"),
                        source: String::from("SUCCESS"),
                    },
                ),
            ])
            .collect(),
            flags: vec![],
            built_in: false,
            configuration_name: String::from(driver_name),
        }
    }

    fn migration_runner_mismatch_migration(driver_name: &str) -> Migration {
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
                        source: String::from("MIGRATION_RUNNER_MISMATCH"),
                    },
                ),
                (
                    Direction::Change,
                    MigrationStep {
                        path: PathBuf::from("built/in/migration"),
                        source: String::from("MIGRATION_RUNNER_MISMATCH"),
                    },
                ),
                (
                    Direction::Down,
                    MigrationStep {
                        path: PathBuf::from("built/in/migration"),
                        source: String::from("MIGRATION_RUNNER_MISMATCH"),
                    },
                ),
            ])
            .collect(),
            flags: vec![],
            built_in: false,
            configuration_name: String::from(driver_name),
        }
    }

    macro_rules! test_driver {
        ($driver_name:ident, $setup:expr) => {
            // Rust-Analyzer look-up bug makes us use a custom import name
            // to avoid it incorrectly resolving the built-in concat_idents.
            // https://github.com/rust-analyzer/rust-analyzer/issues/8828
            concat_idents_from_crate!(
                test_name = "test_",
                $driver_name,
                "_driver_raises_malformed_error_when_migration_has_both_change_and_up_steps",
                {
                    #[test]
                    fn test_name() -> Result<(), String> {
                        let mut driver = $setup;
                        match driver.apply(&malformed_migration(stringify!($driver_name))) {
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
            concat_idents_from_crate!(
                test_name = "test_",
                $driver_name,
                "_raises_migration_runner_mismatch",
                {
                    #[test]
                    fn test_name() -> Result<(), String> {
                        let mut driver = $setup;
                        match driver.apply(&migration_runner_mismatch_migration(stringify!(
                            $driver_name
                        ))) {
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

    // The first :ident must
    test_driver!(noop, { NoopDriver {} });
    test_driver!(succeed_or_fail, { SucceedOrFailDriver {} });
    test_driver!(mysql, { SucceedOrFailDriver {} });

    // test that apply tries "up", and falls-back to "change" on apply

    // test that unapply runs the "down" step, or returns nothing to do if there's no
    // down step.
}
