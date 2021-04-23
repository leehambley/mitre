use super::MARIADB_MIGRATION_STATE_TABLE_NAME;
use crate::config::{Configuration, RunnerConfiguration};
use crate::migrations::{from_stored_migration, Direction, Migration, MigrationStep};
use crate::runner::{BoxedRunner, MigrationResult, MigrationState, RunnersHashMap};
use crate::state_store::{
    Error as StateStoreError, MigrationResultTuple, MigrationStateTuple, StateStore,
};
use itertools::Itertools;
use mysql::{prelude::Queryable, Conn, OptsBuilder};

pub struct MariaDb {
    conn: Conn,
    config: Configuration,
    runner_config: RunnerConfiguration,
    runners: RunnersHashMap,
}

impl MariaDb {
    pub fn select_db(&mut self) -> bool {
        match &self.runner_config.database {
            Some(database) => {
                trace!("select_db database name is {}", database);
                match &self.conn.select_db(&database) {
                    true => {
                        trace!("select_db successfully using {}", database);
                        true
                    }
                    false => {
                        trace!("could not switch to {} (may not exist yet?)", database);
                        false
                    }
                }
            }
            None => {
                trace!("select_db no database name provided");
                false
            }
        }
    }

    fn unapply_migration_step(
        &mut self,
        m: Migration,
        ms: &MigrationStep,
    ) -> (MigrationResult, Migration) {
        let start = std::time::Instant::now();

        match self.get_runner(&m) {
            Err(e) => (
                MigrationResult::Failure {
                    reason: format!("{:?}", e),
                },
                m,
            ),
            Ok(runner) => match runner.apply(ms) {
                Ok(_) => match self.remove_success_record(&m, ms, start.elapsed()) {
                    Ok(_) => (MigrationResult::Success, m),
                    Err(e) => (
                        MigrationResult::Failure {
                            reason: e.to_string(),
                        },
                        m,
                    ),
                },
                Err(e) => (
                    MigrationResult::Failure {
                        reason: e.to_string(),
                    },
                    m,
                ),
            },
        }
    }

    fn apply_migration_step(
        &mut self,
        m: Migration,
        ms: &MigrationStep,
    ) -> (MigrationResult, Migration) {
        let start = std::time::Instant::now();

        match self.get_runner(&m) {
            Err(e) => (
                MigrationResult::Failure {
                    reason: format!("{:?}", e),
                },
                m,
            ),
            Ok(runner) => match runner.apply(ms) {
                Ok(_) => match self.record_success(&m, start.elapsed()) {
                    Ok(_) => (MigrationResult::Success, m),
                    Err(e) => (
                        MigrationResult::Failure {
                            reason: e.to_string(),
                        },
                        m,
                    ),
                },
                Err(e) => (
                    MigrationResult::Failure {
                        reason: e.to_string(),
                    },
                    m,
                ),
            },
        }
    }

    fn remove_success_record(
        &mut self,
        m: &Migration,
        _ms: &MigrationStep,
        _: std::time::Duration,
    ) -> Result<(), StateStoreError> {
        if !self.select_db() {
            return Err(StateStoreError::CouldNotRecordSuccess {
                reason: String::from(
                    "could not select db, that means the bootstrap migrations are not run",
                ),
            });
        }

        match self.conn.prep(format!("DELETE FROM {} WHERE version = ? LIMIT 1", MARIADB_MIGRATION_STATE_TABLE_NAME)) {
          Ok(stmt) => match self.conn.exec_iter(stmt, (m.date_time,)) {
            Ok(query_results) => match query_results.affected_rows() { // TODO: this also contains warnings, could be cool
              1 => Ok(()),
              _ => panic!("error removing success record during down, expected to affect exactly one row")
            },
            Err(e) => panic!("error running query {:?}", e),
          },
          Err(e) => panic!("coult not prepare statement {:?}", e)
        }
    }

