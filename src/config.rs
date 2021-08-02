//! Contains configuration loading, validation and parsing code
//! Relies on [`yaml rust`] for parsing because `serde_yaml` does not support
//! [YAML anchors & tags](https://yaml.org/spec/1.2/spec.html#id2765878).

use super::reserved;
use crate::runner::Configuration as RunnerConfiguration;
use log::trace;
use std::collections::HashMap;
use std::fmt;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use yaml_rust::{Yaml, YamlLoader};

// This is evaluated relative to the configuration file, so the file makes sense
// in context of itself and doesn't depend so much on the runner's effective CWD.
pub const DEFAULT_MIGRATIONS_DIR: &str = ".";

// Most examples are using config.yml, but let's be honest, in a complicated
// polyglot project, we're probably not the only ones looking for that name!
pub const DEFAULT_CONFIG_FILE: &str = "mitre.yml";

#[derive(Debug)]
/// Describe problems with the loading, parsing and general processing
/// of the config. A valid YAML file may still produce an invalid config,
/// however those problems will be reported using [`fn.Configuration.validate`].
pub enum ConfigError {
    Io(io::Error),
    Yaml(yaml_rust::ScanError),
    NoYamlHash,
    GetStringError,
    /// If we would parse with `serde_yaml` all fields on the [`RunnerConfiguration`] would be [`Option<T>`], however
    /// we parse by hand, and we can specify that `_runner` is mandatory, which it is. Failing to provide it
    /// will cause an error. If a language binding is used, and the language provides the config, we may not have a parsing-time
    /// opportunity to notice this problem in the config file, so the [`ConfigProblem::UnsupportedRunnerSpecified`] may manifest
    /// (e.g if the language binding provides an empty string for the runner).
    NoRunnerSpecified {
        config_name: String,
    },
    /// YAML supports ["arbitrarily sized finite mathematical integers"](https://yaml.org/type/int.html) however we may not need or want that.
    /// Parsing a port number which is typically the range `(2^16)-1`, therefore we can constrain what we accept.
    ///
    /// The permitted range differs for example between [`struct.RunnerConfiguration.port`] and [`struct.RunnerConfiguration.ip_or_hostname`].
    IntegerOutOfRange {
        value: u64,
        max: u64,
    }, // value, max value
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &*self {
            // Underlying errors already impl `Display`, so we defer to
            // their implementations.
            ConfigError::Io(ref err) => write!(f, "IO error: {}", err),
            ConfigError::Yaml(ref err) => write!(f, "YAML error: {}", err),
            ConfigError::NoYamlHash => {
                write!(f, "YAML error: the top level doc in the yaml wasn't a hash")
            }
            ConfigError::NoRunnerSpecified { config_name } => {
                write!(f, "No runner specified in config block `{}'", config_name)
            }
            ConfigError::IntegerOutOfRange { value, max } => write!(
                f,
                "YAML error: value '{}' is out of range, max is '{}'",
                value, max
            ),
            ConfigError::GetStringError => {
                write!(f, "YAML error: get_string() passed-thru without match")
            }
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(err: io::Error) -> ConfigError {
        ConfigError::Io(err)
    }
}

impl From<yaml_rust::ScanError> for ConfigError {
    fn from(err: yaml_rust::ScanError) -> ConfigError {
        ConfigError::Yaml(err)
    }
}

// Inexhaustive list for now
#[derive(Debug, PartialEq)]
/// Describe problems with the configuration, may be useful in performing pre-flight
/// sanity checks on the configuration before trying to run things up.
pub enum ConfigProblem {
    /// All configurations are expected to specify a mitre: key. This mitre key
    /// is used to select the runner for the state store so that the library and tool
    /// can set-up a correct working environment. It is recommeneded to use YAML's
    /// "Anchors and Aliases" functions to specify a `mitre:\n<<: *myappdb` block when
    /// Mitre should store state in the same database to which we are applying migrations.
    NoMitreConfiguration,
    /// Unsupported runner specified. See [crate::reserved] for supported runners.
    UnsupportedRunnerSpecified,
    /// Certain databases (e.g ElasticSearch) have a concept of an index. This configuration option
    /// is exposed as `{indexName}` within the templates of migrations targeting a runner where this
    /// concept applies.
    NoIndexSpecified,
    /// Certain databases (e.g Redis) number their databases. This configuration option is exposed
    /// as `{databaseNumber}` within the templates of migrations targeting a runner where this concept
    /// applies.
    NoDatabaseNumberSpecified,
    /// Most all databases expect to be connected over a network or unix domain socket. If nothing is specified
    /// the underlying libraries may fall-back to a default such as `127.0.0.1` or similar. Prefer not to rely on
    /// any such behaviour and specify a proper hostname and port.
    NoIpOrHostnameSpecified,
    /// It is good practice to specify usernames. From development environments in increasing confusing
    /// contemporary network topologies, through cloud-based and shared (e.g public) environments. If not
    /// specified, libraries may default to connecting as the effective system user name of the process running
    /// Mitre which may lead to situations where Mitre works sometimes but not others, depending on who, and how it
    /// is called.
    NoUsernameSpecified,
    /// It is good practice to specify passwords. From development environments in increasing confusing
    /// contemporary network topologies, through cloud-based and shared (e.g public) environments.
    NoPasswordSpecified,
}

/// Alias for a String when using a configuration name, e.g "mitre" is expected to refer to a
/// key in the configured runners map which refers to a MySQL runner. ConfigurationName
/// is an important concept in case more than one MySQL runner is in the config.
pub type ConfigurationName = String;

#[derive(Debug, PartialEq, Clone)]
pub struct Configuration {
    pub migrations_directory: PathBuf,
    pub configured_runners: HashMap<ConfigurationName, RunnerConfiguration>,
}

impl Configuration {
    pub fn from_file(p: &Path) -> Result<Configuration, ConfigError> {
        from_file(p)
    }
    pub fn load_from_str(s: &str) -> Result<Configuration, ConfigError> {
        let yaml_docs = YamlLoader::load_from_str(s)?;
        from_yaml(yaml_docs)
    }

