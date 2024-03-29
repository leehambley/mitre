use super::{ConfigurationName, Error, Flag};
use core::cmp::Ordering;
use itertools::Itertools;
use std::collections::HashMap;
use std::convert::From;
use std::path::PathBuf;

pub mod built_in_migrations;

pub const FORMAT_STR: &str = "%Y%m%d%H%M%S";

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Direction {
    Up,
    Down,
    Change,
}

impl From<String> for Direction {
    fn from(s: String) -> Self {
        match s.as_str() {
            "up" => Direction::Up,
            "down" => Direction::Down,
            "change" => Direction::Change,
            _ => panic!("Unknown direction {:#?}", s),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MigrationStep {
    pub path: PathBuf,
    pub source: String,
}

impl Eq for MigrationStep {}

impl PartialEq for MigrationStep {
    fn eq(&self, other: &Self) -> bool {
        (self.path == other.path) && (self.source == other.source)
    }
}

impl<'a> PartialOrd for MigrationStep {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.source.cmp(&other.source))
    }
}

pub type MigrationSteps = HashMap<Direction, MigrationStep>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    pub date_time: chrono::NaiveDateTime,
    pub steps: MigrationSteps,
    pub built_in: bool,
    pub flags: Vec<Flag>,
    pub configuration_name: ConfigurationName,
}

impl Migration {
    pub fn version(&self) -> String {
        self.date_time.format(FORMAT_STR).to_string()
    }
    pub fn flags_as_string(&self) -> String {
        self.flags.iter().map(|f| f.name).join(",")
    }
    pub fn validate(&self) -> Option<Error> {
        match (
            self.steps.get(&Direction::Up),
            self.steps.get(&Direction::Change),
        ) {
            (Some(_), Some(_)) => Some(Error::MalformedMigration),
            _ => None,
        }
    }
}

/// Implementaion of [`PartialOrd`] for [`Migration`] to ensure that
/// they are sortable by datetime.
impl PartialOrd for Migration {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.date_time.cmp(&other.date_time))
    }
}

impl Ord for Migration {
    fn cmp(&self, other: &Self) -> Ordering {
        self.date_time.cmp(&other.date_time)
    }
}

impl MigrationStep {
    pub fn content(&self) -> Result<mustache::Template, mustache::Error> {
        mustache::compile_str(&self.source)
    }
}
