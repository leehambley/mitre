use std::path::PathBuf;

use super::{Error, IntoIter, Migration, MigrationList, MigrationStorage};
use crate::{
    config::RunnerConfiguration,
    migrations::{Direction, MigrationStep},
};
use log::{debug, info, trace};

use crate::migrations::FORMAT_STR;
use mysql::prelude::Queryable;

const MIGRATION_STATE_TABLE_NAME: &str = "mitre_migration_state";
const MIGRATION_STEPS_TABLE_NAME: &str = "mitre_migration_steps";

struct MySQL {
    conn: mysql::Conn,
    config: RunnerConfiguration,
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

        let migrations = match self
            .conn()
            .query_map::<(String, String, String, bool), _, _, Result<Migration, Error>>(
                q,
                |(version, flags, configuration_name, built_in)| -> Result<Migration, Error> {

                  let q = format!("SELECT `direction`, `source` FROM {t} WHERE version = {v}", t = MIGRATION_STEPS_TABLE_NAME, v = version);
                  let steps = self
            .conn()
              .query_map::<(String, String), _, _, Result<(Direction, MigrationStep), Error>>(q, |(direction, source)| {
                Ok((Direction::Up, MigrationStep{path: PathBuf::from("..."), source: String::from("...")}))
            });

                    Ok(Migration {
                        built_in,
                        configuration_name,
                        date_time: chrono::NaiveDateTime::parse_from_str(
                            version.as_str(),
                            FORMAT_STR,
                        )
                        .unwrap(),
                        flags: crate::reserved::flags_from_str_flags(&flags),
                        steps: std::collections::HashMap::new(),
                    })
                },
            ) {
            Ok(migrations) => migrations.into_iter(),
            Err(e) => {
                return Err(Error::QueryFailed {
                    reason: Some(e),
                    msg: String::from("Querying list of MySQL stored migrations"),
                })
            }
        };

        let m: Vec<Migration> = migrations.filter_map(|mr| mr.ok()).collect();

        Ok(m.into_iter())
    }
}

impl MigrationStorage for MySQL {
    fn add(&mut self, _: Migration) -> Result<(), Error> {
        todo!();
    }
    fn remove(&mut self, _: Migration) -> Result<(), Error> {
        todo!();
    }
}
