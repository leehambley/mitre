use crate::mitre::config::RunnerConfiguration;
use crate::mitre::migrations::Migration;
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
pub enum Error {
    MariaDB(mysql::Error),
    PingFailed(),

    ErrorRunningQuery,
    MigrationStateSchemaDoesNotExist,
    MigrationStateTableDoesNotExist,

    AnyError, // placeholder remove me
}

impl From<mysql::Error> for Error {
    fn from(err: mysql::Error) -> Error {
        Error::MariaDB(err)
    }
}

// non-public helper method
fn ensure_connectivity(db: &mut MariaDB) -> Result<(), Error> {
    return match db.conn.ping() {
        true => Ok(()),
        false => Err(Error::PingFailed()),
    };
}

impl crate::mitre::runner::Runner for MariaDB {
    type Error = Error;
    type Migration = Migration;

    type MigrationStateTuple = (bool, Migration);

    fn new(config: &RunnerConfiguration) -> Result<MariaDB, Error> {
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
    fn bootstrap(&mut self) -> Result<(), Error> {
        ensure_connectivity(self)
    }

    fn diff<'a>(
        &mut self,
        migrations: impl Iterator<Item = Migration> + 'a,
    ) -> Result<Box<dyn Iterator<Item = Self::MigrationStateTuple>>, Error> {
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
                        return Err(Error::MigrationStateSchemaDoesNotExist);
                    } // db doesn't exist in schema.
                }
            }
            None => {
                return Err(Error::ErrorRunningQuery);
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
                        return Err(Error::MigrationStateTableDoesNotExist);
                    } // db doesn't exist in schema.
                }
            }
            None => {
                return Err(Error::ErrorRunningQuery); // table doesn't exist in schema.
            }
        }
        Ok(Box::new(::std::iter::empty()))
    }
}

#[cfg(test)]
mod tests {

    extern crate rand;

    use super::*;
    use crate::mitre::runner::Runner;
    use rand::Rng;

    // use mysql::prelude::*;
    // use mysql::*;

    pub const DEFAULT_CONFIG_FILE: &'static str = "mitre.yml";

    const TEST_DB_IP: &'static str = "127.0.0.1";
    const TEST_DB_PORT: u16 = 3306;
    const TEST_DB_USER: &'static str = "root";
    const TEST_DB_PASSWORD: &'static str = "example";

    fn helper_create_runner_config() -> RunnerConfiguration {
        let random_database_name = format!("mitre_test_{}", rand::thread_rng().gen::<u32>());
        RunnerConfiguration {
            _runner: Some(String::from("mariadb")),
            database: Some(String::from(random_database_name)),
            ip_or_hostname: Some(String::from(TEST_DB_IP)),
            port: Some(TEST_DB_PORT),
            username: Some(String::from(TEST_DB_USER)),
            password: Some(String::from(TEST_DB_PASSWORD)),
            database_number: None,
            index: None,
        }
    }

    fn helper_create_test_db() -> Result<RunnerConfiguration, &'static str> {
        let opts = mysql::Opts::from(
            OptsBuilder::new()
                .ip_or_hostname(Some(TEST_DB_IP))
                .user(Some(TEST_DB_USER))
                .pass(Some(TEST_DB_PASSWORD)),
        );

        // let pool = Pool::new(opts).expect("boom");
        // let mut conn = pool.get_conn().expect("boom");

        let runner_config = helper_create_runner_config();

        let mut conn = Conn::new(opts.clone())
            .expect(format!("cannot connect to test db with {:?}", opts).as_str());

        match &runner_config.database {
            Some(dbname) => {
                let stmt_create_db = conn
                    .prep(format!("CREATE DATABASE {}", dbname))
                    .expect("boom");
                match conn.exec::<bool, _, _>(stmt_create_db, ()) {
                    Err(e) => panic!(e),
                    _ => {}
                }
            }
            None => panic!("I wish I was writing Ruby"),
        }

        Ok(runner_config)
    }

    // fn helper_delete_test_db(t: (String, mysql::Conn)) -> Result<(), ()> {
    //     Ok(())
    // }

    #[test]
    fn fails_if_selected_db_does_not_exists() -> Result<(), &'static str> {
        let config = helper_create_runner_config();
        let mut runner = MariaDB::new(&config).map_err(|e| "Could not create runner")?;
        let migrations = std::iter::empty::<Migration>();

        match runner.diff(migrations) {
            Ok(_) => Err("Expected an error to occur"),
            Err(e) => Ok(()),
        }
    }

    #[test]
    fn creates_db_if_it_does_not_exists() -> Result<(), &'static str> {
        let config = helper_create_test_db().map_err(|e| "Could not create test db")?;
        let mut runner = MariaDB::new(&config).map_err(|e| "Could not create runner")?;
        let migrations = std::iter::empty::<Migration>();

        // TODO: check if DB is present
        runner
            .diff(migrations)
            .map_err(|e| "Could not diff mirgrations")?;

        Ok(())
    }

    #[test]
    fn checks_the_diff_in_run_migrations() {}
}
