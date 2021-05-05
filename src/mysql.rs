use std::path::PathBuf;

use super::{Error, IntoIter, Migration, MigrationList, MigrationStorage};
use crate::{
    config::{Configuration, RunnerConfiguration},
    migrations::{Direction, MigrationStep},
};
use log::{debug, error, info, trace};

use crate::migrations::FORMAT_STR;
use mysql::prelude::Queryable;

const MIGRATION_STATE_TABLE_NAME: &str = "mitre_migration_state";
const MIGRATION_STEPS_TABLE_NAME: &str = "mitre_migration_steps";

pub struct MySQL {
    conn: mysql::Conn,
    config: RunnerConfiguration,
}

impl MySQL {
    pub fn new(config: RunnerConfiguration) -> Result<Self, Error> {
        let opts = mysql::Opts::from(
            mysql::OptsBuilder::new()
                .ip_or_hostname(config.ip_or_hostname.clone())
                .user(config.username.clone())
                // NOTE: Do not specify database name here, otherwise we cannot
                // connect until the database exists. Makes it difficult to
                // bootstrap.
                // .db_name(config.database.clone())
                .pass(config.password.clone()),
        );
        info!("connection opts are {:?}", opts);
        Ok(MySQL {
            conn: match mysql::Conn::new(opts) {
                Ok(conn) => conn,
                Err(e) => {
                    return Err(Error::QueryFailed {
                        reason: Some(e),
                        msg: String::from("Checking for MySQL schema existance"),
                    })
                }
            },
            config: config,
        })
    }
}

impl MySQL {
    fn conn(&mut self) -> &mut mysql::Conn {
        debug!("select_db {:?}", self.config.database);
        match &self.config.database {
            Some(database) => {
                match self.conn.select_db(&database) {
                    true => trace!("select_db {:?} succeeded", database),
                    false => info!("select_db {:?} failed", database),
                }
                ()
            }
            None => info!("no database name provided, mysql driver might have a problem"),
        }
        &mut self.conn
    }
    fn bootstrap(&mut self) -> Result<(), Error> {
        // https://doc.rust-lang.org/std/macro.include_str.html
        let bootstrap_steps = vec![include_str!(
            "migrations/bootstrap_mysql_migration_storage.sql"
        )];
        let database = match &self.config.database {
            Some(database) => database,
            None => {
                return Err(Error::QueryFailed {
                    reason: None {},
                    msg: String::from("Checking for MySQL schema existance"),
                })
            }
        };
        let template_ctx = mustache::MapBuilder::new()
            .insert_str("database_name", database)
            .insert_str("migrations_table", MIGRATION_STATE_TABLE_NAME)
            .insert_str("migration_steps_table", MIGRATION_STEPS_TABLE_NAME)
            .build();

        for (i, bootstrap_step) in bootstrap_steps.iter().enumerate() {
            trace!(
                "about to apply bootstrapping query {}/{} {:#?}",
                bootstrap_step,
                i + 1,
                bootstrap_steps.len()
            );
            let q = match mustache::compile_str(bootstrap_step)
                .unwrap()
                .render_data_to_string(&template_ctx)
            {
                Ok(q) => q,
                Err(_e) => {
                    return Err(Error::QueryFailed {
                        reason: None {},
                        msg: String::from(
                            "couldn't render Mustache template of bootstrap statement",
                        ),
                    })
                }
            };
            debug!("rendered bootstrap query is {:?}", q);
            match self.conn.query_iter(q) {
                Ok(mut res) => {
                    info!(
                        "Applied bootstrapping query {}/{} successfully",
                        i + 1,
                        bootstrap_steps.len()
                    );
                    while let Some(result_set) = res.next_set() {
                        let result_set = result_set.expect("boom");
                        debug!(
                  "Result set _ meta: rows {}, last insert id {:?}, warnings {} info_str {}",
                  result_set.affected_rows(),
                  result_set.last_insert_id(),
                  result_set.warnings(),
                  result_set.info_str(),
              );
                    }
                }
                Err(e) => {
                    error!(
                        "applying bootstrapping query {}/{} failed {:?}",
                        e,
                        i + 1,
                        bootstrap_steps.len()
                    );
                    return Err(Error::QueryFailed {
                        reason: Some(e),
                        msg: String::from("Could not run the mysql query for bootstrapping"),
                    });
                }
            };
        }
        Ok(())
    }
}

