use crate::Direction;

use super::{
    driver_from_config, Error, Migration, MigrationList, MigrationResult, MigrationResultTuple,
    MigrationState, MigrationStateTuple, MigrationStorage,
};
use itertools::Itertools;

pub struct Engine {}

impl Engine {
    pub fn diff(
        src: &mut dyn MigrationList,
        dest: &mut dyn MigrationStorage,
    ) -> Result<impl Iterator<Item = MigrationStateTuple>, Error> {
        let uniq_fn = |m: &Migration| m.date_time;
        let tuple_uniq_fn = |m: &MigrationStateTuple| m.1.date_time;
        let cmp_fn = |l: &Migration, r: &Migration| l.cmp(r);
        let tuple_cmp_fn =
            |l: &MigrationStateTuple, r: &MigrationStateTuple| l.1.date_time.cmp(&r.1.date_time);
        let mut_cmp_fn = |l: &mut Migration, r: &mut Migration| l.cmp(&r);

        let src_migrations = src.all()?.sorted_by(cmp_fn).unique_by(uniq_fn);
        let dest_migrations = dest.all()?.sorted_by(cmp_fn).unique_by(uniq_fn);

        // Applied migrations appear in both sets
        let applied =
            iter_set::union_by(src_migrations.clone(), dest_migrations.clone(), mut_cmp_fn)
                .map(|m| (MigrationState::Applied, m));
        // Pending migrations appear only in known, but not applied
        let pending =
            iter_set::difference_by(src_migrations.clone(), dest_migrations.clone(), mut_cmp_fn)
                .map(|m| (MigrationState::Pending, m));
        // Orphan migrations appear only in applied, but not in known
        let orphan =
            iter_set::difference_by(dest_migrations.clone(), src_migrations.clone(), mut_cmp_fn)
                .map(|m| (MigrationState::Orphaned, m));

        Ok(orphan
            .chain(pending)
            .chain(applied)
            .sorted_by(tuple_cmp_fn)
            .unique_by(tuple_uniq_fn))
    }

