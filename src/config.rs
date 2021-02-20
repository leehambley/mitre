extern crate yaml_rust;

use super::reserved;
use std::collections::HashMap;
use std::error;
use std::fmt;
use std::io;
use std::path::Path;
use std::process::Command;
use yaml_rust::{Yaml, YamlLoader};

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    Yaml(yaml_rust::ScanError),
    NoYamlHash(),
    // ValueForKeyIsNotString(String),
    // ValueForKeyIsNotInteger(String),
    GetStringError(),
    IntegerOutOfRange { value: u64, max: u64 }, // value, max value
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            // Underlying errors already impl `Display`, so we defer to
            // their implementations.
            ConfigError::Io(ref err) => write!(f, "MITRE: IO error: {}", err),
            ConfigError::Yaml(ref err) => write!(f, "MITRE: YAML error: {}", err),
            ConfigError::NoYamlHash() => write!(
                f,
                "MITRE: YAML error: the top level doc in the yaml wasn't a hash"
            ),
            // ConfigError::ValueForKeyIsNotString(ref s) => {
            //     write!(f, "Mitre: YAML error: value at key '{}' is not a string", s)
            // }
            // ConfigError::ValueForKeyIsNotInteger(ref s) => write!(
            //     f,
            //     "Mitre: YAML error: value at key '{}' is not an integer",
            //     s
            // ),
            ConfigError::IntegerOutOfRange { value, max } => write!(
                f,
                "Mitre: YAML error: value '{}' is out of range, max is '{}'",
                value, max
            ),
            ConfigError::GetStringError() => write!(
                f,
                "Mitre: YAML error: get_string() passed-thru without match"
            ),
        }
    }
}

impl error::Error for ConfigError {
    fn cause(&self) -> Option<&dyn error::Error> {
        match *self {
            // N.B. implicitly cast `err` from their concrete
            // types (either `&io::Error` or `&num::ParseIntError`)
            // to a trait object `&Error`. This works because both error types
            // implement `Error`.
            ConfigError::Io(ref err) => Some(err),
            ConfigError::Yaml(ref err) => Some(err),
            ConfigError::NoYamlHash() => None {},
            // ConfigError::ValueForKeyIsNotString(_) => None {},
            // ConfigError::ValueForKeyIsNotInteger(_) => None {},
            ConfigError::GetStringError() => None {},
            ConfigError::IntegerOutOfRange { value: _, max: _ } => None {},
        }
    }
}

impl From<io::Error> for ConfigError {
    fn from(err: io::Error) -> ConfigError {
        ConfigError::Io(err)
    }
}

// impl From<serde_yaml::Error> for ConfigError {
//   fn from(err: serde_yaml::Error) -> ConfigError {
//     ConfigError::Yaml(err)
//   }
// }

impl From<yaml_rust::ScanError> for ConfigError {
    fn from(err: yaml_rust::ScanError) -> ConfigError {
        ConfigError::Yaml(err)
    }
}

