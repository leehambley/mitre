use crate::config::RunnerConfiguration;
use crate::runner::RunnersHashMap;
use mysql::Conn;

pub const MARIADB_MIGRATION_STATE_TABLE_NAME: &str = "mitre_migration_state";

mod runner;
mod state_store;

/// MariaDb is both a StateStore and a runner. The bootstrapping phase
/// means that when no migrations have yet been run, the StateStore may
/// attempt to connect to the database server when no database, or a
/// database with no tables exists. When bootstrapping the first connections
/// may swallow errors, the `diff()` method of StateStore may simply
/// return that all migrations are unapplied. Once the bootstrap migration
/// has run, it should be possible for the state store behaviour to
/// properly store results.
pub struct MariaDb {
    conn: Conn,

    // Configuration in case we are a runner not a state store
    runner_config: RunnerConfiguration,

    // Runners in a muxed'ed hashmap. This hashmap is keyed by [`crate::reserved::Runner`]
    runners: RunnersHashMap,
}

#[cfg(test)]
mod tests {

    extern crate rand;
    extern crate tempdir;

    use super::*;

    use crate::config::Configuration;
    use crate::migrations::{migrations, Migration};
    use crate::runner::{MigrationResult, MigrationState};
    use crate::state_store::{Error as StateStoreError, StateStore};
    use indoc::indoc;
    use maplit::hashmap;
    use mysql::prelude::Queryable;
    use mysql::{Conn, OptsBuilder};
    use rand::Rng;
    use std::path::PathBuf;
    use tempdir::TempDir;

    const TEST_DB_IP: &'static str = "127.0.0.1";
    const TEST_DB_PORT: u16 = 3306;
    const TEST_DB_USER: &'static str = "root";
    const TEST_DB_PASSWORD: &'static str = "example";

    struct TestDB {
        conn: mysql::Conn,
        config: Configuration,
    }

    impl Drop for TestDB {
        fn drop(&mut self) {
            for (_, rc) in &self.config.configured_runners {
                match helper_delete_test_db(&mut self.conn, &rc) {
                    Ok(_) => debug!("success cleaning up db {:?} text database", rc.database),
                    Err(e) => info!(
                        "error, there may be some clean-up to do for {:?}: {:?}",
                        rc, e
                    ),
                };
            }
        }
    }

    fn helper_create_runner_config(dbname: Option<&str>) -> Configuration {
        // None means really none, but Some("") indicates that we should
        // generate a random one. A non-empty string will be used.
        let dbname = match dbname {
            Some(dbname) => Some(match dbname {
                "" => format!("mitre_test_{}", rand::thread_rng().gen::<u32>()),
                _ => dbname.to_string(),
            }),
            None => None,
        };
        Configuration {
            migrations_directory: PathBuf::from(
                TempDir::new("helper_create_runner_config")
                    .expect("could not make tmpdir")
                    .into_path(),
            ),
            configured_runners: hashmap! {
                String::from("mariadb") => RunnerConfiguration {
                  _runner: String::from(crate::reserved::MARIA_DB).to_lowercase(),
                  database_number: None,
                  database: Some(format!("mitre_other_test_db_{}", rand::thread_rng().gen::<u32>()),),
                  index: None,
                  ip_or_hostname: Some(String::from(TEST_DB_IP)),
                  password: Some(String::from(TEST_DB_PASSWORD)),
                  port: Some(TEST_DB_PORT),
                  username: Some(String::from(TEST_DB_USER)),
              },
              String::from("mitre") => RunnerConfiguration {
                _runner: String::from(crate::reserved::MARIA_DB).to_lowercase(),
                database_number: None,
                database: dbname, // the one we want to bootstrap
                index: None,
                ip_or_hostname: Some(String::from(TEST_DB_IP)),
                password: Some(String::from(TEST_DB_PASSWORD)),
                port: Some(TEST_DB_PORT),
                username: Some(String::from(TEST_DB_USER)),
            }
            },
        }
    }

    fn helper_create_test_db() -> Result<TestDB, String> {
        let config = helper_create_runner_config(Some(""));
        let mariadb_config = config
            .configured_runners
            .get("mariadb")
            .ok_or_else(|| "no config")?;
        let mut conn = helper_db_conn()?;

        trace!("helper_create_test_db: creating database");
        match &mariadb_config.database {
            Some(dbname) => {
                let stmt_create_db = conn
                    .prep(format!("CREATE DATABASE `{}`", dbname))
                    .expect("could not prepare db create statement");
                match conn.exec::<bool, _, _>(stmt_create_db, ()) {
                    Err(e) => Err(format!("error creating test db {:?}", e)),
                    Ok(_) => Ok(TestDB {
                        conn,
                        config: config.clone(),
                    }),
                }
            }
            None => Err(String::from(
                "no dbname provided in config, test set-up error",
            )),
        }
    }

    fn helper_db_conn() -> Result<mysql::Conn, String> {
        let opts = mysql::Opts::from(
            OptsBuilder::new()
                .ip_or_hostname(Some(TEST_DB_IP))
                .user(Some(TEST_DB_USER))
                .pass(Some(TEST_DB_PASSWORD)),
        );
        match Conn::new(opts.clone()) {
            Ok(conn) => Ok(conn),
            Err(e) => Err(format!(
                "cannot connect to test db with {:?}: {:?}",
                opts, e
            )),
        }
    }

