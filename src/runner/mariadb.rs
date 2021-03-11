use crate::config::{Configuration, RunnerConfiguration};
use crate::migrations::{Direction, Migration, MigrationStep};
use crate::runner::postgresql::PostgreSQL;
use crate::runner::BoxedRunner;
use crate::runner::RunnersHashMap;
use crate::runner::{Error as RunnerError, MigrationResult, MigrationState, Runner};
use crate::state_store::MigrationResultTuple;
use crate::state_store::MigrationStateTuple;
use crate::state_store::{Error as StateStoreError, StateStore};
use mustache::MapBuilder;
use mysql::prelude::Queryable;
use mysql::{Conn, OptsBuilder};

const MARIADB_MIGRATION_STATE_TABLE_NAME: &str = "mitre_migration_state";

/// MariaDb is both a StateStore and a runner. The bootstrapping phase
/// means that when no migrations have yet been run, the StateStore may
/// attempt to connect to the database server when no database, or a
/// database with no tables exists. When bootstrapping the first connections
/// may swallow errors, the `diff()` method of StateStore may simply
/// return that all migrations are unapplied. Once the bootstrap migration
/// has run, it should be possible for the state store behaviour to
/// properly store results.
pub struct MariaDb {
    conn: Conn,

    // All configurations because as a state-store MariaDb also
    // has to be able to look-up the correct implementation for.
    // Option<T> because we may simply be a runner (if someone uses
    // another store, and MariaDb is only used for running)
    config: Option<Configuration>,

    // Configuration in case we are a runner not a state store
    runner_config: RunnerConfiguration,

    // Runners in a muxed'ed hashmap. This hashmap is keyed by [`crate::reserved::Runner`]
    runners: RunnersHashMap,
}

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
                MigrationResult::Failure(format!("error setting up runner for change step{:?}", e)),
                m,
            ),
            Ok(runner) => match runner.apply(ms) {
                Ok(_) => match self.record_success(&m, ms, start.elapsed()) {
                    Ok(_) => (MigrationResult::Success, m),
                    Err(e) => (MigrationResult::Failure(format!("{:?}", e)), m),
                },
                Err(e) => (MigrationResult::Failure(format!("{:?}", e)), m),
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
            let _ = match &self.config {
                Some(c) => c.configured_runners.iter().find(|(_name, cr)| {
                    cr._runner.to_lowercase() == m.runner_and_config.0.name.to_lowercase()
                }),
                None => None,
            };

            let new_runner: BoxedRunner = match m.runner_and_config.0.name {
                crate::reserved::MARIA_DB => {
                    Box::new(MariaDb::new_runner(m.runner_and_config.1.clone())?)
                }
                crate::reserved::POSTGRESQL => {
                    Box::new(PostgreSQL::new_runner(m.runner_and_config.1.clone())?)
                }
                _ => return Err(StateStoreError::CouldNotFindOrCreateRunner),
            };

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
                        MigrationResult::Failure(format!(
                            "Migration has both up, and change parts. This is forbidden {:?}",
                            migration,
                        )),
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

impl Runner for MariaDb {
    fn new_runner(config: RunnerConfiguration) -> Result<MariaDb, RunnerError> {
        let runner_name = String::from(crate::reserved::MARIA_DB).to_lowercase();
        if config._runner.to_lowercase() != runner_name {
            return Err(RunnerError::RunnerNameMismatch {
                expected: runner_name,
                found: config._runner,
            });
        };

        let opts = mysql::Opts::from(
            OptsBuilder::new()
                .ip_or_hostname(config.ip_or_hostname.clone())
                .user(config.username.clone())
                // NOTE: Do not specify database name here, otherwise we cannot
                // connect until the database exists. Makes it difficult to
                // bootstrap.
                // .db_name(config.database.clone())
                .pass(config.password.clone()),
        );
        Ok(MariaDb {
            conn: Conn::new(opts)?,
            config: None {}, // we are a runner
            runner_config: config,
            runners: RunnersHashMap::new(),
        })
    }

    // Applies a single migration (each runner needs something like this)
    fn apply(&mut self, ms: &MigrationStep) -> Result<(), RunnerError> {
        let template_ctx = MapBuilder::new()
            .insert_str(
                "mariadb_migration_state_table_name",
                MARIADB_MIGRATION_STATE_TABLE_NAME,
            )
            .insert_str(
                "mariadb_migration_state_database_name",
                self.runner_config.database.as_ref().unwrap(),
            )
            .build();

        trace!("rendering template to string");
        let parsed = match ms.content.render_data_to_string(&template_ctx) {
            Ok(str) => Ok(str),
            Err(e) => Err(RunnerError::TemplateError {
                reason: e.to_string(),
                template: ms.content.clone(),
            }),
        }?;
        trace!("template rendered to string successfully: {:?}", parsed);

        debug!("executing query");
        match self.conn.query_iter(parsed) {
            Ok(mut res) => {
                // TODO: do something more with QueryResult
                trace!(
                    "Had {} warnings and this info: {}",
                    res.warnings(),
                    res.info_str()
                );
                while let Some(result_set) = res.next_set() {
                    let result_set = result_set.expect("boom");
                    info!(
                        "Result set meta: rows {}, last insert id {:?}, warnings {} info_str {}",
                        result_set.affected_rows(),
                        result_set.last_insert_id(),
                        result_set.warnings(),
                        result_set.info_str(),
                    );
                }
                Ok(())
            }
            Err(e) => {
                trace!("applying parsed query failed {:?}", e);
                Err(RunnerError::ErrorRunningMigration { cause: e })
            }
        }
    }
}

#[cfg(test)]
mod tests {

    extern crate rand;
    extern crate tempdir;

    use super::*;
    use crate::migrations::migrations;
    use indoc::indoc;
    use maplit::hashmap;
    use rand::Rng;
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
                    Err(e) => warn!(
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
    fn it_returns_all_migrations_pending_if_migrations_table_does_not_exist() -> Result<(), String>
    {
        let test_db = helper_create_test_db()?;
        let config = match Configuration::load_from_str(indoc! {r"
          ---
          migrations_directory: /tmp/must/not/exist
          mitre:
            _runner: mariadb
        "})
        {
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
    fn migrating_up_just_the_built_in_migrations() -> Result<(), String> {
        let test_db = helper_create_test_db()?;
        let config = match Configuration::load_from_str(indoc! {r"
        ---
        migrations_directory: /tmp/must/not/exist
        mitre:
          _runner: mariadb
      "})
        {
            Ok(c) => c,
            Err(e) => Err(format!("error generating config: {}", e))?,
        };

        let mut runner = MariaDb::new_state_store(&test_db.config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;
        let migrations = migrations(&config).expect("should make at least default migrations");

        // Arrange: Run up (only built-in, because tmp dir
        match runner.up(migrations.clone()) {
            Ok(migration_results) => {
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
        match runner.up(migrations) {
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