    /// We do not record failures,
    fn record_success(
        &mut self,
        m: &Migration,
        d: std::time::Duration,
    ) -> Result<(), StateStoreError> {
        if !self.select_db() {
            return Err(StateStoreError::CouldNotRecordSuccess {
                reason: String::from(
                    "could not select db, that means the bootstrap migrations are not run",
                ),
            });
        }

        match self.conn.prep(format!("INSERT INTO {} (`version`, `up`, `down`, `change`, `applied_at_utc`, `apply_time_ms`, `built_in`, `configuration_name`, `flags`) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?);", MARIADB_MIGRATION_STATE_TABLE_NAME)) {
        Ok(stmt) => match self.conn.exec_iter(stmt, (
            m.date_time,
            m.steps.get(&Direction::Up).map(|ms| format!("{:?}", ms.source )),
            m.steps.get(&Direction::Down).map(|ms| format!("{:?}", ms.source )),
            m.steps.get(&Direction::Change).map(|ms| format!("{:?}", ms.source )),
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            d.as_millis(),
            m.built_in,
            m.configuration_name.clone(),
            m.flags.clone().into_iter().map(|f|f.name).join(","),
          )) {
          Ok(query_results) => match query_results.affected_rows() { // TODO: this also contains warnings, could be cool
            1 => Ok(()),
            _ => panic!("error recording success, expected to affect exactly one row")
          },
          Err(e) => panic!("error running query {:?}", e),
        },
        Err(e) => panic!("coult not prepare statement {:?}", e)
      }
    }
}

impl StateStore for MariaDb {
    #[cfg(test)]
    fn reset(&mut self) -> Result<(), StateStoreError> {
        match self.config.configured_runners.get("mitre") {
            Some(mitre_config) => match crate::runner::from_config(mitre_config) {
                Ok(mut runner) => {
                    let drop_db = MigrationStep {
                        path: std::path::PathBuf::from("./reset_state_store"),
                        source: String::from(
                            "DROP DATABASE IF EXISTS {{mariadb_migration_state_database_name}}",
                        ),
                    };
                    Ok(runner.apply(&drop_db)?)
                }
                Err(e) => {
                    format!("cannot get mitre runner from config: {:?}", e);
                    Err(StateStoreError::CouldNotFindOrCreateRunner)
                }
            },
            None => {
                format!("Cannot get config for mitre");
                Err(StateStoreError::NoMitreConfigProvided)
            }
        }
    }

    fn new_state_store(config: &Configuration) -> Result<MariaDb, StateStoreError> {
        // Ensure this is a proper config for this runner
        let runner_name = String::from(crate::reserved::MARIA_DB).to_lowercase();
        let mariadb_config = match config.get("mitre") {
            None => {
                debug!("no config entry `mitre' found, please check the docs");
                return Err(StateStoreError::NoMitreConfigProvided);
            }
            Some(c) => {
                if c._runner.to_lowercase() == runner_name {
                    c.clone()
                } else {
                    warn!("runner name mismatch, please check the docs and your config");
                    return Err(StateStoreError::RunnerNameMismatch {
                        expected: runner_name,
                        found: c._runner.to_lowercase(),
                    });
                }
            }
        };

        let opts = mysql::Opts::from(
            OptsBuilder::new()
                .ip_or_hostname(mariadb_config.ip_or_hostname.clone())
                .user(mariadb_config.username.clone())
                // NOTE: Do not specify database name here, otherwise we cannot
                // connect until the database exists. Makes it difficult to
                // bootstrap.
                // .db_name(mariadb_config.database.clone())
                .pass(mariadb_config.password.clone()),
        );
        Ok(MariaDb {
            config: config.clone(),
            conn: Conn::new(opts)?,
            runner_config: mariadb_config,
            runners: RunnersHashMap::new(),
        })
    }

    /// Given the set of runner configs on config, this will
    /// try to create a
    fn get_runner(&mut self, m: &Migration) -> Result<&mut BoxedRunner, StateStoreError> {
        // If we have a cached runner miss, let's
        trace!(
            "looking up runner for {} in MariaDB StateStore",
            m.configuration_name
        );
        if self.runners.get(&m.configuration_name).is_none() {
            let runner_config = match self.config.get(&m.configuration_name) {
                Some(rc) => rc,
                None => return Err(StateStoreError::CouldNotFindOrCreateRunner),
            };

            let new_runner = crate::runner::from_config(&runner_config)?;

            match self
                .runners
                .insert(m.configuration_name.clone(), new_runner)
            {
                None => trace!(
                    "clean insert of {} ({}) into runners map, no old value",
                    m.configuration_name,
                    runner_config._runner
                ),
                Some(_) => warn!(
                    "insert of {} ({}) into runners map overwrote a previous value, race condition?",
                    m.configuration_name,
                    runner_config._runner
                ),
            };
        }

        match self.runners.get_mut(&m.configuration_name) {
            Some(r) => Ok(r),
            None => Err(StateStoreError::CouldNotFindOrCreateRunner),
        }
    }

