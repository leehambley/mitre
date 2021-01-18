use crate::built_in_migrations;
use crate::config::Configuration;
use mysql::prelude::Queryable;
use mysql::{Conn, OptsBuilder};
use std::convert::From;

const MARIADB_MIGRATION_STATE_TABLE_NAME: &'static str = "mitre_migration_state";

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
    MigrationStateSchemaDoesNotExist,
    MigrationStateTableDoesNotExist,

    ErrorRunningQuery,

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
    type Error = RunnerError;
    fn new(config: &Configuration) -> Result<MariaDB, RunnerError> {
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

    fn diff(
        &mut self,
        _migrations: Vec<crate::migrations::Migration>,
    ) -> Result<Vec<(bool, crate::migrations::Migration)>, MariaDBMigrationStateStoreError> {
        let _v = built_in_migrations::built_in_migrations();

        // Database doesn't exist, then obviously nothing ran... (or we have no permission)
        let schema_exists = self.conn.exec_first::<bool, _, _>(
            "SELECT EXISTS(SELECT SCHEMA_NAME FROM INFORMATION_SCHEMA.SCHEMATA WHERE SCHEMA_NAME = ?)",
            (self.db_name.as_ref(),), //trailing comma makes this a tuple
        )?;
        match schema_exists {
            Some(schema_exists) => {
                println!("schema exists?: {}", schema_exists);
                match schema_exists {
                    true => println!("nothing to do"),
                    false => {
                        return Err(
                            MariaDBMigrationStateStoreError::MigrationStateSchemaDoesNotExist,
                        );
                    } // db doesn't exist in schema.
                }
            }
            None => {
                return Err(MariaDBMigrationStateStoreError::ErrorRunningQuery);
            }
        }

        // Same story for the table
        let table_exists = self.conn.exec_first::<bool, _, _>(
          "SELECT EXISTS( SELECT * FROM information_schema.tables WHERE table_schema = ? AND table_name = ? );",
          (self.db_name.as_ref(), MARIADB_MIGRATION_STATE_TABLE_NAME), //trailing comma makes this a tuple
        )?;

        match table_exists {
            Some(table_exists) => {
                println!("table exists? {}", table_exists);
                match table_exists {
                    true => println!("nothing to do"),
                    false => {
                        return Err(
                            MariaDBMigrationStateStoreError::MigrationStateTableDoesNotExist,
                        );
                    } // db doesn't exist in schema.
                }
            }
            None => {
                return Err(MariaDBMigrationStateStoreError::ErrorRunningQuery); // table doesn't exist in schema.
            }
        }

        return Err(MariaDBMigrationStateStoreError::AnyError {});
    }
}
