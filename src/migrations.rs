// #[cfg(not (test))]
use super::filename;
use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct MigrationListingError;

impl fmt::Display for MigrationListingError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "error listing files")
  }
}

impl From<io::Error> for MigrationListingError {
  fn from(_err: io::Error) -> MigrationListingError {
    MigrationListingError {}
  }
}

#[derive(Debug)]
pub struct Migration {
  pub parsed: filename::Parsed,
}

// TODO: anything to document here about max depth, and or/whether we
// traverse filesystems, or whether there is a timeout (e.g slow network share)
pub fn migrations(dir: &Path) -> Result<Vec<Migration>, MigrationListingError> {
  eprintln!("listing stuff in {:?}", dir);
  let mut migrations: Vec<Migration> = Vec::new();
  for entry in fs::read_dir(dir)? {
    eprintln!("fof fo f {:?}", entry);
    match filename::parse(&entry?.path()) {
      Some(file_name) => migrations.push(Migration { parsed: file_name }),
      None => {
        eprintln!("continuing");
        continue;
      }
    };
  }
  return Ok(migrations);
}

#[cfg(test)]
mod tests {

  use super::*;

  // unsupportted runner
  // use of reserved word out of place
  // dot separated parts not at end of filename
}