    fn up(
        &mut self,
        migrations: Vec<Migration>,
        dt: Option<chrono::NaiveDateTime>,
    ) -> Result<Vec<MigrationResultTuple>, StateStoreError> {
        let _apply_until = dt.unwrap_or_else(|| chrono::Utc::now().naive_utc());
        let mut stop_applying = false;
        Ok(self
            .diff(migrations)?
            .into_iter()
            .map(|(migration_state, migration)| match migration_state {
                MigrationState::Pending => match (
                    stop_applying,
                    migration.steps.get(&Direction::Change),
                    migration.steps.get(&Direction::Up),
                ) {
                    (true, _, _) => (MigrationResult::SkippedDueToEarlierError, migration),
                    (false, Some(_up_step), Some(_change_step)) => (
                        MigrationResult::Failure {
                            reason: String::from("contains both up and down parts"),
                        },
                        migration,
                    ),
                    (false, Some(up_step), None) => {
                        let (migration_result, migration) =
                            self.apply_migration_step(migration.clone(), up_step);
                        match migration_result {
                            MigrationResult::Failure { reason: _ } => {
                                warn!("migration {:?} failed, will stop applying", migration);
                                stop_applying = true;
                            }
                            _ => {}
                        }
                        (migration_result, migration)
                    }
                    (false, None, Some(change_step)) => {
                        let (migration_result, migration) =
                            self.apply_migration_step(migration.clone(), change_step);
                        match migration_result {
                            MigrationResult::Failure { reason: _ } => {
                                warn!("migration {:?} failed, will stop applying", migration);
                                stop_applying = true;
                            }
                            _ => {}
                        }
                        (migration_result, migration)
                    }
                    (false, None, None) => (MigrationResult::NothingToDo, migration),
                },
                MigrationState::Orphaned => match migration.steps.get(&Direction::Down) {
                    Some(down_step) => self.unapply_migration_step(migration.clone(), down_step),
                    None => (MigrationResult::IrreversibleMigration, migration),
                },
                MigrationState::Applied => (MigrationResult::AlreadyApplied, migration),
            })
            .collect())
    }

    fn down(
        &mut self,
        migrations: Vec<Migration>,
        dt: Option<chrono::NaiveDateTime>,
    ) -> Result<Vec<MigrationResultTuple>, StateStoreError> {
        let _unapply_after = dt.unwrap_or_else(|| chrono::Utc::now().naive_utc());
        Ok(self
            .diff(migrations)?
            .into_iter()
            .rev()
            .map(
                |(migration_state, migration)| -> (MigrationResult, Migration) {
                    match migration_state {
                        MigrationState::Applied | MigrationState::Orphaned => {
                            match migration.steps.get(&Direction::Down) {
                                Some(down_step) => {
                                    self.unapply_migration_step(migration.clone(), down_step)
                                }
                                None => (MigrationResult::IrreversibleMigration, migration),
                            }
                        }
                        MigrationState::Pending => (MigrationResult::NothingToDo, migration),
                    }
                },
            )
            .collect())
    }

