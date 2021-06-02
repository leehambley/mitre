use super::MARIADB_MIGRATION_STATE_TABLE_NAME;
use crate::migrations::{Direction, Migration, MigrationStep, FORMAT_STR};
use crate::runner::MigrationState;
use crate::state_store::{Error as StateStoreError, MigrationStateTuple};
use crate::{
    config::{Configuration, RunnerConfiguration},
    state_store::StateStoreAdapter,
};
use itertools::Itertools;
use log::{debug, trace, warn};
use maplit::hashmap;
use mysql::{prelude::Queryable, Conn, OptsBuilder};
use std::path::PathBuf;

pub struct MariaDb {
    conn: Conn,
    config: Configuration,
    runner_config: RunnerConfiguration, // TODO: rename or deconstruct?
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
}

pub fn from_stored_migration(
    version: String,
    config_name: String,
    down_migration: Option<String>,
) -> Migration {
    let date_time = chrono::NaiveDateTime::parse_from_str(version.as_str(), FORMAT_STR).unwrap();
    let steps = match down_migration {
        Some(down_migration) => hashmap! {Direction::Down => MigrationStep {
          source: down_migration,
          path: PathBuf::new(),
        }},
        None => hashmap! {},
    };

    Migration {
        steps,
        built_in: false,
        date_time,
        flags: vec![], // TODO: fill me
        configuration_name: config_name,
    }
}

impl StateStoreAdapter for MariaDb {
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

    fn new(config: &Configuration) -> Result<MariaDb, StateStoreError> {
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
        })
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
