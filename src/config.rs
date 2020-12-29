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

// TODO: validate at least one `mitre` config with a compatible runner in the HashMap<String,...>

pub fn from_file(p: &Path) -> Result<HashMap<String, Configuration>, ConfigError> {
  // TODO: File doesn't exist
  // TODO: File isn't a file
  // TODO: File isn't readable
  // TODO: File isn't YAML
  // TODO: File isn't _valid_ YAML
  let s = std::fs::read_to_string(p)?;
  let yaml_docs = YamlLoader::load_from_str(&s)?;

  let hm: HashMap<String, Configuration> = HashMap::new();
  for yaml in yaml_docs {
    match yaml {
      Yaml::Hash(ref map) => {
        for (k, v) in map {
          println!("k: {:?} === v: {:?}", k, v);
        }
      }
      _ => {
        warn!("unexpected type at top level of YAML");
        return Err(ConfigError::NoYamlHash {});
      }
    }
    // println!(">>> yaml is {:?}", yaml);
    // let _ = yaml.into_iter().map(|item| {
    //   println!("item: {:?}", item);
    // });
    println!("after iter");
    // for key in yaml_docs[yaml].into_iter() {
    //   println!("a yaml {:?}", yaml);
    // }
  }
  // let hm: HashMap<String, Configuration> = serde_yaml::from_reader(f)?;
  // println!("Read YAML string: {:?}", hm);
  Ok(hm) // Ok(serde_yaml::from_reader(f))
}

#[cfg(test)]
mod tests {

  // use super::*;

  // unsupportted runner
  // use of reserved word out of place
  // dot separated parts not at end of filename
}
