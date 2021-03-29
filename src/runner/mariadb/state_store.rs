use super::{MariaDb, MARIADB_MIGRATION_STATE_TABLE_NAME};
use crate::config::Configuration;
use crate::migrations::{Direction, Migration, MigrationStep};
use crate::runner::{BoxedRunner, MigrationResult, MigrationState, RunnersHashMap};
use crate::state_store::{
    Error as StateStoreError, MigrationResultTuple, MigrationStateTuple, StateStore,
};
use mysql::{prelude::Queryable, Conn, OptsBuilder};

/// Helper methods for MariaDb (non-public) used in the context
/// of fulfilling the implementation of the runner::Runner trait.
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
                Ok(_) => match self.record_success(&m, ms, start.elapsed()) {
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
        _ms: &MigrationStep,
        d: std::time::Duration,
    ) -> Result<(), StateStoreError> {
        if !self.select_db() {
            return Err(StateStoreError::CouldNotRecordSuccess {
                reason: String::from(
                    "could not select db, that means the bootstrap migrations are not run",
                ),
            });
        }

        match self.conn.prep(format!("INSERT INTO {} (`version`, `up`, `down`, `change`, `applied_at_utc`, `apply_time_ms`, `built_in`, `environment`) VALUES (?, ?, ?, ?, ?, ?, ?, ?);", MARIADB_MIGRATION_STATE_TABLE_NAME)) {
        Ok(stmt) => match self.conn.exec_iter(stmt, (
            m.date_time,
            m.steps.get(&Direction::Up).map(|ms| format!("{:?}", ms.source )),
            m.steps.get(&Direction::Down).map(|ms| format!("{:?}", ms.source )),
            m.steps.get(&Direction::Change).map(|ms| format!("{:?}", ms.source )),
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            d.as_millis(),
            m.built_in,
            "NOT IMPLEMENTED"
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
    fn reset_state_store(config: &Configuration) -> Result<(), StateStoreError> {
        match config.configured_runners.get("mitre") {
            Some(mitre_config) => match crate::runner::from_config(mitre_config) {
                Ok(mut runner) => {
                    let drop_db = MigrationStep {
                        content: mustache::compile_str(
                            "DROP DATABASE IF EXISTS {{mariadb_migration_state_database_name}}",
                        )
                        .unwrap(),
                        path: std::path::PathBuf::from("./reset_state_store"),
                        source: String::from("no source"),
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
            m.runner_and_config.configuration_name
        );
        if self
            .runners
            .get(&m.runner_and_config.configuration_name)
            .is_none()
        {
            let new_runner = crate::runner::from_config(&m.runner_and_config.runner_configuration)?;

            match self
                .runners
                .insert(m.runner_and_config.configuration_name.clone(), new_runner)
            {
                None => trace!(
                    "clean insert of {} into runners map, no old value",
                    m.runner_and_config.runner.name
                ),
                Some(_) => warn!(
                    "insert of {} into runners map overwrote a previous value, race condition?",
                    m.runner_and_config.runner.name
                ),
            };
        }

        match self
            .runners
            .get_mut(&m.runner_and_config.configuration_name)
        {
            Some(r) => Ok(r),
            None => Err(StateStoreError::CouldNotFindOrCreateRunner),
        }
    }

    fn up(
        &mut self,
        migrations: Vec<Migration>,
        dt: Option<chrono::NaiveDateTime>,
    ) -> Result<Vec<MigrationResultTuple>, StateStoreError> {
        let _apply_until = dt.unwrap_or(chrono::Utc::now().naive_utc());
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
                MigrationState::Applied => (MigrationResult::AlreadyApplied, migration),
            })
            .collect())
    }

    fn down(
        &mut self,
        migrations: Vec<Migration>,
        dt: Option<chrono::NaiveDateTime>,
    ) -> Result<Vec<MigrationResultTuple>, StateStoreError> {
        let _unapply_after = dt.unwrap_or(chrono::Utc::now().naive_utc());
        Ok(self
            .diff(migrations)?
            .into_iter()
            .rev()
            .map(
                |(migration_state, migration)| -> (MigrationResult, Migration) {
                    match migration_state {
                        MigrationState::Applied => match migration.steps.get(&Direction::Down) {
                            Some(down_step) => {
                                self.unapply_migration_step(migration.clone(), down_step)
                            }
                            None => (MigrationResult::IrreversibleMigration, migration),
                        },
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
        match self.conn.exec_first::<bool, _, _>(
    "SELECT EXISTS( SELECT * FROM information_schema.tables WHERE table_schema = ? AND table_name = ? );",
      (database, MARIADB_MIGRATION_STATE_TABLE_NAME), //trailing comma makes this a tuple
    )? {
    Some(table_exists) => {
      if !table_exists {
          Ok(migrations.into_iter().map(|m| (MigrationState::Pending, m)).collect())
      } else {
          match self.conn.query_map::<_,_,_,String>(format!("SELECT `version` FROM `{}` ORDER BY `version` ASC;", MARIADB_MIGRATION_STATE_TABLE_NAME), |version| version) {
          Ok(stored_migration_versions) =>
             Ok(migrations.into_iter().map(move |m| {
              let migration_version = format!("{}", m.date_time.format(crate::migrations::FORMAT_STR));
              match stored_migration_versions.clone().into_iter().find(|stored_m| &migration_version == stored_m ) {
                  Some(_) => { trace!("found applied"); (MigrationState::Applied, m)},
                  None => { trace!("found pending"); (MigrationState::Pending, m)}
              }
            }).collect()),
          Err(e) => {
            warn!("could not check for migrations {:?}", e);
            Err(StateStoreError::MariaDb(e))
          }
        }
      }
    },
    None => Err(StateStoreError::ErrorReadingMigrationState)
  }
    }
}
