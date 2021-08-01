use crate::config::RunnerConfiguration;
use crate::migrations::{Migration, MigrationStep};
use crate::runner::Runner;
use mustache::MapBuilder;

// TCP ports are unsigned 16 bit ints
// https://tools.ietf.org/html/rfc793#section-3.1
const REDIS_DEFAULT_PORT: u16 = 6379;

use redis_raw::RedisConnection;
use tokio::net::TcpStream;

pub struct Redis {
    conn: redis_raw::RedisConnection,
}

#[derive(Debug)]
pub enum Error {
    /// Shadowing errors from the underlying postgresql library
    Redis(redis_raw::RedisError),

    // (reason, the template)
    Template {
        reason: String,
        template: mustache::Template,
    },

    RunnerNameMismatch {
        expected: String,
        found: String,
    }, // TODO: this is the same as the MySQL & PostgreSQL one
}

impl From<redis_raw::RedisError> for Error {
    fn from(err: redis_raw::RedisError) -> Error {
        Error::Redis(err)
    }
}

impl Runner for Redis {
    fn new_runner(config: RunnerConfiguration) -> Result<Redis, Error> {
        // Ensure this is a proper config for this runner
        let runner_name = String::from(crate::reserved::REDIS).to_lowercase();
        if config._runner.to_lowercase() != runner_name {
            return Err(Error::RunnerNameMismatch {
                expected: runner_name,
                found: config._runner,
            });
        };

        let conn_string = match (config.ip_or_hostname, config.port) {
            (Some(ip_or_hostname), Some(port)) => format!("{}:{}", ip_or_hostname, port),
            (Some(ip_or_hostname), None) => format!("{}:{}", ip_or_hostname, REDIS_DEFAULT_PORT),
            _ => format!("{}:{}", std::net::Ipv4Addr::LOCALHOST, REDIS_DEFAULT_PORT),
        };

        // Some backflips here, `new_runner` isn't async, so we do this little dance
        let future = async move {
            let stream = TcpStream::connect(conn_string).await?;
            let con: RedisConnection = stream.into();
            Ok(con)
        };

        let rt = tokio::runtime::Runtime::new()?;

        Ok(Redis {
            conn: rt.block_on(future),
        })
    }

    fn apply(&mut self, ms: &MigrationStep) -> Result<(), Error> {
        let template_ctx = MapBuilder::new().build();
        let parsed = match ms.content.render_data_to_string(&template_ctx) {
            Ok(str) => Ok(str),
            Err(e) => Err(Error::Template {
                reason: e.to_string(),
                template: ms.content.clone(),
            }),
        }?;
        match self.client.simple_query(&parsed) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::PostgreSQL(e)),
        }
    }

    fn migration_template(&self) -> (MigrationTemplate, MigrationTemplate, MigrationFileExtension) {
        (
            indoc!(
                "
          SET foo bar
        "
            ), indoc!("DEL foo"),
            "redis",
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
            _runner: String::from(crate::reserved::POSTGRESQL).to_lowercase(),
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
        match PostgreSQL::new_runner(rc) {
            Ok(_psql) => Ok(()),
            Err(e) => Err(format!("Error: {:?}", e)),
        }
    }
}
