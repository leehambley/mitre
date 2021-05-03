use super::{Error, Migration, MigrationList};

pub trait MigrationStorage: MigrationList {
    #[cfg(test)]
    fn reset(&mut self) -> Result<(), Error>;

    fn add(&mut self, _: Migration) -> Result<(), Error>;
    fn remove(&mut self, _: Migration) -> Result<(), Error>;
}

#[cfg(test)]
mod tests {

    use super::super::{Direction, MigrationStep, MigrationSteps};
    use super::*;
    use crate::{migrations::FORMAT_STR, reserved, InMemoryMigrations};
    use std::{array::IntoIter, collections::HashMap, iter::FromIterator, path::PathBuf};

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
        let mut impls = HashMap::<String, MigrationStorage>::from_iter(IntoIter::new([(
            String::from("InMemory"),
            InMemoryMigrations::new(),
        )]));
        for (name, implementation) in &mut impls {
            match lists_what_it_stores(implementation) {
                Err(e) => return Err((name.clone(), e)),
                _ => {}
            };
        }
        Ok(())
    }
    fn lists_what_it_stores(ms: &mut impl MigrationStorage) -> Result<(), String> {
        for migration in migration_fixture() {
            ms.add(migration);
        }
        Ok(())
    }
}