    pub fn validate(&self) -> Result<(), Vec<ConfigProblem>> {
        // TODO: write tests
        let mut problems = vec![];
        if self.configured_runners.get("mitre").is_none() {
            problems.push(ConfigProblem::NoMitreConfiguration)
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }

    pub fn get(&self, k: &str) -> Option<&RunnerConfiguration> {
        self.configured_runners.get(k)
    }
}

impl RunnerConfiguration {
    pub fn validate(&self) -> Result<(), Vec<ConfigProblem>> {
        let mut vec = Vec::new();

        if reserved::runner_by_name(&self._runner).is_none() {
            vec.push(ConfigProblem::UnsupportedRunnerSpecified);
        }

        if self._runner.to_lowercase() == reserved::REDIS.to_lowercase()
            && self.database_number.is_none()
        {
            vec.push(ConfigProblem::NoDatabaseNumberSpecified)
        }

        if !vec.is_empty() {
            Err(vec)
        } else {
            Ok(())
        }
    }
}

fn dig_yaml_value(yaml: &yaml_rust::Yaml, key: &str) -> Result<yaml_rust::Yaml, ConfigError> {
    match yaml {
        Yaml::Hash(ref map) => {
            for (k, v) in map {
                if as_string(k).eq(key) {
                    return Ok(v.clone());
                }
            }
        }
        _ => return Err(ConfigError::NoYamlHash),
    };
    Err(ConfigError::GetStringError)
}

fn dig_string(yaml: &yaml_rust::Yaml, key: &str) -> Option<String> {
    match dig_yaml_value(yaml, key) {
        Ok(Yaml::String(value)) => Some(value),
        _ => None {},
    }
}

fn dig_u8(yaml: &yaml_rust::Yaml, key: &str) -> Result<Option<u8>, ConfigError> {
    match dig_yaml_value(yaml, key) {
        Ok(Yaml::Integer(value)) => {
            if value > u8::MAX as i64 {
                return Err(ConfigError::IntegerOutOfRange {
                    value: value as u64,
                    max: u8::MAX as u64,
                });
            }
            Ok(Some(value as u8))
        }
        _ => Ok(None {}),
    }
}

fn dig_u16(yaml: &yaml_rust::Yaml, key: &str) -> Result<Option<u16>, ConfigError> {
    match dig_yaml_value(yaml, key) {
        Ok(Yaml::Integer(value)) => {
            if value > u16::MAX as i64 {
                return Err(ConfigError::IntegerOutOfRange {
                    value: value as u64,
                    max: u16::MAX as u64,
                });
            }
            Ok(Some(value as u16))
        }
        _ => Ok(None {}),
    }
}

fn as_string(yaml: &yaml_rust::Yaml) -> String {
    match yaml {
        yaml_rust::Yaml::String(yaml) => yaml.to_owned(),
        _ => String::from(""),
    }
}

pub fn from_file(p: &Path) -> Result<Configuration, ConfigError> {
    let s = std::fs::read_to_string(p)?;
    let yaml_docs = YamlLoader::load_from_str(&s)?;
    from_yaml(yaml_docs).map(|mut c| {
        if c.migrations_directory.is_relative() {
            c.migrations_directory = p.parent().unwrap().join(c.migrations_directory);
        }
        c
    })
}

pub fn default_config_to_file(p: &Path) -> Result<(), ConfigError> {
    let default_config = "
# Mitre Config
# This document describes the data stores that mitre runs migrations against
# as well as the data store that stores the migration table for mitre.
# Below you can find example configurations for the supported data stores:


my-mysql-db: &my-mysql-db
  _runner: mysql
  database: mitre
  ip_or_hostname: 127.0.0.1
  logLevel: debug
  password: example
  port: 3306
  username: root

# Curl Runner Data Stores can not be used as mitre config
my-elasticsearch:
  _runner: curl
  ip_or_hostname: es
  protocol: http
  logLevel: debug

# The key mitre signals that this data store is going to be used for mitres migration table
# It does not necessary need to be a data store you want to run migrations against, but it can be
mitre:
  <<: *my-mysql-db # using YAML anchors is optional but encouraged so no duplication is necessary
";

    std::fs::write(p, default_config).map_err(ConfigError::Io)
}

fn from_yaml(yaml_docs: Vec<yaml_rust::Yaml>) -> Result<Configuration, ConfigError> {
    let mut hm: HashMap<ConfigurationName, RunnerConfiguration> = HashMap::new();
    let mut mig_dir = DEFAULT_MIGRATIONS_DIR;
    match yaml_docs
        .iter()
        .filter_map(|yaml| {
            if let Yaml::Hash(ref map) = yaml {
                Some(map)
            } else {
                None
            }
        })
        .flat_map(|map| map.iter())
        .filter_map(|(k, v)| {
            if let Yaml::String(key) = k {
                if key.as_str() == "migrations_directory" {
                    trace!(
                        "setting migrations dir from entry in config file {:?} to {:?}",
                        key,
                        v
                    );
                    mig_dir = match v {
                        Yaml::String(value) => value,
                        _ => panic!("must be a string value for migrations_directory"),
                    }
                }
            };
            match v {
                Yaml::Hash(value) => {
                    let is_anchor = value.keys().find(|key| as_string(key).eq("<<"));
                    if is_anchor == None {
                        Some((k, v))
                    } else {
                        let anchor_element = value.iter().next(); // shows up as <<
                        let referenced_value = anchor_element.unwrap().1;
                        Some((k, referenced_value))
                    }
                }
                _ => {
                    // conditional prevents trace logging something misleading
                    // about the migrations_directory key, which we _do_ handle
                    // above.
                    if &Yaml::String(String::from("migrations_directory")) != k {
                        trace!("key {:?} ignored in config file", k);
                    }
                    None {}
                }
            }
        })
        .try_for_each(|(k, config_value)| {
            let c = RunnerConfiguration {
                _runner: match dig_string(config_value, &String::from("_runner")) {
                    Some(s) => s,
                    None => {
                        return Err(ConfigError::NoRunnerSpecified {
                            config_name: as_string(k),
                        })
                    }
                },
                database: dig_string(config_value, &String::from("database")),
                index: dig_string(config_value, &String::from("index")),
                database_number: match dig_u8(config_value, &String::from("database_number")) {
                    Ok(res) => res,
                    Err(e) => return Err(e),
                },
                ip_or_hostname: dig_string(config_value, &String::from("ip_or_hostname")),
                port: match dig_u16(config_value, &String::from("port")) {
                    Ok(res) => res,
                    Err(e) => return Err(e),
                },
                username: dig_string(config_value, &String::from("username")),
                password: dig_string(config_value, &String::from("password")),
            };
            hm.insert(as_string(k), c);
            Ok(())
        }) {
        Err(e) => Err(e),
        Ok(_) => Ok(Configuration {
            migrations_directory: PathBuf::from(mig_dir),
            configured_runners: hm,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use std::fs::File;
    use std::io::Write;
    use tempdir::TempDir;
    use tempfile::tempdir;
    use yaml_rust::YamlLoader;

    // -> fn validate_on_config_structs
    // unsupportted runner
    // use of reserved word out of place
    // dot separated parts not at end of filename

    #[test]
    fn dig_string_gets_string() -> Result<(), &'static str> {
        let yaml_docs = match YamlLoader::load_from_str(indoc! {r#"
          ---
          key: bestValue
        "#})
        {
            Ok(docs) => docs,
            _ => return Err("doc didn't parse"),
        };
        let v = match yaml_docs.first() {
            Some(document) => dig_string(document, &String::from("key")),
            _ => return Err("yaml docs was empty"),
        };
        assert_eq!(v, Some(String::from("bestValue")));
        Ok(())
    }

    #[test]
    fn dig_u8_gets_u8() -> Result<(), &'static str> {
        let yaml_docs = match YamlLoader::load_from_str(indoc! {r#"
          ---
          key: 255
        "#})
        {
            Ok(docs) => docs,
            _ => return Err("doc didn't parse"),
        };
        let v = match yaml_docs.first() {
            Some(document) => match dig_u8(document, &String::from("key")) {
                Ok(value) => value,
                _ => return Err("result didn't match"),
            },
            _ => return Err("yaml docs was empty"),
        };
        assert_eq!(v, Some(255));
        Ok(())
    }

    #[test]
    fn dig_u8_gets_returns_integer_error_on_overflow() -> Result<(), &'static str> {
        let yaml_docs = match YamlLoader::load_from_str(indoc! {r#"
          ---
          key: 2550000
        "#})
        {
            Ok(docs) => docs,
            _ => return Err("doc didn't parse"),
        };
        match yaml_docs.first() {
            Some(document) => match dig_u8(document, &String::from("key")) {
                Ok(_) => return Err("expected an error"),
                Err(e) => match e {
                    ConfigError::IntegerOutOfRange { value, max } => {
                        assert_eq!(value, 2550000 as u64);
                        assert_eq!(max, u8::MAX as u64);
                        return Ok(());
                    }
                    _ => return Err("wrong class of error returned"),
                },
            },
            _ => return Err("yaml docs was empty"),
        };
    }

    #[test]
    fn loads_a_complete_config() -> Result<(), &'static str> {
        let yaml_docs = match YamlLoader::load_from_str(indoc! {r#"
          ---
          migrations_dir: "./migrations/here"
          a:
            _runner: mysql
            database: mitre
            ip_or_hostname: 127.0.0.1
            logLevel: debug
            password: example
            port: 3306
            username: root
        "#})
        {
            Ok(docs) => docs,
            _ => return Err("doc didn't parse"),
        };

        let config = match from_yaml(yaml_docs) {
            Err(_) => return Err("failed to load doc"),
            Ok(config) => config,
        };

        let rc_config_a = RunnerConfiguration {
            _runner: String::from("mysql"),
            database: Some(String::from("mitre")),
            ip_or_hostname: Some(String::from("127.0.0.1")),
            // log_level: Some(String::from("debug")),
            password: Some(String::from("example")),
            port: Some(3306),
            username: Some(String::from("root")),
            database_number: None {},
            index: None {},
        };

        assert_eq!(1, config.configured_runners.keys().len());
        assert_eq!(rc_config_a, config.configured_runners["a"]);
        assert_eq!(
            PathBuf::from(DEFAULT_MIGRATIONS_DIR),
            config.migrations_directory
        );

        Ok(())
    }

    #[test]
    fn test_has_a_default_migrations_dir() -> Result<(), &'static str> {
        let yaml_docs = match YamlLoader::load_from_str(indoc! {r#"
          ---
          a:
            _runner: foobarbaz
        "#})
        {
            Ok(docs) => docs,
            _ => return Err("doc didn't parse"),
        };

        let config = match from_yaml(yaml_docs) {
            Err(_) => return Err("failed to load doc"),
            Ok(config) => config,
        };

        assert_eq!(
            PathBuf::from(DEFAULT_MIGRATIONS_DIR),
            config.migrations_directory
        );

        Ok(())
    }

    #[test]
    fn defaults_the_migrations_directory_relative_to_the_file_directory() -> Result<(), String> {
        let example_config = indoc! {r#"
          ---
          # migrations_directory: "." # is implied here because of the default value
          mitre:
            _runner: mysql
        "#};

        let tmp_dir = TempDir::new("example").expect("must be able to make tmpdir");
        let path = tmp_dir.path().join("config.yml").clone();

        let mut file = File::create(&path).expect("couldn't create tmpfile");
        file.write_all(example_config.as_bytes())
            .expect("coulnd't write file");

        match from_file(&path) {
            Ok(c) => {
                assert_eq!(&c.migrations_directory, tmp_dir.path());
                Ok(())
            }
            Err(e) => Err(format!("error is {}", e)),
        }
    }

    #[test]
    fn validates_presense_of_a_supported_runner() -> Result<(), &'static str> {
        let yaml_docs = match YamlLoader::load_from_str(indoc! {r#"
        ---
        a:
          _runner: foobarbaz
        "#})
        {
            Ok(docs) => docs,
            _ => return Err("doc didn't parse"),
        };

        let config = match from_yaml(yaml_docs) {
            Err(_) => return Err("failed to load doc"),
            Ok(config) => config,
        };

        let c = RunnerConfiguration {
            _runner: String::from("foobarbaz"),
            database: None {},
            ip_or_hostname: None {},
            password: None {},
            port: None {},
            username: None {},
            database_number: None {},
            index: None {},
        };

        assert_eq!(1, config.configured_runners.keys().len());
        assert_eq!(c, config.configured_runners["a"]);

        match c.validate() {
            Ok(_) => return Err("expected not-ok from validate"),
            Err(problems) => {
                if problems
                    .iter()
                    .any(|p| *p == ConfigProblem::UnsupportedRunnerSpecified)
                {
                    return Ok(());
                } else {
                    return Err("didn't find expected UnsupportedRunnerSpecified problem");
                }
            }
        }
    }

    #[test]
    fn default_config_is_valid() -> Result<(), String> {
        // Create test directory
        let tmp_dir = tempdir().map_err(|e| format!("Could not create a tmp dir: {}", e))?;
        let file_path = tmp_dir.path().join("config.yaml");
        let p = file_path
            .to_str()
            .ok_or("Could not get file path of tmp config.yaml")?;

        // Create default config in test dir
        default_config_to_file(Path::new(p))
            .map_err(|err| format!("Could not create default config: {}", err))?;

        // Load default config from test dir
        from_file(Path::new(p)).map_err(|err| format!("Could not load default config: {}", err))?;

        // Delete test directory
        tmp_dir
            .close()
            .map_err(|e| format!("Could not close tmp dir: {}", e))?;
        Ok(())
    }
}
