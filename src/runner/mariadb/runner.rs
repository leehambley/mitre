use super::MARIADB_MIGRATION_STATE_TABLE_NAME;
use crate::config::RunnerConfiguration;
use crate::migrations::MigrationStep;
use crate::runner::{Error as RunnerError, MigrationFileExtension, MigrationTemplate, Runner};
use indoc::indoc;
use log::{debug, info, trace};
use mustache::MapBuilder;
use mysql::prelude::Queryable;
use mysql::{Conn, OptsBuilder};

pub struct MariaDb {
    conn: Conn,
    config: RunnerConfiguration,
}

impl MariaDb {
    // This methoe exists in two places, almost certainly a code-smell.
    pub fn select_db(&mut self) -> bool {
        match &self.config.database {
            Some(database) => {
                trace!("select_db database name is {}", database);
                match &self.conn.select_db(&database) {
                    true => {
                        trace!("select_db successfully using {}", database);
                        true
                    }
                    false => {
                        trace!("could not switch to {} (may not exist yet?)", database);
                        false
                    }
                }
            }
            None => {
                trace!("select_db no database name provided");
                false
            }
        }
    }
}

impl Runner for MariaDb {
    fn new_runner(config: RunnerConfiguration) -> Result<MariaDb, RunnerError> {
        let runner_name = String::from(crate::reserved::MARIA_DB).to_lowercase();
        if config._runner.to_lowercase() != runner_name {
            return Err(RunnerError::RunnerNameMismatch {
                expected: runner_name,
                found: config._runner,
            });
        };

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
        Ok(MariaDb {
            conn: Conn::new(opts)?,
            config: config,
        })
    }

    // Applies a single migration (each runner needs something like this)
    // apply() does not try and record results, applying a migration may
    // drop a table or database leaving the system in a state where that
    // could fail. Up/down/migrate record state _using_ apply().
    fn apply(&mut self, ms: &MigrationStep) -> Result<(), RunnerError> {
        self.select_db();
        let template_ctx = MapBuilder::new()
            .insert_str(
                "mariadb_migration_state_table_name",
                MARIADB_MIGRATION_STATE_TABLE_NAME,
            )
            .insert_str(
                "mariadb_migration_state_database_name",
                self.config.database.as_ref().unwrap(),
            )
            .build();

        trace!("rendering template to string from {:?}", ms.path);
        let tpl = match ms.content() {
            Ok(tpl) => tpl,
            Err(e) => {
                return Err(RunnerError::TemplateError {
                    reason: e.to_string(),
                    template: mustache::compile_str("").unwrap(),
                })
            }
        };
        let parsed = match tpl.render_data_to_string(&template_ctx) {
            Ok(str) => Ok(str),
            Err(e) => Err(RunnerError::TemplateError {
                reason: e.to_string(),
                template: tpl.clone(),
            }),
        }?;
        trace!("template rendered to string successfully: {:?}", parsed);

        debug!("executing query {}", parsed);
        match self.conn.query_iter(parsed) {
            Ok(mut res) => {
                // TODO: do something more with QueryResult
                trace!(
                    "Had {} warnings and this info: {}",
                    res.warnings(),
                    res.info_str()
                );
                // TODO: With a fault in one of the migrations, it's possible to get stuck
                // here seemingly indefinitely, go ahead, add a stray comma after one of the
                // columns in the migration_schema.mitre.sql and watch this hang forever
                // waiting for a res.next_set() that never seems to come.
                while let Some(result_set) = res.next_set() {
                    let result_set = result_set.expect("boom");
                    info!(
                        "Result set _ meta: rows {}, last insert id {:?}, warnings {} info_str {}",
                        result_set.affected_rows(),
                        result_set.last_insert_id(),
                        result_set.warnings(),
                        result_set.info_str(),
                    );
                }
                Ok(())
            }
            Err(e) => {
                trace!("applying parsed query failed {:?}", e);
                Err(RunnerError::ErrorRunningMigration {
                    cause: e.to_string(),
                })
            }
        }
    }

    fn migration_template(&self) -> (MigrationTemplate, MigrationTemplate, MigrationFileExtension) {
        (
            indoc!(
                "
          # Put your migration here
          CREATE TABLE your_table (
              column_one VARCHAR(255) NOT NULL
          )
        "
            ),
            indoc!(
                "
              DROP TABLE your_table;
            "
            ),
            "sql",
        )
    }
}