impl MigrationList for MySQL {
    // It is important that this function return with an emtpy list when
    // the MySQL tables have not been bootstrapped yet to trigger the built-in migrations
    // to run.
    //
    // For readability, and because frankly, it ought to be fast enough for dozens, or even
    // small hundreds of Migrations that most apps probably have, there is a deliberate 1+n
    // query pattern here, where we first grab the migrations themselves from the migrations
    // table, and then follow-up to collect the steps in a 2nd round.
    //
    // This implementation is a bit over-careful, we could simply bypass the schema and table
    // checks, technically that would all still be an empty list, but having clear error
    // codes should make for a more useful piece of software in general, so we keep it.
    fn all(&mut self) -> Result<IntoIter<Migration>, Error> {
        let database = match &self.config.database {
            Some(database) => database.clone(),
            None => return Err(Error::ConfigurationIncomplete),
        };

        let schema_exists = match self.conn().exec_first::<bool, _, _>(
      "SELECT EXISTS(SELECT SCHEMA_NAME FROM INFORMATION_SCHEMA.SCHEMATA WHERE SCHEMA_NAME = ?)",
      (&database,)
    ) {
      Ok(r) => match r {
        Some(r) => r,
        None => return Err(Error::QueryFailed{reason: None{}, msg: String::from("No result (empty Option<T>) from schema presense check")}),
      },
      Err(e) => return Err(Error::QueryFailed{reason: Some(e), msg: String::from("Checking for MySQL schema existance")}),
    };

        let table_exists = match self.conn().exec_first::<bool, _, _>(
      "SELECT EXISTS(SELECT SCHEMA_NAME FROM INFORMATION_SCHEMA.SCHEMATA WHERE SCHEMA_NAME = ?)",
      (&database,)
    ) {
      Ok(r) => match r {
        Some(r) => r,
        None => return Err(Error::QueryFailed{reason: None{}, msg: String::from("No result (empty Option<T>) from table presense check")}),
      },
      Err(e) => return Err(Error::QueryFailed{reason: Some(e), msg: String::from("Checking for MySQL table existance")}),
    };

        if !schema_exists || !table_exists {
            info!(
                "schema_exists: {}, table_exists: {}",
                schema_exists, table_exists
            );
            info!("early return with empty migration list, we appear not to be initialized");
            return Ok(vec![].into_iter());
        }

        let q = format!("SELECT `version`, `flags`, `configuration_name`, `built_in` FROM {t} ORDER BY version ASC", t = MIGRATION_STATE_TABLE_NAME);

        let mut migrations = match self
            .conn()
            .query_map::<(String, String, String, bool), _, _, Migration>(
                q,
                |(version, flags, configuration_name, built_in)| -> Migration {
                    Migration {
                        built_in,
                        configuration_name,
                        date_time: chrono::NaiveDateTime::parse_from_str(
                            version.as_str(),
                            FORMAT_STR,
                        )
                        .unwrap(),
                        flags: crate::reserved::flags_from_str_flags(&flags),
                        steps: std::collections::HashMap::new(),
                    }
                },
            ) {
            Ok(migrations) => migrations,
            Err(e) => {
                return Err(Error::QueryFailed {
                    reason: Some(e),
                    msg: String::from("Querying list of MySQL stored migrations"),
                })
            }
        };

        for m in &mut migrations {
            let q = format!(
                "SELECT `direction`, `source`, `path` FROM {t} WHERE version = {v}",
                t = MIGRATION_STEPS_TABLE_NAME,
                v = m
                    .date_time
                    .format(crate::migrations::FORMAT_STR)
                    .to_string(),
            );

            let steps = match self
                .conn()
                .query_map::<(String, String, String), _, _, (Direction, MigrationStep)>(
                    q,
                    |(direction, source, path)| {
                        (
                            Direction::from(direction),
                            MigrationStep {
                                source,
                                path: PathBuf::from(path),
                            },
                        )
                    },
                ) {
                Ok(steps) => steps,
                Err(e) => {
                    return Err(Error::QueryFailed {
                        reason: Some(e),
                        msg: String::from("Querying list of MySQL stored migrations"),
                    })
                }
            };
            for (direction, step) in steps {
                m.steps.insert(direction, step);
            }
        }

        Ok(migrations.into_iter())
    }
}

