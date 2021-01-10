use crate::config::Configuration;
use mysql::{Conn, OptsBuilder};

#[derive(Debug)]
pub struct MariaDB {
    conn: Conn,
}

#[derive(Debug)]
pub enum MariaDBRunnerError {
    MySQL(mysql::Error),
    PingFailed(),
}

impl From<mysql::Error> for MariaDBRunnerError {
    fn from(err: mysql::Error) -> MariaDBRunnerError {
        MariaDBRunnerError::MySQL(err)
    }
}

#[derive(Debug)]
pub enum MariaDBMigrationStateStoreError {
    MySQL(mysql::Error),
    AnyError, // todo remove me, just placeholding
}

impl From<mysql::Error> for MariaDBMigrationStateStoreError {
    fn from(err: mysql::Error) -> MariaDBMigrationStateStoreError {
        MariaDBMigrationStateStoreError::MySQL(err)
    }
}

// non-public helper method
fn ensure_connectivity(db: &mut MariaDB) -> Result<(), MariaDBRunnerError> {
    return match db.conn.ping() {
        true => Ok(()),
        false => Err(MariaDBRunnerError::PingFailed()),
    };
}

impl crate::runner::Runner for MariaDB {
    type Error = MariaDBRunnerError;
    fn new(config: &Configuration) -> Result<MariaDB, MariaDBRunnerError> {
        println!("using config {:?}", config);
        let opts = OptsBuilder::new()
            .ip_or_hostname(config.ip_or_hostname.clone())
            .user(config.username.clone())
            .db_name(config.database.clone())
            .pass(config.password.clone());
        println!("Connection options are: {:?}", opts);
        let conn = Conn::new(opts)?;
        return Ok(MariaDB { conn });
    }

    // https://docs.rs/mysql/20.1.0/mysql/struct.Conn.html
    fn bootstrap(&mut self) -> Result<(), MariaDBRunnerError> {
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
        return Err(MariaDBMigrationStateStoreError::AnyError {});
    }
}
