use super::{Error, Migration, MigrationStep, MySQL};

#[cfg(test)]
pub mod noop;
#[cfg(test)]
pub mod succeed_or_fail;

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
    fn name() -> &'static str
    where
        Self: Sized;
}
/// [`Driver`] is trait which can be implemented for one or more
/// technologies for which we manage migrations/schema changes.
///
/// Drivers which cannot unapply migrations (e.g HTTP) must report
/// [`NothingToDo`] because this is a *successful* operation, but
/// also a no-op.
pub trait Driver: NamedDriver {
    // Apply will take a Migration and run any
    fn apply(&mut self, _: &Migration) -> Result<DriverResult, Error>;
    fn unapply(&mut self, _: &Migration) -> Result<DriverResult, Error>;
}

// Subtrait for convenience about a driver that only runs a single step.
// mostly used to keep implementations tidy and reusable.
// The trait doesn't make sense alone, so only allow this to be a convenience
// for existing drivers. (e.g noop::Driver doesn't need this)
pub trait StepDriver: Driver {
    fn run(&mut self, _: &MigrationStep) -> Result<DriverResult, Error>;
}

// Given a config YAML such as:
// ---
// es-mysql: &es-mysql
//   _driver: mysql
//   database: mitre_test_fixture_four
//   ip_or_hostname: 127.0.0.1
//   password: example
//   port: 3306
//   username: root
// This function takes the parsed configuration
// and a name (e.g es-mysql) and will use the
// configuration to instantiate a new MySQL driver
// and return it boxed.
//
// Calling this function with an invalid configuration
// name (e.g es-foo) will return NoSuchConfiguration.
//
// Other errors can be returned when the value of _driver
// is incorrect (e.g an unsupported runner due to conditional
// features, or typos)
//
// Some drivers may return an error if all configuration is
// correct but the server is not responding, or credentials
// are incorrect.
pub fn from_config(
    c: &crate::config::Configuration,
    config_name: &str,
) -> Result<Box<dyn Driver>, Error> {
    log::debug!(
        "Searching for runner {:?} in configured runners {:?}",
        config_name,
        c.configured_drivers.keys(),
    );

    let rc = c
        .configured_drivers
        .get(config_name)
        .ok_or(Error::NoSuchConfiguration {
            configuration_name: config_name.to_string(),
        })?;

    #[cfg(feature = "runner_mysql")]
    log::trace!(
        "comparing {} to {} and {}",
        rc._driver.to_lowercase(),
        crate::reserved::MYSQL.to_lowercase(),
        crate::reserved::MARIA_DB.to_lowercase()
    );
    if rc._driver.to_lowercase() == crate::reserved::MYSQL.to_lowercase()
        || rc._driver.to_lowercase() == crate::reserved::MARIA_DB.to_lowercase()
    {
        log::info!("matched, returning a MySQL driver");
        return Ok(Box::new(MySQL::new(rc.clone())?));
    }
    log::error!(
        "There seems to be no avaiable (not compiled, not enabled) runner for {} (runner: {})",
        config_name,
        rc._driver,
    );
    Err(Error::UnsupportedDriverSpecified)
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
    fn malformed_migration(runner_name: &str) -> Migration {
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
            configuration_name: String::from(runner_name),
        }
    }

    fn migration_runner_mismatch_migration(runner_name: &str) -> Migration {
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
            configuration_name: String::from(runner_name),
        }
    }

    macro_rules! test_driver {
        ($runner_name:ident, $setup:expr) => {
            // Rust-Analyzer look-up bug makes us use a custom import name
            // to avoid it incorrectly resolving the built-in concat_idents.
            // https://github.com/rust-analyzer/rust-analyzer/issues/8828
            concat_idents_from_crate!(
                test_name = "test_",
                $runner_name,
                "_driver_raises_malformed_error_when_migration_has_both_change_and_up_steps",
                {
                    #[test]
                    fn test_name() -> Result<(), String> {
                        let mut driver = $setup;
                        match driver.apply(&malformed_migration(stringify!($runner_name))) {
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
                $runner_name,
                "_raises_migration_runner_mismatch",
                {
                    #[test]
                    fn test_name() -> Result<(), String> {
                        let mut driver = $setup;
                        match driver.apply(&migration_runner_mismatch_migration(stringify!(
                            $runner_name
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
    test_driver!(noop, { noop::Driver {} });
    test_driver!(succeed_or_fail, { succeed_or_fail::Driver {} });
    test_driver!(mysql, { succeed_or_fail::Driver {} });

    // test that apply tries "up", and falls-back to "change" on apply

    // test that unapply runs the "down" step, or returns nothing to do if there's no
    // down step.
}
