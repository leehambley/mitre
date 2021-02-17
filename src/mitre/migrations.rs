extern crate mustache;

use super::reserved::{runners, Runner};
use regex;
use rust_embed::RustEmbed;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(RustEmbed)]
#[folder = "src/migrations/"]
#[prefix = "src/migrations/"]
struct BuiltInMigrations;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Direction {
    Up,
    Down,
    Change,
}

// static ICONS: Map<Direction, &'static str> = phf_map! {
//   Direction::Up => "⬆",
//   Direction::Down => "⬇",
//   Direction::Change => "⭬",
// };

pub const FORMAT_STR: &str = "%Y%m%d%H%M%S";

#[derive(Debug)]
pub enum MigrationsError {
    IO(io::Error),
    Mustache(mustache::Error),
}

impl From<io::Error> for MigrationsError {
    fn from(err: io::Error) -> MigrationsError {
        MigrationsError::IO(err)
    }
}

impl From<mustache::Error> for MigrationsError {
    fn from(err: mustache::Error) -> MigrationsError {
        MigrationsError::Mustache(err)
    }
}

#[derive(Debug, Clone)]
pub struct MigrationStep {
    pub path: PathBuf,
    pub content: mustache::Template,
    pub source: String,
    pub runner: Runner, // runners are compiled-in
}

#[derive(Debug, Clone)]
pub struct Migration {
    pub date_time: chrono::NaiveDateTime,
    pub steps: HashMap<Direction, MigrationStep>,
    pub built_in: bool,
}

impl Migration {
    fn new(
        date_time: chrono::NaiveDateTime,
        steps: HashMap<Direction, MigrationStep>,
    ) -> Migration {
        Migration {
            date_time,
            steps,
            built_in: false,
        }
    }
}

/// List all migrations known in the given context.
///
/// Returns a lazy iterator, or some wrapped error from std::io
/// or the template library (Mustache).
///
/// Order of the returned migrations is not guaranteed as the filesytem
/// walk cannot be guaranteed to run a specific way, also depending on
/// system locale the built-in migrations (Mitre's own migration management migrations)
/// may be interspersed.
///
/// Runners must *run* these in chronological order to maintain the library
/// guarantees, so the lazy iterator is used more for it's neat interface
/// and composability than any specific optimization reason.
///
/// Ideally provide an absolute path.
pub fn migrations(p: &Path) -> Result<Vec<Migration>, MigrationsError> {
    let mut m = built_in_migrations();
    &m.extend(migrations_in(p)?);
    Ok(m)
}

// https://rust-lang-nursery.github.io/rust-cookbook/file/dir.html
// This should take an *absolute* path
fn migrations_in(p: &Path) -> Result<Vec<Migration>, MigrationsError> {
    Ok(WalkDir::new(p)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            match extract_timestamp(entry.path().to_path_buf()) {
                Ok(timestamp) => {
                    let path_buf = entry.path().to_path_buf();
                    if entry.file_type().is_dir() {
                        match parts_in_migration_dir(path_buf.clone()).ok()? {
                            Some(parts) => Some(Migration {
                                date_time: timestamp,
                                steps: parts,
                                built_in: false,
                            }),
                            _ => None {},
                        }
                    } else {
                        let mut f = File::open(path_buf.clone()).ok()?;
                        let mut buffer = String::new();
                        f.read_to_string(&mut buffer).ok()?;
                        match part_from_migration_file(path_buf.clone(), &buffer).ok()? {
                            Some(parts) => Some(Migration {
                                date_time: timestamp,
                                steps: parts,
                                built_in: false,
                            }),
                            _ => None {},
                        }
                    }
                }
                Err(_) => None {}, // err contains a string reason why from timestamp parser, ignore it
            }
        })
        .collect())
}

