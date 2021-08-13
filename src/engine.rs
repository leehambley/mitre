use crate::Direction;

use super::{
    driver_from_config, runner_from_config, Error, Migration, MigrationList, MigrationResult,
    MigrationResultTuple, MigrationState, MigrationStateTuple, MigrationStorage,
};
use itertools::Itertools;

pub struct Engine {}

impl Engine {
    pub fn diff(
        mut src: impl MigrationList,
        mut dest: impl MigrationStorage,
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

    pub fn apply<'a>(
        config: &'a crate::config::Configuration,
        src: impl MigrationList + 'a,
        dest: impl MigrationStorage + 'a,
        _work_filter: Option<Vec<&Direction>>,
    ) -> Result<impl Iterator<Item = MigrationResultTuple> + 'a, Error> {
        let work_list = Engine::diff(src, dest)?;
        let c = config.clone();
        Ok(work_list.map(move |(state, migration)| {
            log::debug!("checking migration {:?}", migration);
            match state {
                MigrationState::Pending => {
                    match runner_from_config(&c, &migration.configuration_name) {
                        Ok(_) => (MigrationResult::Success, migration),
                        // Ok(boxed_runner) => match boxed_runner.apply(migration) {
                        //     Ok(_) => (MigrationResult::Success, migration),
                        //     Err(e) => (
                        //         MigrationResult::Failure {
                        //             reason: format!("{}", e),
                        //         },
                        //         migration,
                        //     ),
                        // },
                        Err(e) => {
                            log::error!("Error getting runner from config {:?}", e);
                            (
                                MigrationResult::Failure {
                                    reason: format!("{}", e),
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
    use log::trace;
    use std::path::PathBuf;

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
            configuration_name: String::from("anything"),
        }]
    }

    fn empty_migration_list() -> impl MigrationStorage {
        InMemoryMigrations::new()
    }

    fn non_empty_migration_list() -> impl MigrationStorage {
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
        match Engine::diff(non_empty_migration_list(), empty_migration_list()) {
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
}
