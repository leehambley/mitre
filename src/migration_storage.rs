use super::{Configuration, Error, Migration, MigrationList, MySQL};

pub fn from_config(c: &Configuration) -> Result<impl MigrationStorage, Error> {
    if let Some(config) = c.get("mitre") {
        if config._runner.to_lowercase() == crate::reserved::MARIA_DB.to_lowercase() {
            let storage = MySQL::new(config.clone())?;
            Ok(storage)
        } else {
            Err(Error::UnsupportedDriverSpecified)
        }
    } else {
        Err(Error::NoMitreConfigProvided)
    }
}

pub trait MigrationStorage: MigrationList {
    #[cfg(test)]
    fn reset(&mut self) -> Result<(), Error>;

    fn add(&mut self, _: Migration) -> Result<(), Error>;
    fn remove(&mut self, _: Migration) -> Result<(), Error>;
}

#[cfg(test)]
mod tests {

    use super::super::{Direction, MigrationStep, MigrationSteps, MigrationStorage};
    use super::*;
    use crate::{
        migrations::FORMAT_STR, reserved, runner::Configuration as RunnerConfiguration,
        InMemoryMigrations, MySQL,
    };
    use std::{array::IntoIter, collections::HashMap, iter::FromIterator, path::PathBuf};

    fn test_mysql_storage_configuration() -> RunnerConfiguration {
        RunnerConfiguration {
            _runner: String::from("mysql"),
            database_number: None {},
            database: Some(String::from("mitre_test")),
            index: None {},
            ip_or_hostname: Some(String::from("127.0.0.1")),
            password: Some(String::from("example")),
            port: None {},
            username: Some(String::from("root")),
        }
    }

    fn mysql_migration_storage() -> Box<dyn MigrationStorage> {
        let mut mysql = MySQL::new(test_mysql_storage_configuration()).unwrap();
        mysql.reset().unwrap(); // boom
        Box::new(mysql)
    }

    fn in_memory_migration_storage() -> Box<dyn MigrationStorage> {
        Box::new(InMemoryMigrations::new())
    }

    fn migration_fixture() -> Vec<Migration> {
        vec![Migration {
            date_time: chrono::NaiveDateTime::parse_from_str("20210503213400", FORMAT_STR).unwrap(),
            steps: MigrationSteps::from_iter(IntoIter::new([
                (
                    Direction::Up,
                    MigrationStep {
                        path: PathBuf::from("/foo/up.sql"),
                        source: String::from("CREATE TABLE foo"),
                    },
                ),
                (
                    Direction::Down,
                    MigrationStep {
                        path: PathBuf::from("/foo/down.sql"),
                        source: String::from("DROP TABLE foo"),
                    },
                ),
            ])),
            flags: reserved::flags_from_str_flags(&String::from("data,long,risky")),
            built_in: false,
            configuration_name: String::from("example"),
        }]
    }

    #[test]
    // Returns a tuple of implementation name and the test error, if any
    fn test_all_known_implementations() -> Result<(), (String, String)> {
        let mut impls = HashMap::<String, Box<dyn MigrationStorage>>::from_iter(IntoIter::new([
            (String::from("InMemory"), in_memory_migration_storage()),
            (String::from("MySQL"), mysql_migration_storage()),
        ]));
        for (name, implementation) in &mut impls {
            match lists_what_it_stores(implementation) {
                Err(e) => return Err((name.clone(), e)),
                _ => {}
            };
        }
        Ok(())
    }
    fn lists_what_it_stores(ms: &mut Box<dyn MigrationStorage>) -> Result<(), String> {
        for migration in migration_fixture() {
            match ms.add(migration) {
                Err(e) => return Err(format!("error: {:#?}", e)),
                _ => {}
            };
        }
        Ok(())
    }
}
