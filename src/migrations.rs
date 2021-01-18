// #[cfg(not (test))]
use crate::filename;
// use ::phf::{phf_map, Map};
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

#[derive(Debug, PartialEq, Clone)]
pub enum Direction {
    Up,
    Down,
    Change,
}

#[derive(Debug, Clone)]
pub struct Migration {
    pub parsed: filename::Parsed,
    pub direction: Direction,
}

// static ICONS: Map<Direction, &'static str> = phf_map! {
//   Direction::Up => "⬆",
//   Direction::Down => "⬇",
//   Direction::Change => "⭬",
// };

// TODO: anything to document here about max depth, and or/whether we
// traverse filesystems, or whether there is a timeout (e.g slow network share)
pub fn migrations(dir: &Path) -> Result<Vec<Migration>, MigrationListingError> {
    let mut migrations: Vec<Migration> = Vec::new();
    for entry in fs::read_dir(dir)? {
        match filename::parse(&entry?.path()) {
            Ok(file_name) => migrations.push(Migration {
                parsed: file_name,
                direction: Direction::Change,
            }),
            Err(e) => {
                trace!("{:?}", e);
                continue;
            }
        };
    }
    Ok(migrations)
}

#[cfg(test)]
mod tests {

    // use super::*;

    // unsupportted runner
    // use of reserved word out of place
    // dot separated parts not at end of filename
}
