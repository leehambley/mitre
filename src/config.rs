extern crate yaml_rust;
use std::collections::HashMap;
use std::error;
use std::fmt;
use std::io;
use std::path::Path;
use yaml_rust::{Yaml, YamlLoader};

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    Yaml(yaml_rust::ScanError),
    NoYamlHash(),
    ValueForKeyIsNotString(String),
    ValueForKeyIsNotInteger(String),
    GetStringError(),
    IntegerOutOfRange(u64, u64), // value, max value
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
            ConfigError::ValueForKeyIsNotString(ref s) => {
                write!(f, "Mitre: YAML error: value at key '{}' is not a string", s)
            }
            ConfigError::ValueForKeyIsNotInteger(ref s) => {
                write!(
                    f,
                    "Mitre: YAML error: value at key '{}' is not an integer",
                    s
                )
            }
            ConfigError::IntegerOutOfRange(ref v, ref max) => {
                write!(
                    f,
                    "Mitre: YAML error: value '{}' is out of range, max is '{}'",
                    v, max
                )
            }
            ConfigError::GetStringError() => write!(
                f,
                "MITRE: YAML error: get_string() passed-thru without match"
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
            ConfigError::ValueForKeyIsNotString(_) => None {},
            ConfigError::ValueForKeyIsNotInteger(_) => None {},
            ConfigError::GetStringError() => None {},
            ConfigError::IntegerOutOfRange(_, _) => None {},
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

//
// Load YAML using serde-yaml,
//
#[derive(Debug)]
pub struct Configuration {
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

fn dig_yaml_value(yaml: &yaml_rust::Yaml, key: &String) -> Result<yaml_rust::Yaml, ConfigError> {
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

fn dig_string(yaml: &yaml_rust::Yaml, key: &String) -> Result<Option<String>, ConfigError> {
    match dig_yaml_value(yaml, key) {
        Ok(Yaml::String(value)) => return Ok(Some(value.to_string())),
        _ => return Err(ConfigError::ValueForKeyIsNotString(key.to_string())),
    }
}

fn dig_u8(yaml: &yaml_rust::Yaml, key: &String) -> Result<Option<u8>, ConfigError> {
    match dig_yaml_value(yaml, key) {
        Ok(Yaml::Integer(value)) => {
            if value > u8::MAX as i64 {
                return Err(ConfigError::IntegerOutOfRange(value as u64, u8::MAX as u64));
            }
            return Ok(Some(value as u8));
        }
        _ => return Err(ConfigError::ValueForKeyIsNotInteger(key.to_string())),
    }
}

fn dig_u16(yaml: &yaml_rust::Yaml, key: &String) -> Result<Option<u16>, ConfigError> {
    match dig_yaml_value(yaml, key) {
        Ok(Yaml::Integer(value)) => {
            if value > u16::MAX as i64 {
                return Err(ConfigError::IntegerOutOfRange(
                    value as u64,
                    u16::MAX as u64,
                ));
            }
            return Ok(Some(value as u16));
        }
        _ => return Err(ConfigError::ValueForKeyIsNotInteger(key.to_string())),
    }
}

fn as_string(yaml: &yaml_rust::Yaml) -> String {
    match yaml {
        yaml_rust::Yaml::String(yaml) => yaml.to_owned(),
        _ => String::from(""),
    }
}

// TODO: validate at least one `mitre` config with a compatible runner in the HashMap<String,...>

pub fn from_file(p: &Path) -> Result<HashMap<String, Configuration>, ConfigError> {
    // TODO: File doesn't exist
    // TODO: File isn't a file
    // TODO: File isn't readable
    // TODO: File isn't YAML
    // TODO: File isn't _valid_ YAML
    let s = std::fs::read_to_string(p)?;
    let yaml_docs = YamlLoader::load_from_str(&s)?;

    let mut hm: HashMap<String, Configuration> = HashMap::new();

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
            let c = Configuration {
                _runner: match dig_string(config_value, &String::from("_runner")) {
                    Ok(res) => res,
                    Err(e) => return Err(e),
                },
                database: match dig_string(config_value, &String::from("database")) {
                    Ok(res) => res,
                    Err(e) => return Err(e),
                },
                index: match dig_string(config_value, &String::from("index")) {
                    Ok(res) => res,
                    Err(e) => return Err(e),
                },
                database_number: match dig_u8(config_value, &String::from("port")) {
                    Ok(res) => res,
                    Err(e) => return Err(e),
                },
                ip_or_hostname: match dig_string(config_value, &String::from("ip_or_hostname")) {
                    Ok(res) => res,
                    Err(e) => return Err(e),
                },
                port: match dig_u16(config_value, &String::from("port")) {
                    Ok(res) => res,
                    Err(e) => return Err(e),
                },
                username: match dig_string(config_value, &String::from("username")) {
                    Ok(res) => res,
                    Err(e) => return Err(e),
                },
                password: match dig_string(config_value, &String::from("password")) {
                    Ok(res) => res,
                    Err(e) => return Err(e),
                },
            };
            hm.insert(as_string(k), c);
            Ok(())
        }) {
        Err(e) => return Err(e),
        Ok(_) => return Ok(hm),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

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
            Ok(doc) => doc,
            _ => return Err("doc didn't parse"),
        };
        let v = match yaml_docs.first() {
            Some(document) => match dig_string(document, &String::from("key")) {
                Ok(value) => value,
                _ => return Err("result didn't match"),
            },
            _ => return Err("dig_string returned nothing"),
        };
        assert_eq!(v, Some(String::from("bestValue")));
        Ok(())
    }

    fn dig_u8_gets_u8() -> Result<(), &'static str> {
        let yaml_docs = match YamlLoader::load_from_str(
            r#"
---
key: 255
      "#,
        ) {
            Ok(doc) => doc,
            _ => return Err("doc didn't parse"),
        };
        let v = match yaml_docs.first() {
            Some(document) => match dig_u8(document, &String::from("key")) {
                Ok(value) => value,
                _ => return Err("result didn't match"),
            },
            _ => return Err("dig_u8 returned nothing"),
        };
        assert_eq!(v, Some(255));
        Ok(())
    }

    fn dig_u8_gets_truncates_to_u8() -> Result<(), &'static str> {
        let yaml_docs = match YamlLoader::load_from_str(
            r#"
---
key: 255000
      "#,
        ) {
            Ok(doc) => doc,
            _ => return Err("doc didn't parse"),
        };
        let v = match yaml_docs.first() {
            Some(document) => match dig_u8(document, &String::from("key")) {
                Ok(value) => value,
                _ => return Err("result didn't match"),
            },
            _ => return Err("dig_u8 returned nothing"),
        };
        assert_eq!(v, Some(255));
        Ok(())
    }
}