// WARNING: Built-in migrations do not support the up/down director
//          style of migration yet. Please stick to "change" only files
fn built_in_migrations() -> Vec<Migration> {
    BuiltInMigrations::iter()
        .filter_map(|file| {
            let p = PathBuf::from(file.as_ref());
            let bytes = BuiltInMigrations::get(file.as_ref()).unwrap();
            let contents = std::str::from_utf8(&bytes).ok().unwrap();

            match extract_timestamp(p.clone()) {
                Ok(timestamp) => match part_from_migration_file(p, contents).ok()? {
                    Some(parts) => Some(Migration {
                        date_time: timestamp,
                        steps: parts,
                        built_in: true,
                    }),
                    _ => None {},
                },
                Err(_) => panic!("built-in migration has bogus filename"),
            }
        })
        .collect()
}

fn runner_reserved_word_from_str(s: &&str) -> Option<Runner> {
    runners().find(|word| word.exts.contains(s))
}

pub fn part_from_migration_file(
    p: PathBuf,
    c: &str,
) -> Result<Option<HashMap<Direction, MigrationStep>>, MigrationsError> {
    let parts = p
        .to_str()
        .unwrap()
        .split(|x| x == std::path::MAIN_SEPARATOR || x == '_' || x == '.');

    // TODO: warn if more than one runner found?
    let runner: Option<Runner> = parts
        .filter_map(|p| runner_reserved_word_from_str(&p))
        .take(1)
        .next();

    let t = mustache::compile_str(c)?;

    match runner {
        Some(r) => Ok(Some(
            vec![(
                Direction::Change,
                MigrationStep {
                    content: t,
                    source: String::from(c),
                    path: p,
                    runner: r,
                },
            )]
            .into_iter()
            .collect(),
        )),
        None => Ok(None {}),
    }
}

fn parts_in_migration_dir(
    p: PathBuf,
) -> Result<Option<HashMap<Direction, MigrationStep>>, MigrationsError> {
    fn has_proper_name(p: PathBuf) -> Option<(Direction, MigrationStep)> {
        let (up, down) = (OsStr::new("up"), OsStr::new("down"));
        // TODO: warn on `change` direction in a directory
        let direction = match p.file_stem().unwrap_or(OsStr::new("")) {
            up => Some(Direction::Up),
            down => Some(Direction::Down),
        };
        let runner = match p.extension() {
            Some(e) => runner_reserved_word_from_str(&e.to_str().unwrap()),
            None => None {},
        };

        // TODO: bubble errors and do more with pattern matching
        // here to avoid .ok()? ok.
        let mut f = File::open(&p).ok()?;
        let mut buffer = String::new();
        f.read_to_string(&mut buffer).ok()?;

        let template = mustache::compile_str(&buffer).ok()?;
        match (direction, runner) {
            (Some(d), Some(r)) => Some((
                d,
                MigrationStep {
                    content: template,
                    source: buffer,
                    path: p,
                    runner: r,
                },
            )),
            _ => None {},
        }
    }
    Ok(Some(
        fs::read_dir(p)?
            .filter_map(|res| res.map(|e| e.path()).ok())
            .filter_map(has_proper_name)
            .collect(),
    ))
}

