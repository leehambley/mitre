use crate::config::Configuration;
use mysql::prelude::Queryable;
use mysql::{Conn, OptsBuilder};

#[derive(Debug)]
pub struct MariaDB {
    conn: Conn,
    db_name: Option<String>,
}

#[derive(Debug)]
pub enum RunnerError {
    MariaDB(mysql::Error),
    PingFailed(),
}

impl From<mysql::Error> for RunnerError {
    fn from(err: mysql::Error) -> RunnerError {
        RunnerError::MariaDB(err)
    }
}

#[derive(Debug)]
pub enum MariaDBMigrationStateStoreError {
    MariaDB(mysql::Error),
    AnyError, // todo remove me, just placeholding
}

impl From<mysql::Error> for MariaDBMigrationStateStoreError {
    fn from(err: mysql::Error) -> MariaDBMigrationStateStoreError {
        MariaDBMigrationStateStoreError::MariaDB(err)
    }
}

// non-public helper method
fn ensure_connectivity(db: &mut MariaDB) -> Result<(), RunnerError> {
    return match db.conn.ping() {
        true => Ok(()),
        false => Err(RunnerError::PingFailed()),
    };
}

impl crate::runner::Runner for MariaDB {
    type Errorrr = RunnerError;
    fn new(config: &Configuration) -> Result<MariaDB, RunnerError> {
        let opts = OptsBuilder::new()
            .ip_or_hostname(config.ip_or_hostname.clone())
            .user(config.username.clone())
            .db_name(config.database.clone())
            .pass(config.password.clone());
        return Ok(MariaDB {
            conn: Conn::new(opts)?,
            db_name: config.database.clone(),
        });
    }

    // https://docs.rs/mysql/20.1.0/mysql/struct.Conn.html
    fn bootstrap(&mut self) -> Result<(), RunnerError> {
        ensure_connectivity(self)
    }
}

impl crate::migration_state_store::MigrationStateStore for MariaDB {
    type Error = MariaDBMigrationStateStoreError;
    type Migration = crate::migrations::Migration;
    type MigrationState = (bool, crate::migrations::Migration);
    fn filter(
        &mut self,
        _migrations: Vec<crate::migrations::Migration>,
    ) -> Result<Vec<(bool, crate::migrations::Migration)>, MariaDBMigrationStateStoreError> {
        // Database doesn't exist, then obviously nothing ran... (or we have no permission)
        let schema_name = self.conn.exec_first::<String, _, _>(
            "SELECT SCHEMA_NAME
          FROM INFORMATION_SCHEMA.SCHEMATA
         WHERE SCHEMA_NAME = ?",
            (self.db_name.as_ref(),),
        )?; //trailing comma makes this a tuple
        match schema_name {
            Some(_schema_name) => {}
            None => {
                return Err(MariaDBMigrationStateStoreError::AnyError {}); // db doesn't exist in schema.
            }
        }

        // Same story for the table

        return Err(MariaDBMigrationStateStoreError::AnyError {});
    }
}
