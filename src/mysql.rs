use super::{Error, IntoIter, Migration, MigrationList, MigrationStorage};
use crate::config::RunnerConfiguration;
use log::{debug, info, trace};

use mysql::{prelude::Queryable, Conn, OptsBuilder};

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
    // to run
    fn all(&mut self) -> Result<IntoIter<Migration>, Error> {
        let database = &mut self
            .config
            .database
            .as_ref()
            .ok_or(Error::ConfigurationIncomplete)?;

        let schema_exists = match self.conn().exec_first::<bool, _, _>(
      "SELECT EXISTS(SELECT SCHEMA_NAME FROM INFORMATION_SCHEMA.SCHEMATA WHERE SCHEMA_NAME = ?)",
      (database,)
    ) {
      Ok(r) => match r {
        Some(r) => r,
        None => return Err(Error::QueryFailed{reason: String::from("No result (empty Option<T>) from schema presense check")}),
      },
      Err(e) => return Err(Error::QueryFailed{reason: String::from("Checking for MySQL schema existance")}),
    };

        let table_exists = match self.conn().exec_first::<bool, _, _>(
      "SELECT EXISTS(SELECT SCHEMA_NAME FROM INFORMATION_SCHEMA.SCHEMATA WHERE SCHEMA_NAME = ?)",
      (database,)
    ) {
      Ok(r) => match r {
        Some(r) => r,
        None => return Err(Error::QueryFailed{reason: String::from("No result (empty Option<T>) from table presense check")}),
      },
      Err(e) => return Err(Error::QueryFailed{reason: String::from("Checking for MySQL table existance")}),
    };

        if !schema_exists || !table_exists {
            info!(
                "schema_exists: {}, table_exists: {}",
                schema_exists, table_exists
            );
            info!("early return with empty migration list, we appear not to be initialized");
            return Ok(vec![].into_iter());
        }

        let q = format!(
            indoc::indoc!(
                "
      SELECT  a.version,
              a.flags,
              a.configuration_name,
              b.direction,
              b.source
      FROM    {migrations} AS a
              INNER JOIN {steps} AS b
                      ON a.version = b.version;  
    "
            ),
            migrations = MIGRATION_STEPS_TABLE_NAME,
            steps = MIGRATION_STEPS_TABLE_NAME
        );

        return Ok(vec![].into_iter());
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
