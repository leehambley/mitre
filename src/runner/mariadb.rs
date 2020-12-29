use crate::config::Configuration;
use mysql::{Conn, OptsBuilder};

#[derive(Debug)]
pub struct MariaDB {
    conn: Conn,
}

#[derive(Debug)]
pub enum MariaDBError {
    MySQL(mysql::Error),
    PingFailed(),
}

impl From<mysql::Error> for MariaDBError {
    fn from(err: mysql::Error) -> MariaDBError {
        MariaDBError::MySQL(err)
    }
}

fn ensure_connectivity(db: &mut MariaDB) -> Result<(), MariaDBError> {
    return match db.conn.ping() {
        true => Ok(()),
        false => Err(MariaDBError::PingFailed()),
    };
}

impl super::Runner for MariaDB {
    type Error = MariaDBError;
    fn new(config: &Configuration) -> Result<MariaDB, MariaDBError> {
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
    fn bootstrap(&mut self) -> Result<(), MariaDBError> {
        ensure_connectivity(self)
    }
}