// Inexhaustive list for now
#[derive(Debug, PartialEq)]
pub enum ConfigProblem {
    NoMitreConfiguration,
    NoRunnerSpecified,
    UnsupportedRunnerSpecified,
    NoIndexSpecified,
    NoDatabaseNumberSpecified,
    NoIpOrHostnameSpecified,
    NoUsernameSpecified,
    NoPasswordSpecified,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Configuration {
    pub configured_runners: HashMap<String, RunnerConfiguration>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct RunnerConfiguration {
    // Runner is not optional, but we need to option it here to maintain
    // serde::Deserialize compatibility
    pub _runner: Option<String>,

    pub database: Option<String>, // used by MariaDB, MySQL, PostgreSQL runners

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

impl Configuration {
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

        if self._runner.is_none() {
            vec.push(ConfigProblem::NoRunnerSpecified);
            return Err(vec);
        }

        if reserved::runner_by_name(self._runner.as_ref()).is_none() {
            vec.push(ConfigProblem::UnsupportedRunnerSpecified);
        }

        if self
            ._runner
            .clone()
            .map(|r| r.to_lowercase() == reserved::REDIS.to_lowercase())
            .is_some()
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

/// Reads patterns to exclude from the .gitignore file, an excludesfile
/// if configured locally or globally. Requires `git` to be on the Path
/// which is a safe bet.
///
/// https://docs.github.com/en/github/using-git/ignoring-files
//
// TODO Ensure this works on Windows?
// TODO extract in a library?
pub fn ignore_patterns() -> Result<Vec<String>, io::Error> {
    let unshared_excludesfile = String::from(".git/info/exclude");
    let default_excludesfile = String::from(".gitignore");
    let local_excludesfile = Command::new("git")
        .arg("config")
        .arg("core.excludesfile")
        .output()
        .expect("failed to execute process");
    let global_excludesfile = Command::new("git")
        .arg("config")
        .arg("core.excludesfile")
        .output()
        .expect("failed to execute process");

    let global_excludes = String::from_utf8(global_excludesfile.stdout)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    let local_excludes = String::from_utf8(local_excludesfile.stdout)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

    //.filter_map( |s| s.map(|s| Path::from(s) )).collect()
    Ok(vec![
        global_excludes,
        default_excludesfile,
        local_excludes,
        unshared_excludesfile,
    ])
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
        _ => return Err(ConfigError::NoYamlHash()),
    };
    Err(ConfigError::GetStringError())
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
    Ok(Configuration {
        configured_runners: from_yaml(yaml_docs)?,
    })
}

fn from_yaml(
    yaml_docs: Vec<yaml_rust::Yaml>,
) -> Result<HashMap<String, RunnerConfiguration>, ConfigError> {
    let mut hm: HashMap<String, RunnerConfiguration> = HashMap::new();
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
            if let Yaml::Hash(value) = v {
                let is_anchor = value.keys().find(|key| as_string(key).eq("<<"));
                if is_anchor == None {
                    Some((k, v))
                } else {
                    let anchor_element = value.iter().next(); // shows up as <<
                    let referenced_value = anchor_element.unwrap().1;
                    Some((k, referenced_value))
                }
            } else {
                None
            }
        })
        .try_for_each(|(k, config_value)| {
            let c = RunnerConfiguration {
                _runner: dig_string(config_value, &String::from("_runner")),
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
        Ok(_) => Ok(hm),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

    // -> fn validate_on_config_structs
    // unsupportted runner
    // use of reserved word out of place
    // dot separated parts not at end of filename

    #[test]
    fn dig_string_gets_string() -> Result<(), &'static str> {
        let yaml_docs = match YamlLoader::load_from_str(
            r#"
---
key: bestValue
      "#,
        ) {
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
        let yaml_docs = match YamlLoader::load_from_str(
            r#"
---
key: 255
      "#,
        ) {
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
        let yaml_docs = match YamlLoader::load_from_str(
            r#"
---
key: 2550000
      "#,
        ) {
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
        let yaml_docs = match YamlLoader::load_from_str(
            r#"
---
a:
  _runner: mysql
  database: mitre
  ip_or_hostname: 127.0.0.1
  logLevel: debug
  password: example
  port: 3306
  username: root
"#,
        ) {
            Ok(docs) => docs,
            _ => return Err("doc didn't parse"),
        };

        let configs = match from_yaml(yaml_docs) {
            Err(_) => return Err("failed to load doc"),
            Ok(configs) => configs,
        };

        let c = RunnerConfiguration {
            _runner: Some(String::from("mysql")),
            database: Some(String::from("mitre")),
            ip_or_hostname: Some(String::from("127.0.0.1")),
            // log_level: Some(String::from("debug")),
            password: Some(String::from("example")),
            port: Some(3306),
            username: Some(String::from("root")),
            database_number: None {},
            index: None {},
        };

        assert_eq!(1, configs.keys().len());
        assert_eq!(c, configs["a"]);

        Ok(())
    }

    #[test]
    fn validates_presense_of_a_supported_runner() -> Result<(), &'static str> {
        let yaml_docs = match YamlLoader::load_from_str(
            r#"
---
a:
  _runner: foobarbaz
"#,
        ) {
            Ok(docs) => docs,
            _ => return Err("doc didn't parse"),
        };

        let configs = match from_yaml(yaml_docs) {
            Err(_) => return Err("failed to load doc"),
            Ok(configs) => configs,
        };

        let c = RunnerConfiguration {
            _runner: Some(String::from("foobarbaz")),
            database: None {},
            ip_or_hostname: None {},
            password: None {},
            port: None {},
            username: None {},
            database_number: None {},
            index: None {},
        };

        assert_eq!(1, configs.keys().len());
        assert_eq!(c, configs["a"]);

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
}