    fn diff(
        &mut self,
        migrations: Vec<Migration>,
    ) -> Result<Vec<MigrationStateTuple>, StateStoreError> {
        // Try and select the DB here, don't worry about the result
        // a valid result for diff is "no database, even, so no data"
        // selectdb is used other places where we *require* a positive result.
        self.select_db();

        let database = match &self.runner_config.database {
            Some(database) => Ok(database),
            None => Err(StateStoreError::NoStateStoreDatabaseNameProvided),
        }?;

        let schema_exists = self.conn.exec_first::<bool, _, _>(
    "SELECT EXISTS(SELECT SCHEMA_NAME FROM INFORMATION_SCHEMA.SCHEMATA WHERE SCHEMA_NAME = ?)",
    (database,)
  )?;

        match schema_exists {
            Some(schema_exists) => {
                trace!("state store schema found? {}", schema_exists);
                if !schema_exists {
                    return Ok(migrations
                        .into_iter()
                        .map(|m| (MigrationState::Pending, m))
                        .collect());
                }
            }
            None => {
                return Err(StateStoreError::ErrorReadingMigrationState);
            }
        }

        // Same story for the table when diffing, we don't want to run any migrations, so
        // we simply say, if the table doesn't exist, then we answer that all migrations (incl. built-in)
        // _must_ be un-run as far as we know.
        if let Some(table_exists) = self.conn.exec_first::<bool, _, _>(
            "SELECT EXISTS( SELECT * FROM information_schema.tables WHERE table_schema = ? AND table_name = ? );",
              (database, MARIADB_MIGRATION_STATE_TABLE_NAME), //trailing comma makes this a tuple
            )? {
          if !table_exists {
              Ok(migrations.into_iter().map(|m| (MigrationState::Pending, m)).collect())
          } else {

            // Comparator functions for sorting the migrations, and de-duping them
            let uniq_fn = |m: &Migration| m.date_time;
            let tuple_uniq_fn = |m: &MigrationStateTuple| m.1.date_time;
            let cmp_fn = |l: &Migration, r: &Migration| l.cmp(r);
            let tuple_cmp_fn = |l: &MigrationStateTuple, r: &MigrationStateTuple| l.1.date_time.cmp(&r.1.date_time);
            let mut_cmp_fn = |l: &mut Migration, r: &mut Migration| l.cmp(&r);

            // This collects an interator of all the date-times on the provided list,
            //
            let known_migrations = migrations.into_iter().sorted_by(cmp_fn).unique_by(uniq_fn);
            // let closure = |(version, down, configuration_name)| {
            //   crate::migrations::from_stored_migration(self.config, version, configuration_name, down)
            // };
            let q = format!("SELECT `version`, `down`, `configuration_name` FROM `{}` ORDER BY `version` ASC;", MARIADB_MIGRATION_STATE_TABLE_NAME);
            let applied_migrations = self.conn.query_map::<(String, Option<String>, String),_,_,Migration>(q, |(version, down, configuration_name)| -> Migration {
              from_stored_migration(version, configuration_name, down)
            })?.into_iter().sorted_by(cmp_fn).unique_by(uniq_fn);

            // Applied migrations appear in both sets
            let applied = iter_set::union_by(known_migrations.clone(), applied_migrations.clone(), mut_cmp_fn).map(|m| (MigrationState::Applied, m));
            // Pending migrations appear only in known, but not applied
            let pending = iter_set::difference_by(known_migrations.clone(), applied_migrations.clone(), mut_cmp_fn).map(|m| (MigrationState::Pending, m));
            // Orphan migrations appear only in applied, but not in known
            let orphan = iter_set::difference_by(applied_migrations.clone(), known_migrations.clone(), mut_cmp_fn).map(|m| (MigrationState::Orphaned, m));

            Ok(orphan.chain(pending).chain(applied).sorted_by(tuple_cmp_fn).unique_by(tuple_uniq_fn).collect())
          }
        } else {
            Err(StateStoreError::ErrorReadingMigrationState)
        }
    }
}

#[cfg(test)]
mod tests {

    extern crate rand;
    extern crate tempdir;

    use super::*;

    use crate::config::Configuration;
    use crate::migrations::{migrations, Migration};
    use crate::runner::{MigrationResult, MigrationState};
    use crate::state_store::Error as StateStoreError;
    use indoc::indoc;
    use maplit::hashmap;
    use mysql::prelude::Queryable;
    use mysql::{Conn, OptsBuilder};
    use rand::Rng;
    use serial_test::serial;
    use std::path::PathBuf;
    use tempdir::TempDir;

    const TEST_DB_IP: &'static str = "127.0.0.1";
    const TEST_DB_PORT: u16 = 3306;
    const TEST_DB_USER: &'static str = "root";
    const TEST_DB_PASSWORD: &'static str = "example";