    pub fn apply(
        config: crate::config::Configuration,
        src: &mut dyn MigrationList,
        dest: &mut dyn MigrationStorage,
        _work_filter: Option<Vec<&Direction>>,
    ) -> Result<impl Iterator<Item = MigrationResultTuple>, Error> {
        let work_list = Engine::diff(src, dest)?;
        let c = config.clone();
        let mut failed_at: Option<Migration> = None;
        Ok(work_list.map(move |(state, migration)| {
            log::info!("=== Examining Migration {:?} ===", migration);
            match (failed_at.as_ref(), state) {
                // Match clause guards that we always stop if there's a "Some" error
                // we tracked earlier
                (Some(_), _) => (MigrationResult::SkippedDueToEarlierError, migration),
                (None, MigrationState::Pending) => {
                    match driver_from_config(&c, &migration.configuration_name) {
                        Ok(mut driver) => match driver.apply(&migration) {
                            Ok(_) => (MigrationResult::Success, migration),
                            Err(e) => {
                                failed_at = Some(migration.clone());
                                (
                                    MigrationResult::Failure {
                                        reason: format!("{:?}", e),
                                    },
                                    migration,
                                )
                            }
                        },
                        Err(e) => {
                            log::error!("Error getting runner from config {:?}", e);
                            (
                                MigrationResult::Failure {
                                    reason: format!("{:?}", e),
                                },
                                migration,
                            )
                        }
                    }
                }
                _ => {
                    todo!("boom")
                }
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        Direction, InMemoryMigrations, Migration, MigrationStateTuple, MigrationStep,
        MigrationStorage, TIMESTAMP_FORMAT_STR,
    };
    use super::*;
    use crate::config;
    use crate::config::Configuration;
    use crate::mysql::MySQL;
    use log::trace;
    use rand::Rng;
    use std::path::PathBuf;
    use test_case::test_case;

    fn config() -> Configuration {
        Configuration::load_from_str(config::DEFAULT_CONFIG).unwrap()
    }

    fn fixture() -> Vec<Migration> {
        vec![Migration {
            date_time: chrono::NaiveDateTime::parse_from_str(
                "20210511204055",
                TIMESTAMP_FORMAT_STR,
            )
            .unwrap(),
            steps: std::array::IntoIter::new([
                (
                    Direction::Up,
                    MigrationStep {
                        path: PathBuf::from("built/in/migration"),
                        source: String::from(include_str!(
                            "migrations/bootstrap_mysql_migration_storage.sql"
                        )),
                    },
                ),
                (
                    Direction::Down,
                    MigrationStep {
                        path: PathBuf::from("built/in/migration"),
                        source: String::from("DROP DATABASE IF EXISTS `{{database_name}}`;"),
                    },
                ),
            ])
            .collect(),
            flags: vec![],
            built_in: false,
            configuration_name: String::from("my-mysql-db"),
        }]
    }

    fn all_success_fixture() -> Box<dyn MigrationList> {
        let str = format!("mitre_test_db_{}", rand::thread_rng().gen::<u32>());
        let migrations = vec![
            Migration {
                date_time: chrono::NaiveDateTime::parse_from_str(
                    "20211108145400",
                    TIMESTAMP_FORMAT_STR,
                )
                .unwrap(),
                steps: std::array::IntoIter::new([
                    (
                        Direction::Up,
                        MigrationStep {
                            path: PathBuf::from("built/in/migration/one"),
                            source: String::from(format!("CREATE DATABASE {};", str)),
                        },
                    ),
                    (
                        Direction::Down,
                        MigrationStep {
                            path: PathBuf::from("built/in/migration/one"),
                            source: String::from(format!("DROP DATABASE {};", str)),
                        },
                    ),
                ])
                .collect(),
                flags: vec![],
                built_in: false,
                configuration_name: String::from("my-mysql-db"),
            },
            Migration {
                date_time: chrono::NaiveDateTime::parse_from_str(
                    "20211108145401",
                    TIMESTAMP_FORMAT_STR,
                )
                .unwrap(),
                steps: std::array::IntoIter::new([
                    (
                        Direction::Up,
                        MigrationStep {
                            path: PathBuf::from("built/in/migration/two"),
                            source: String::from(format!(
                                "CREATE TABLE {}.bar (column_one VARCHAR(255) NOT NULL);",
                                str
                            )),
                        },
                    ),
                    (
                        Direction::Down,
                        MigrationStep {
                            path: PathBuf::from("built/in/migration/two"),
                            source: String::from(format!("DROP TABLE {}.bar;", str)),
                        },
                    ),
                ])
                .collect(),
                flags: vec![],
                built_in: false,
                configuration_name: String::from("my-mysql-db"),
            },
        ];
        let mut imm = InMemoryMigrations::new();
        for migration in migrations {
            imm.add(migration).unwrap();
        }
        Box::new(imm)
    }

    fn empty_migration_storage() -> Box<dyn MigrationStorage> {
        Box::new(InMemoryMigrations::new())
    }

    fn empty_migration_list() -> Box<dyn MigrationStorage> {
        Box::new(InMemoryMigrations::new())
    }

    #[cfg(feature = "runner_mysql")]
    fn mysql_migration_storage(c: config::Configuration) -> Box<dyn MigrationStorage> {
        Box::new(MySQL::new(c.get("mitre").unwrap().clone()).unwrap())
    }

    fn non_empty_migration_list() -> Box<dyn MigrationStorage> {
        let mut imms = empty_migration_list();
        for migration in fixture().iter() {
            trace!("Added migration {}", migration.date_time);
            if let Err(e) = imms.add(migration.clone()) {
                panic!("failed to add a migration to the test fixture: {:?}", e);
            }
        }
        imms
    }

    #[test]
    fn test_diff_lists_unknown_dest_migrations_as_pending() -> Result<(), String> {
        match Engine::diff(
            &mut (*non_empty_migration_list()),
            &mut (*empty_migration_list()),
        ) {
            Ok(r) => {
                let r_vec = r.collect::<Vec<MigrationStateTuple>>();
                assert_eq!(r_vec.len(), fixture().len());
                for (state, _migration) in r_vec {
                    assert_eq!(MigrationState::Pending, state);
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    #[test]
    fn test_diff_lists_unknown_src_migrations_as_orphaned() -> Result<(), String> {
        match Engine::diff(empty_migration_list(), non_empty_migration_list()) {
            Ok(r) => {
                let r_vec = r.collect::<Vec<MigrationStateTuple>>();
                assert_eq!(r_vec.len(), fixture().len());
                for (state, _migration) in r_vec {
                    assert_eq!(MigrationState::Orphaned, state);
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    #[test]
    fn test_diff_lists_all_known_in_src_dest_migrations_as_applied() -> Result<(), String> {
        match Engine::diff(non_empty_migration_list(), non_empty_migration_list()) {
            Ok(r) => {
                let r_vec = r.collect::<Vec<MigrationStateTuple>>();
                assert_eq!(r_vec.len(), fixture().len());
                for (state, _migration) in r_vec {
                    assert_eq!(MigrationState::Applied, state);
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    #[test_case(config(), empty_migration_list(), empty_migration_storage() ; "with an in-memory store")]
    #[cfg(feature = "runner_mysql")]
    #[test_case(config(), empty_migration_list(), mysql_migration_storage(config()) ; "with an mysql store")]
    fn test_empty_stores_with_no_config_apply_uniformly(
        config: Configuration,
        src: &dyn MigrationList,
        mut dest: &dyn MigrationStorage,
    ) -> Result<(), String> {
        // integration fixtures may have external state, reset them noisily
        dest.reset().unwrap();
        // let c = config::load
        match Engine::apply(&config, src, dest, None {}) {
            Ok(r) => {
                for result in r {
                    println!("{:?}", result)
                }
                Ok(())
            }
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    #[test_case(config(), all_success_fixture(), empty_migration_storage() ; "with an in-memory store")]
    #[cfg(feature = "runner_mysql")]
    #[test_case(config(), all_success_fixture(), mysql_migration_storage(config()) ; "with an mysql store")]
    fn test_all_success_fixture_stores_all_during_apply_uniformly(
        config: Configuration,
        src: &dyn MigrationList,
        mut dest: &dyn MigrationStorage,
    ) -> Result<(), String> {
        // integration fixtures may have external state, reset them noisily
        dest.reset().unwrap();
        // let c = config::load
        match Engine::apply(&config, src, &dest, None {}) {
            Ok(r) => {
                // Every migration should report as success
                for (result, _migration) in r {
                    assert_eq!(result, MigrationResult::Success);
                }
                // Every migration should be stored with applied status in the store
                match dest.all() {
                    Ok(iter) => Ok(()),
                    Err(e) => Err(format!("{:?}", e)),
                }
            }
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    // #[test_case(config(), all_success_fixture(), empty_migration_storage() ; "with an in-memory store")]
    // #[cfg(feature = "runner_mysql")]
    // fn test_engine_stops_applying_after_first_failure(
    //     config: Configuration,
    //     src: &dyn MigrationList,
    //     mut dest: &dyn MigrationStorage,
    // ) -> Result<(), String> {
    //     // TODO: Ahoy
    //     // Test that we stop processing on the first error and
    //     // return a Skipped(reason prior failure) error for every
    //     // subsequent one.
    //     Ok(())
    // }
}