    fn helper_delete_test_db(
        conn: &mut mysql::Conn,
        config: &RunnerConfiguration,
    ) -> Result<(), String> {
        match &config.database {
            Some(dbname) => {
                let stmt_create_db = conn
                    .prep(format!("DROP DATABASE {}", dbname))
                    .expect("could not prepare statement");
                match conn.exec::<bool, _, _>(stmt_create_db, ()) {
                    Err(e) => Err(format!("error dropping test db {:?}", e)),
                    Ok(_) => Ok(()),
                }
            }
            None => Err(String::from(
                "no dbname provided in config, test set-up error",
            )),
        }
    }

    #[test]
    fn it_requires_a_config_with_a_table_name() -> Result<(), String> {
        let config = helper_create_runner_config(None {});
        let mut runner = MariaDb::new_state_store(&config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;
        let migrations = vec![];

        let x = match runner.diff(migrations) {
            Ok(_) => Err(String::from("expected an error about missing dbname")),
            Err(e) => match e {
                StateStoreError::NoStateStoreDatabaseNameProvided => Ok(()),
                _ => Err(format!("did not expect error {:?}", e)),
            },
        };
        x
    }

    #[test]
    fn it_returns_all_migrations_pending_if_db_does_not_exist() -> Result<(), String> {
        let config = helper_create_runner_config(Some(""));
        let mut runner = MariaDb::new_state_store(&config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;
        let migrations = migrations(&config).expect("should make at least default migrations");

        let x = match runner.diff(migrations) {
            Ok(pending_migrations) => {
                match pending_migrations
                    .iter()
                    .all(|pm| pm.0 == MigrationState::Pending)
                {
                    true => Ok(()),
                    false => Err(String::from("expected all migrations to be pending")),
                }
            }
            Err(e) => Err(format!("did not expect error {:?}", e)),
        };
        x
    }

    #[test]
    fn it_returns_all_migrations_pending_if_migrations_table_does_not_exist() -> Result<(), String>
    {
        let test_db = helper_create_test_db()?;
        let config = match Configuration::load_from_str(indoc!(
            r"
          ---
          mitre:
            _runner: mariadb
        "
        )) {
            Ok(c) => c,
            Err(e) => Err(format!("error generating config: {}", e))?,
        };

        let mut runner = MariaDb::new_state_store(&test_db.config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;
        let migrations = migrations(&config).expect("should make at least default migrations");

        match runner.diff(migrations) {
            Ok(pending_migrations) => {
                match pending_migrations
                    .iter()
                    .all(|pm| pm.0 == MigrationState::Pending)
                {
                    true => Ok(()),
                    false => Err(String::from("expected all migrations to be pending")),
                }
            }
            Err(e) => Err(format!("did not expect error {:?}", e)),
        }
    }

    #[test]
    fn migrating_up_just_the_built_in_migrations() -> Result<(), String> {
        let config = helper_create_runner_config(Some(""));

        //   let _ = std::fs::remove_dir_all("/tmp/migrating_up_just_the_built_in_migrations");

        //   match std::fs::create_dir("/tmp/migrating_up_just_the_built_in_migrations") {
        //     Err(e) => return Err(format!("cannot create tempdir: {}", e)),
        //     _ => {},
        //   };

        //   let config = match Configuration::load_from_str(indoc! {r"
        //   ---
        //   migrations_directory: /tmp/migrating_up_just_the_built_in_migrations
        //   mitre:
        //     _runner: mariadb
        // "})
        // {
        //     Ok(c) => c,
        //     Err(e) => Err(format!("error generating config: {}", e))?,
        // };

        let mut runner = MariaDb::new_state_store(&config)
            .map_err(|e| format!("Could not create state store {:?}", e))?;
        let migrations = migrations(&config).expect("should make at least default migrations");

        // Arrange: Run up (only built-in, because tmp dir)
        match runner.up(migrations.clone(), None) {
            Ok(migration_results) => {
                print!("{:#?}", migration_results);

                let v = migration_results;
                assert_eq!(1, v.len());

                let v_success: Vec<&(MigrationResult, Migration)> = v
                    .iter()
                    .filter(|mr| mr.0 == MigrationResult::Success)
                    .collect();
                assert_eq!(1, v_success.len());
            }
            Err(e) => return Err(format!("did not expect error {:?}", e)),
        };

        // Assert that diff thinks all is clear
        match runner.diff(migrations.clone()) {
            Err(e) => return Err(format!("didn't expect err from diff {:?}", e)),
            Ok(diff_result) => {
                let diff_pending: Vec<(MigrationState, Migration)> = diff_result
                    .into_iter()
                    .filter(|mr| mr.0 == MigrationState::Pending)
                    .collect();
                assert_eq!(0, diff_pending.len());
            }
        };

        // Assert up is a noop
        match runner.up(migrations, None) {
            Ok(migration_results) => {
                let diff_pending: Vec<(MigrationResult, Migration)> = migration_results
                    .into_iter()
                    .filter(|mr| mr.0 == MigrationResult::AlreadyApplied)
                    .collect();

                assert_eq!(1, diff_pending.len());
            }
            Err(e) => return Err(format!("did not expect error running up again {:?}", e)),
        };

        Ok(())
    }

    #[test]
    fn checks_the_diff_in_run_migrations() {}
}
