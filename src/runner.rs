use super::{Migration, MigrationStep, RunnerMeta};

#[cfg(feature = "runner_mysql")]
pub mod mysql;
#[cfg(feature = "runner_postgres")]
pub mod postgresql;

/// [`Runner`] specific configuration, there is
/// also  [`crate::config::Configuration`] which holds
/// the global configuration.
#[derive(Debug, PartialEq, Eq, PartialOrd, Clone)]
pub struct Configuration {
    // Runner is not optional, but we need to option it here to maintain
    // serde::Deserialize compatibility
    pub _runner: String,

    pub database: Option<String>, // used by MySQL, PostgreSQL runners

    pub index: Option<String>, // used by ElasticSearch

    pub database_number: Option<u8>, // used by Redis runner

    // Maybe this should have another name, we also would
    // probably accept IPs or anything resolveable here.
    pub ip_or_hostname: Option<String>, // used by cURL, MySQL, Redis, MySQL, PostgreSQL, ElasticSearch

    // Max value for port on linux comes from `cat /proc/sys/net/ipv4/ip_local_port_range`
    // u16 should be enough for most people most of the time.
    pub port: Option<u16>, // used by cURL, MySQL, Redis, MySQL, PostgreSQL, ElasticSearch

    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug)]
pub enum Error {
    #[cfg(feature = "runner_mysql")]
    MySql(::mysql::Error),

    #[cfg(feature = "runner_postgres")]
    PostgreSql(postgres::error::Error),

    /// No configuration provided for the runner, which is a problem
    NoConfigForRunner {
        name: String,
    },

    // Attempted to use the wrong runner/config combo
    RunnerNameMismatch {
        expected: String,
        found: String,
    },

    /// Some runners need a database name to be provided (typically RDBMS) for flexibility
    /// including the ability to create databases in migrations, that database is tentatively
    /// selected and we won't fail until the very last moment that we need to select the database
    /// but cannot.
    CouldNotSelectDatabase,

    /// Could not get a runner
    CouldNotGetRunner {
        reason: String,
    },

    /// Template error such as a syntax error.
    Template {
        reason: String,
        template: mustache::Template,
    },

    /// TODO: Describe these
    RunningMigration {
        cause: String,
    },

    /// We successfully ran the migration, but we didn't succeed in
    /// recording the status
    RecordingMigrationResult {
        cause: String,
    },

    // Couldn't make a runner from the config
    CouldNotFindOrCreateRunner {
        config_name: String,
    },

    /// Migrations may not contain both "up" and "change"
    MigrationContainsBothUpAndChange(Migration),

    MigrationHasFailed(String, Migration),
}

#[cfg(feature = "runner_mysql")]
impl From<::mysql::Error> for Error {
    fn from(err: ::mysql::Error) -> Error {
        Error::MySql(err)
    }
}

#[cfg(feature = "runner_postgres")]
impl From<postgres::error::Error> for Error {
    fn from(err: postgres::error::Error) -> Error {
        Error::PostgreSql(err)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Runner Error {:?}", self)
    }
}

#[derive(PartialEq, Debug)]
pub enum MigrationState {
    Pending,
    Applied,
    Orphaned,

    FilteredOut,
}

impl std::fmt::Display for MigrationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            MigrationState::Pending => write!(f, "Pending"),
            MigrationState::Applied => write!(f, "Applied"),
            MigrationState::Orphaned => write!(f, "Orphaned"),
            MigrationState::FilteredOut => write!(f, "Filtered Out"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum MigrationResult {
    AlreadyApplied,
    Success,
    Failure { reason: String },
    NothingToDo,
    IrreversibleMigration, // migration contains no "down" part.
    SkippedDueToEarlierError,
}

/// A Boxed runner helps us with the trait objects
/// and is practically a first-class citizen.
pub type BoxedRunner = Box<dyn Runner>;

/// Analog the `from_config` in MigrationStore trait, which however does
/// not box the StateStore result. Takes a runner::Configuration and
/// obeys the currently compiled in features to ensure that we have a single
/// point runner factory.
///
/// In this place synonyms are taken taken for the runner drivers.
pub fn from_config(
    c: &crate::config::Configuration,
    config_name: &str,
) -> Result<BoxedRunner, Error> {
    log::debug!(
        "Searching for runner {:?} in configured runners {:?}",
        config_name,
        c.configured_runners.keys(),
    );

    let rc = c
        .configured_runners
        .get(config_name)
        .ok_or(Error::NoConfigForRunner {
            name: config_name.to_string(),
        })?;

    #[cfg(feature = "runner_mysql")]
    log::trace!(
        "comparing {} to {} and {}",
        rc._runner.to_lowercase(),
        crate::reserved::MYSQL.to_lowercase(),
        crate::reserved::MARIA_DB.to_lowercase()
    );
    if rc._runner.to_lowercase() == crate::reserved::MYSQL.to_lowercase()
        || rc._runner.to_lowercase() == crate::reserved::MARIA_DB.to_lowercase()
    {
        log::info!("matched, returning a MySQL runner");
        return Ok(Box::new(mysql::runner::MySql::new_runner(rc.clone())?));
    }
    #[cfg(feature = "runner_postgres")]
    if rc._runner.to_lowercase() == crate::reserved::POSTGRESQL.to_lowercase() {
        return Ok(Box::new(postgresql::PostgreSql::new_runner(rc.clone())?));
    }
    log::error!(
        "There seems to be no avaiable (not compiled, not enabled) runner for {} (runner: {})",
        config_name,
        rc._runner,
    );
    Err(Error::CouldNotFindOrCreateRunner {
        config_name: config_name.to_string(),
    })
}

pub type MigrationTemplate = &'static str;
pub type MigrationFileExtension = &'static str;

pub trait Runner {
    fn new_runner(config: Configuration) -> Result<Self, Error>
    where
        Self: Sized;

    fn apply(&mut self, _: &MigrationStep) -> Result<(), Error>;

    /// Returns tuple with up, down and file extension for the migration
    fn migration_template(&self) -> (MigrationTemplate, MigrationTemplate, MigrationFileExtension);

    /// Provides metadata about this runner. Each runner implementation
    /// must implement this.
    fn meta(&self) -> RunnerMeta;
}