impl MigrationStorage for MySQL {
    #[cfg(test)]
    fn reset(&mut self) -> Result<(), Error> {
        let database = match &self.config.database {
            Some(database) => database.clone(),
            None => return Err(Error::ConfigurationIncomplete),
        };
        match self
            .conn()
            .exec_first::<bool, _, _>("DROP DATABASE IF EXISTS = ?)", (&database,))
        {
            Ok(r) => match r {
                Some(_) => Ok(()),
                None => {
                    return Err(Error::QueryFailed {
                        reason: None {},
                        msg: String::from("No result from table presense check"),
                    })
                }
            },
            Err(e) => {
                return Err(Error::QueryFailed {
                    reason: Some(e),
                    msg: String::from("Checking for MySQL table existance"),
                })
            }
        }
    }

    // https://docs.rs/mysql/20.1.0/mysql/index.html#transaction
    fn add(&mut self, m: Migration) -> Result<(), Error> {
        self.bootstrap()?;

        // Note, that transaction will be rolled back implicitly on Drop, if not committed.
        let mut tx = match self.conn().start_transaction(::mysql::TxOpts::default()) {
            Ok(tr) => Ok(tr),
            Err(e) => Err(Error::QueryFailed {
                reason: Some(e),
                msg: String::from("could not start transaction"),
            }),
        }?;
        // TODO, should we really REPLACE (upsert) into?
        let q = indoc::formatdoc!(
            "
          REPLACE INTO {} 
          (
            `version`, 
            `flags`, 
            `configuration_name`, 
            `built_in`
          ) 
          VALUES ( ?, ?, ?, ? );",
            MIGRATION_STATE_TABLE_NAME
        );
        match tx.exec_drop(
            q,
            (
                m.version(),
                m.flags_as_string(),
                &m.configuration_name,
                m.built_in,
            ),
        ) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::QueryFailed {
                reason: Some(e),
                msg: String::from("could not insert migration into migration state table"),
            }),
        }?;
        for (direction, s) in m.steps.clone() {
            let q = indoc::formatdoc!(
                "
          INSERT INTO {}
          (
                      `version`,
                      `direction`,
                      `source`,
                      `path`
          )
          VALUES ( ?, ?, ?, ? );",
                MIGRATION_STEPS_TABLE_NAME
            );
            match tx.exec_drop(
                q,
                (
                    m.version(),
                    format!("{:?}", direction).to_lowercase(),
                    &s.source,
                    &s.path.to_str(),
                ),
            ) {
                Ok(_) => Ok(()),
                Err(e) => Err(Error::QueryFailed {
                    reason: Some(e),
                    msg: String::from(
                        "could not insert migration steps into migration steps table",
                    ),
                }),
            }?;
        }
        match tx.commit() {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::QueryFailed {
                reason: Some(e),
                msg: String::from("could not commit transaction"),
            }),
        }
    }

    fn remove(&mut self, _: Migration) -> Result<(), Error> {
        todo!();
    }
}