    struct TestDB {
        conn: mysql::Conn,
        config: Configuration,
    }

    impl Drop for TestDB {
        fn drop(&mut self) {
            for (_, rc) in &self.config.configured_runners {
                match helper_delete_test_db(&mut self.conn, &rc) {
                    Ok(_) => debug!("success cleaning up db {:?} text database", rc.database),
                    Err(e) => info!(
                        "error, there may be some clean-up to do for {:?}: {:?}",
                        rc, e
                    ),
                };
            }
        }
    }

    fn helper_create_runner_config(dbname: Option<&str>) -> Configuration {
        // None means really none, but Some("") indicates that we should
        // generate a random one. A non-empty string will be used.
        let dbname = match dbname {
            Some(dbname) => Some(match dbname {
                "" => format!("mitre_test_{}", rand::thread_rng().gen::<u32>()),
                _ => dbname.to_string(),
            }),
            None => None,
        };
        Configuration {
            migrations_directory: PathBuf::from(
                TempDir::new("helper_create_runner_config")
                    .expect("could not make tmpdir")
                    .into_path(),
            ),
            configured_runners: hashmap! {
                String::from("mariadb") => RunnerConfiguration {
                  _runner: String::from(crate::reserved::MARIA_DB).to_lowercase(),
                  database_number: None,
                  database: Some(format!("mitre_other_test_db_{}", rand::thread_rng().gen::<u32>()),),
                  index: None,
                  ip_or_hostname: Some(String::from(TEST_DB_IP)),
                  password: Some(String::from(TEST_DB_PASSWORD)),
                  port: Some(TEST_DB_PORT),
                  username: Some(String::from(TEST_DB_USER)),
              },
              String::from("mitre") => RunnerConfiguration {
                _runner: String::from(crate::reserved::MARIA_DB).to_lowercase(),
                database_number: None,
                database: dbname, // the one we want to bootstrap
                index: None,
                ip_or_hostname: Some(String::from(TEST_DB_IP)),
                password: Some(String::from(TEST_DB_PASSWORD)),
                port: Some(TEST_DB_PORT),
                username: Some(String::from(TEST_DB_USER)),
            }
            },
        }
    }

    fn helper_create_test_db() -> Result<TestDB, String> {
        let config = helper_create_runner_config(Some(""));
        let mariadb_config = config
            .configured_runners
            .get("mariadb")
            .ok_or_else(|| "no config")?;
        let mut conn = helper_db_conn()?;

        trace!("helper_create_test_db: creating database");
        match &mariadb_config.database {
            Some(dbname) => {
                let stmt_create_db = conn
                    .prep(format!("CREATE DATABASE `{}`", dbname))
                    .expect("could not prepare db create statement");
                match conn.exec::<bool, _, _>(stmt_create_db, ()) {
                    Err(e) => Err(format!("error creating test db {:?}", e)),
                    Ok(_) => Ok(TestDB {
                        conn,
                        config: config.clone(),
                    }),
                }
            }
            None => Err(String::from(
                "no dbname provided in config, test set-up error",
            )),
        }
    }

    fn helper_db_conn() -> Result<mysql::Conn, String> {
        let opts = mysql::Opts::from(
            OptsBuilder::new()
                .ip_or_hostname(Some(TEST_DB_IP))
                .user(Some(TEST_DB_USER))
                .pass(Some(TEST_DB_PASSWORD)),
        );
        match Conn::new(opts.clone()) {
            Ok(conn) => Ok(conn),
            Err(e) => Err(format!(
                "cannot connect to test db with {:?}: {:?}",
                opts, e
            )),
        }
    }

    fn helper_delete_test_db(
        conn: &mut mysql::Conn,
        config: &RunnerConfiguration,
    ) -> Result<(), String> {
        match &config.database {
            Some(dbname) => {
                let stmt_create_db = conn
                    .prep(format!("DROP DATABASE {}", dbname))
                    .expect("could not prepare statement");
                match conn.exec::<bool, _, _>(stmt_create_db, ()) {
                    Err(e) => Err(format!("error dropping test db {:?}", e)),
                    Ok(_) => Ok(()),
                }
            }
            None => Err(String::from(
                "no dbname provided in config, test set-up error",
            )),
        }
    }

