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

pub struct Configuration {}

pub fn from_file(p: &Path) -> Result<HashMap<String, Configuration>, ConfigError> {
  // File doesn't exist
  // File isn't a file
  // File isn't readable
  // File isn't YAML
  // File isn't _valid_ YAML
  let hm = HashMap::new();

  let f = std::fs::File::open(p)?;
  let d: String = serde_yaml::from_reader(f)?;
  println!("Read YAML string: {}", d);
  Ok(hm) // Ok(serde_yaml::from_reader(f))
}

#[cfg(test)]
mod tests {

  // use super::*;

  // unsupportted runner
  // use of reserved word out of place
  // dot separated parts not at end of filename
}
