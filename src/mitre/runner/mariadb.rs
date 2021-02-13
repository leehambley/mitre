use crate::mitre::config::RunnerConfiguration;
use crate::mitre::migrations::Migration;
use mysql::prelude::Queryable;
use mysql::{Conn, OptsBuilder};
use std::convert::From;

const MARIADB_MIGRATION_STATE_TABLE_NAME: &'static str = "mitre_migration_state";

#[derive(Debug)]
pub struct MariaDB {
    conn: Conn,
    config: RunnerConfiguration,
}

#[derive(Debug)]
pub enum Error {
    MariaDB(mysql::Error),
    PingFailed(),

    ErrorRunningQuery,
    MigrationStateSchemaDoesNotExist,
    MigrationStateTableDoesNotExist,

    NoStateStoreDatabaseNameProvided,

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
            config: config.to_owned(),
        });
    }

    // https://docs.rs/mysql/20.1.0/mysql/struct.Conn.html
    fn bootstrap(&mut self) -> Result<(), Error> {
        ensure_connectivity(self)
    }

    fn diff<'a>(
        &mut self,
        migrations: impl Iterator<Item = Migration> + 'a,
    ) -> Result<Box<dyn Iterator<Item = Self::MigrationStateTuple> + 'a>, Error> {
        let database = match &self.config.database {
            Some(database) => Ok(database),
            None => Err(Error::NoStateStoreDatabaseNameProvided),
        }?;

        // Database doesn't exist, then obviously nothing ran... (or we have no permission)
        let schema_exists = self.conn.exec_first::<bool, _, _>(
          "SELECT EXISTS(SELECT SCHEMA_NAME FROM INFORMATION_SCHEMA.SCHEMATA WHERE SCHEMA_NAME = ?)",
          (database,) //trailing comma makes this a tuple
        )?;
        match schema_exists {
            Some(schema_exists) => {
                println!("schema exists?: {}", schema_exists);
                if !schema_exists {
                    println!("about to create {}", database);
                    let stmt_create_db = self.conn.prep(format!("CREATE DATABASE {}", database))?;
                    self.conn.exec::<bool, _, _>(stmt_create_db, ())?;
                }
            }
            None => {
                return Err(Error::ErrorRunningQuery);
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
            // let iter = migrations.map(|m| (false, m)).collect::<Vec<Self::MigrationStateTuple>>().into_iter();
            // let iter = migrations.map(|m| (false, m)).into_iter();
            return Ok(Box::new(migrations.map(|m| (false, m)).into_iter()));
            } else {

              // TODO check what migrations did run, and 
              // Thinking something like a SELECT * FROM <migration schema table> WHERE timestamp NOT IN <1,2,3,4,5,6> 
              // (the migrations we know about) .. theory being we send our list, they send back a list, maybe
              // that list isn't enormous, or we check out cursor based 

              panic!("this road isn't finished yet, turn around!");

            }
          },
          None => return Err(Error::ErrorRunningQuery)
        }
        Ok(Box::new(std::iter::empty()))
    }
}

#[cfg(test)]
mod tests {

    extern crate rand;
    extern crate tempdir;

    use super::*;
    use crate::mitre::migrations::migrations;
    use crate::mitre::runner::Runner;
    use rand::Rng;
    use tempdir::TempDir;

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

    fn helper_create_test_db() -> Result<(mysql::Conn, RunnerConfiguration), &'static str> {
        let runner_config = helper_create_runner_config();

        let mut conn = helper_db_conn();

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
            None => panic!(),
        }

        Ok((conn, runner_config))
    }

    fn helper_db_conn() -> mysql::Conn {
        let opts = mysql::Opts::from(
            OptsBuilder::new()
                .ip_or_hostname(Some(TEST_DB_IP))
                .user(Some(TEST_DB_USER))
                .pass(Some(TEST_DB_PASSWORD)),
        );
        Conn::new(opts.clone())
            .expect(format!("cannot connect to test db with {:?}", opts).as_str())
    }

    fn helper_delete_test_db(
        mut conn: mysql::Conn,
        config: &RunnerConfiguration,
    ) -> Result<(), &'static str> {
        let stmt_create_db = conn
            .prep(format!("DELETE DATABASE {:?}", config.database))
            .expect("boom");
        match conn.exec::<bool, _, _>(stmt_create_db, ()) {
            Err(e) => panic!(e),
            _ => {
                println!("remmoved test db")
            }
        }
        Ok(())
    }

    #[test]
    fn fails_if_selected_db_does_not_exists() -> Result<(), String> {
        let mut config = helper_create_runner_config();
        config.database = None {};
        let mut runner = MariaDB::new(&config).map_err(|e| "Could not create runner")?;
        let migrations = std::iter::empty::<Migration>();

        match runner.diff(migrations) {
            Ok(_) => Err(String::from("Expected an error to occur")),
            Err(e) => match e {
                Error::NoStateStoreDatabaseNameProvided => Ok(()),
                _ => Err(format!("did not expect error {:?}", e)),
            },
        }
    }

    // Mitre does not *require* the configured database to exist, but we must
    // be able to create it. This allows us to init the connection to the server
    // specifying no database, and proceed with better errors than hitting a
    // wall attempting server and database connection in the same call.
    //
    // Note at the diff stage, we don't want to create the *table* because
    // that would mean running the first of the built-in migrations, and we
    // don't want to _quite_ do that yet.
    #[test]
    fn creates_db_if_it_does_not_exists() -> Result<(), &'static str> {
        let tmp_dir = TempDir::new("example").expect("gen tmpdir");
        let config = helper_create_runner_config();
        let mut runner = MariaDB::new(&config).map_err(|e| "Could not create runner")?;
        let migrations =
            migrations(tmp_dir.as_ref()).expect("should make at least default migrations");

        match runner.diff(migrations) {
            Ok(_) => {}
            Err(e) => panic!("diff erred: {:?}", e),
        }

        // helper_delete_test_db(helper_db_conn(), &config)
        Ok(())
    }

    #[test]
    fn checks_the_diff_in_run_migrations() {}
}