    #[test]
    #[serial]
    fn it_requires_a_config_with_a_table_name() -> Result<(), String> {
        let config = helper_create_runner_config(None {});
        let mut runner = MariaDb::new_state_store(&config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;
        let migrations = vec![];

        let x = match runner.diff(migrations) {
            Ok(_) => Err(String::from("expected an error about missing dbname")),
            Err(e) => match e {
                StateStoreError::NoStateStoreDatabaseNameProvided => Ok(()),
                _ => Err(format!("did not expect error {:?}", e)),
            },
        };
        x
    }

    #[test]
    #[serial]
    fn it_returns_all_migrations_pending_if_db_does_not_exist() -> Result<(), String> {
        let config = helper_create_runner_config(Some(""));
        let mut runner = MariaDb::new_state_store(&config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;
        let migrations = migrations(&config).expect("should make at least default migrations");

        let x = match runner.diff(migrations) {
            Ok(pending_migrations) => {
                match pending_migrations
                    .iter()
                    .all(|pm| pm.0 == MigrationState::Pending)
                {
                    true => Ok(()),
                    false => Err(String::from("expected all migrations to be pending")),
                }
            }
            Err(e) => Err(format!("did not expect error {:?}", e)),
        };
        x
    }

    #[test]
    #[serial]
    fn it_returns_all_migrations_pending_if_migrations_table_does_not_exist() -> Result<(), String>
    {
        let test_db = helper_create_test_db()?;
        let config = match Configuration::load_from_str(indoc!(
            r"
          ---
          mitre:
            _runner: mariadb
        "
        )) {
            Ok(c) => c,
            Err(e) => Err(format!("error generating config: {}", e))?,
        };

        let mut runner = MariaDb::new_state_store(&test_db.config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;
        let migrations = migrations(&config).expect("should make at least default migrations");

        match runner.diff(migrations) {
            Ok(pending_migrations) => {
                match pending_migrations
                    .iter()
                    .all(|pm| pm.0 == MigrationState::Pending)
                {
                    true => Ok(()),
                    false => Err(String::from("expected all migrations to be pending")),
                }
            }
            Err(e) => Err(format!("did not expect error {:?}", e)),
        }
    }

    #[test]
    #[serial]
    fn migrating_up_just_the_built_in_migrations() -> Result<(), String> {
        let config = helper_create_runner_config(Some(""));

        let mut runner = MariaDb::new_state_store(&config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;
        let migrations = migrations(&config).expect("should make at least default migrations");

        // Arrange: Run up (only built-in, because tmp dir)
        match runner.up(migrations.clone(), None) {
            Ok(migration_results) => {
                print!("{:#?}", migration_results);

                let v = migration_results;
                assert_eq!(1, v.len());

                let v_success: Vec<&(MigrationResult, Migration)> = v
                    .iter()
                    .filter(|mr| mr.0 == MigrationResult::Success)
                    .collect();
                assert_eq!(1, v_success.len());
            }
            Err(e) => return Err(format!("did not expect error {:?}", e)),
        };

        // Assert that diff thinks all is clear
        match runner.diff(migrations.clone()) {
            Err(e) => return Err(format!("didn't expect err from diff {:?}", e)),
            Ok(diff_result) => {
                let diff_pending: Vec<(MigrationState, Migration)> = diff_result
                    .into_iter()
                    .filter(|mr| mr.0 == MigrationState::Pending)
                    .collect();
                assert_eq!(0, diff_pending.len());
            }
        };

        // Assert up is a noop
        match runner.up(migrations, None) {
            Ok(migration_results) => {
                let diff_pending: Vec<(MigrationResult, Migration)> = migration_results
                    .into_iter()
                    .filter(|mr| mr.0 == MigrationResult::AlreadyApplied)
                    .collect();

                assert_eq!(1, diff_pending.len());
            }
            Err(e) => return Err(format!("did not expect error running up again {:?}", e)),
        };

        Ok(())
    }

    #[test]
    fn checks_the_diff_in_run_migrations() {}
}
