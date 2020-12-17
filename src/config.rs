use serde::Deserialize;
use std::collections::HashMap;
use std::error;
use std::fmt;
use std::io;
use std::path::Path;

#[derive(Debug)]
pub enum ConfigError {
  Io(io::Error),
  Yaml(serde_yaml::Error),
}

impl fmt::Display for ConfigError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match *self {
      // Underlying errors already impl `Display`, so we defer to
      // their implementations.
      ConfigError::Io(ref err) => write!(f, "IO error: {}", err),
      ConfigError::Yaml(ref err) => write!(f, "YAML error: {}", err),
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
    }
  }
}

impl From<io::Error> for ConfigError {
  fn from(err: io::Error) -> ConfigError {
    ConfigError::Io(err)
  }
}

impl From<serde_yaml::Error> for ConfigError {
  fn from(err: serde_yaml::Error) -> ConfigError {
    ConfigError::Yaml(err)
  }
}

//
// Load YAML using serde-yaml,
//
#[derive(Deserialize, Debug)]
pub struct Configuration {
  runner: String,

  database: Option<String>, // used by MariaDB, MySQL, PostgreSQL runners

  index: Option<String>, // used by ElasticSearch

  database_number: Option<u8>, // used by Redis runner

  // Maybe this should have another name, we also would
  // probably accept IPs or anything resolveable here.
  hostname: Option<String>, // used by cURL, MySQL, Redis, MySQL, PostgreSQL, ElasticSearch

  // Max value for port on linux comes from `cat /proc/sys/net/ipv4/ip_local_port_range`
  // u16 should be enough for most people most of the time.
  port: Option<u16>, // used by cURL, MySQL, Redis, MySQL, PostgreSQL, ElasticSearch
}

pub fn from_file(p: &Path) -> Result<HashMap<String, Configuration>, ConfigError> {
  // File doesn't exist
  // File isn't a file
  // File isn't readable
  // File isn't YAML
  // File isn't _valid_ YAML
  let f = std::fs::File::open(p)?;
  let hm: HashMap<String, Configuration> = serde_yaml::from_reader(f)?;
  println!("Read YAML string: {:?}", hm);
  Ok(hm) // Ok(serde_yaml::from_reader(f))
}

#[cfg(test)]
mod tests {

  // use super::*;

  // unsupportted runner
  // use of reserved word out of place
  // dot separated parts not at end of filename
}