fn extract_timestamp(p: PathBuf) -> Result<chrono::NaiveDateTime, &'static str> {
    // Search for "SEPARATOR\d{14}_[^SEPARATOR]+$" (dir separator, 14 digits, underscore, no separator until the end)
    // Note: cannot use FORMAT_STR.len() here because %Y is 2 chars, but wants 4 for example.
    let re = regex::Regex::new(
        format!(
            r#"{}(\d{{14}})_[^{}]+$"#,
            regex::escape(format!("{}", std::path::MAIN_SEPARATOR).as_str()),
            regex::escape(format!("{}", std::path::MAIN_SEPARATOR).as_str())
        )
        .as_str(),
    )
    .unwrap();
    match re.captures(p.to_str().expect("path to_str failed")) {
        None => Err("pattern did not match"),
        Some(c) => match c.get(1) {
            Some(m) => match chrono::NaiveDateTime::parse_from_str(m.as_str(), FORMAT_STR) {
                Ok(ndt) => Ok(ndt),
                Err(_) => Err("timestamp did not parse"),
            },
            None => Err("no capture group"),
        },
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn text_extract_timestamp() -> Result<(), &'static str> {
        let p = PathBuf::from("test/fixtures/example-1-simple-mixed-migrations/migrations/20200904205000_get_es_health.es-docker.curl");
        extract_timestamp(p)?;

        let p = PathBuf::from(
            "test/fixtures/example-1-simple-mixed-migrations/migrations/example.curl",
        );
        match extract_timestamp(p) {
            Ok(_) => Err("should not have extracted anything"),
            _ => Ok(()),
        }
    }

    #[test]
    fn test_extract_timestamp_no_match_files_in_migration_dirs() -> Result<(), String> {
        // With a walkdir (as we use) it's possible to pass through
        // a path such as p1 twice, at the dir level, and at the file level
        // the walkdir is _not_ recursing, so we can't traverse, we walk.
        //
        // For that reason it is important not to detect a timstamp in
        // the files in a timestamped dir
        let p1 =
            PathBuf::from("migrations/20210119200000_new_year_new_migration.es-postgres/up.sql");
        match extract_timestamp(p1) {
            Ok(_) => Err(format!("should not have matched").to_string()),
            Err(e) => match e {
                "pattern did not match" => Ok(()),
                _ => Err(format!("Unexpected err from timestamp extractor: {:?}", e)),
            },
        }
    }

    #[test]
    fn test_extract_timestamp_match_migration_dirs() -> Result<(), String> {
        // With a walkdir (as we use) it's possible to pass through
        // a path such as p1 twice, at the dir level, and at the file level
        // the walkdir is _not_ recursing, so we can't traverse, we walk.
        //
        // For that reason it is important not to detect a timstamp in
        // the files in a timestamped dirs
        let p1 = PathBuf::from("migrations/20210119200000_new_year_new_migration.es-postgres");
        match extract_timestamp(p1) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Error: {:?}", e)),
        }
    }

    #[test]
    fn test_step_from_migration_file() -> Result<(), String> {
        // requires a real file or directory, will try to
        // build the template after reading the file
        let path = PathBuf::from("test/fixtures/example-1-simple-mixed-migrations/migrations/20200904205000_get_es_health.es-docker.curl");
        let mut f = File::open(&path).map_err(|e| "Could not open path")?;
        let mut buffer = String::new();
        f.read_to_string(&mut buffer)
            .map_err(|e| "Could not read path")?;

        match part_from_migration_file(path.clone(), &buffer) {
            Err(e) => Err(format!("Error: {:?}", e)),
            Ok(part) => match part {
                None => Err("no matches".to_string()),
                Some(p) => match p.get(&Direction::Change) {
                    None => Err("steps doesn't have a Change direction step".to_string()),
                    Some(change) => {
                        assert_eq!(change.runner.name, "cURL");
                        assert_eq!(change.path, path);
                        // TODO: no test here for the Mustache contents, probably OK
                        Ok(())
                    }
                },
            },
        }
    }

    #[test]
    fn test_steps_in_migration_dir() -> Result<(), String> {
        let path = PathBuf::from("test/fixtures/example-1-simple-mixed-migrations/migrations/20210119200000_new_year_new_migration.es-postgres");
        match parts_in_migration_dir(path.clone()) {
            Err(e) => Err(format!("Error: {:?}", e)),
            Ok(part) => match part {
                None => Err("no matches".to_string()),
                Some(p) => {
                    match p.get(&Direction::Up) {
                        None => Err("steps doesn't have an Up direction step".to_string()),
                        Some(up) => {
                            assert_eq!(up.runner.name, "MariaDB");
                            assert_eq!(up.path, path.join("up.sql"));
                            // TODO: no test here for the Mustache contents, probably OK
                            Ok(())
                        }
                    }
                }
            },
        }
    }

    #[test]
    fn test_the_fixture_returns_correct_results() -> Result<(), String> {
        let path = Path::new("./test/fixtures/example-1-simple-mixed-migrations");
        match migrations(path.clone()) {
            Err(e) => Err(format!("Error: {:?}", e)),
            Ok(migrations) => {
                assert_eq!(migrations.len(), 4);
                Ok(())
            }
        }
    }

    #[test]
    fn test_build_in_migrations() -> Result<(), String> {
        let migrations = built_in_migrations();
        assert_eq!(migrations.len(), 1);
        Ok(())
    }
}
