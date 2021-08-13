use crate::config::RunnerConfiguration;
use crate::migrations::MigrationStep;
use crate::runner::{Error, MigrationFileExtension, MigrationTemplate, Runner};
use indoc::indoc;
use mustache::MapBuilder;

pub struct PostgreSql {
    client: postgres::Client,
}

impl Runner for PostgreSql {
    fn meta(&self) -> crate::reserved::Runner {
        crate::reserved::runner_by_name(crate::reserved::POSTGRESQL)
            .expect("reserved word not found")
    }

    fn new_runner(config: RunnerConfiguration) -> Result<PostgreSql, Error> {
        // Ensure this is a proper config for this runner
        let runner_name = String::from(crate::reserved::POSTGRESQL).to_lowercase();
        if config._driver.to_lowercase() != runner_name {
            return Err(Error::RunnerNameMismatch {
                expected: runner_name,
                found: config._driver,
            });
        };

        let mut c = &mut postgres::Config::new();
        c = match config.username {
            Some(ref username) => c.user(username.as_str()),
            _ => c,
        };
        c = match config.password {
            Some(ref password) => c.password(password),
            _ => c,
        };
        c = match config.ip_or_hostname {
            Some(ref ip_or_hostname) => c.host(ip_or_hostname.as_str()),
            _ => c,
        };
        c = match config.port {
            Some(ref port) => c.port(*port),
            _ => c,
        };

        Ok(PostgreSql {
            client: c.connect(postgres::NoTls)?,
        })
    }

    fn apply(&mut self, ms: &MigrationStep) -> Result<(), Error> {
        let template_ctx = MapBuilder::new().build();
        let parsed = match ms.content() {
            Ok(tpl) => match tpl.render_data_to_string(&template_ctx) {
                Ok(str) => Ok(str),
                Err(e) => Err(Error::Template {
                    reason: e.to_string(),
                    template: tpl,
                }),
            },
            Err(e) => Err(Error::Template {
                reason: e.to_string(),
                template: mustache::compile_str("no template").unwrap(),
            }),
        }?;
        match self.client.simple_query(&parsed) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::PostgreSql(e)),
        }
    }

    fn migration_template(&self) -> (MigrationTemplate, MigrationTemplate, MigrationFileExtension) {
        (
            indoc!(
                "
          # Put your migration here
          CREATE TABLE your_table;
          "
            ),
            indoc!("DROP TABLE your_table"),
            "sql",
        )
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use rand::Rng;

    const TEST_DB_IP: &'static str = "127.0.0.1";
    const TEST_DB_PORT: u16 = 5432;
    const TEST_DB_USER: &'static str = "postgres";
    const TEST_DB_PASSWORD: &'static str = "example";

    fn helper_create_runner_config() -> RunnerConfiguration {
        RunnerConfiguration {
            _driver: String::from(crate::reserved::POSTGRESQL).to_lowercase(),
            database_number: None,
            database: Some(format!(
                "mitre_other_test_db_{}",
                rand::thread_rng().gen::<u32>()
            )),
            index: None,
            ip_or_hostname: Some(String::from(TEST_DB_IP)),
            password: Some(String::from(TEST_DB_PASSWORD)),
            port: Some(TEST_DB_PORT),
            username: Some(String::from(TEST_DB_USER)),
        }
    }

    #[test]
    fn test_creating_postgresql() -> Result<(), String> {
        let rc = helper_create_runner_config();
        match PostgreSql::new_runner(rc) {
            Ok(_psql) => Ok(()),
            Err(e) => Err(format!("Error: {:?}", e)),
        }
    }
}
