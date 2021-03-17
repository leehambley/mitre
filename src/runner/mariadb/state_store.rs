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
    fn select_db(&mut self) {
        match &self.runner_config.database {
            Some(database) => {
                trace!("select_db database name is {}", database);
                match &self.conn.select_db(&database) {
                    true => trace!("select_db successfully using {}", database),
                    false => trace!("could not switch to {} (may not exist yet?)", database),
                }
            }
            None => trace!("select_db no database name provided"),
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

    /// We do not record failures,
    fn record_success(
        &mut self,
        m: &Migration,
        _ms: &MigrationStep,
        d: std::time::Duration,
    ) -> Result<(), StateStoreError> {
        self.select_db(); // TODO: maybe move select_db inside .conn -> .conn()
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
    /// Given the set of runner configs on config, this will
    /// try to create a
    fn get_runner(&mut self, m: &Migration) -> Result<&mut BoxedRunner, StateStoreError> {
        // If we have a cached runner miss, let's
        trace!("looking up runner for {}", m.runner_and_config.0.name);
        if self.runners.get(&m.runner_and_config.0).is_none() {
            // Here we are checking that c.configured_runners contains a config for
            // the suitable runner.
            //
            // I feel like this check is _entirely_ redundant, the `runner_and_config`
            // tuple we get here has already done the mapping, and the migrations finder
            // raises an error if we have no suitable config
            let rc = match &self.config {
                Some(c) => c.configured_runners.iter().find(|(_name, cr)| {
                    cr._runner.to_lowercase() == m.runner_and_config.0.name.to_lowercase()
                }),
                None => None,
            }
            .ok_or_else(|| crate::state_store::Error::CouldNotFindOrCreateRunner)?;

            let new_runner = crate::runner::from_config(rc.1)?;

            match self
                .runners
                .insert(m.runner_and_config.0.clone(), new_runner)
            {
                None => trace!(
                    "clean insert of {} into runners map, no old value",
                    m.runner_and_config.0.name
                ),
                Some(_) => warn!(
                    "insert of {} into runners map overwrote a previous value, race condition?",
                    m.runner_and_config.0.name
                ),
            };
        }

        match self.runners.get_mut(&m.runner_and_config.0) {
            Some(r) => Ok(r),
            None => Err(StateStoreError::CouldNotFindOrCreateRunner),
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
            config: None {}, // we are a runner
            runner_config: mariadb_config,
            runners: RunnersHashMap::new(),
        })
    }

    fn up(
        &mut self,
        migrations: Vec<Migration>,
    ) -> Result<Vec<MigrationResultTuple>, StateStoreError> {
        Ok(self
            .diff(migrations)?
            .into_iter()
            .map(|(migration_state, migration)| match migration_state {
                MigrationState::Pending => match (
                    migration.steps.get(&Direction::Change),
                    migration.steps.get(&Direction::Up),
                ) {
                    (Some(_up_step), Some(_change_step)) => (
                        MigrationResult::Failure {
                            reason: String::from("contains both up and down parts"),
                        },
                        migration,
                    ),
                    (Some(up_step), None) => self.apply_migration_step(migration.clone(), up_step),
                    (None, Some(change_step)) => {
                        self.apply_migration_step(migration.clone(), change_step)
                    }
                    (None, None) => (MigrationResult::NothingToDo, migration),
                },
                MigrationState::Applied => (MigrationResult::AlreadyApplied, migration),
            })
            .collect())
    }

    fn diff(
        &mut self,
        migrations: Vec<Migration>,
    ) -> Result<Vec<MigrationStateTuple>, StateStoreError> {
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
