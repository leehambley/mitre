extern crate mustache;
use crate::migrations::Direction;
use crate::reserved::{words, Word};
use regex;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub const FORMAT_STR: &str = "%Y%m%d%H%M%S";

#[derive(Debug)]
pub enum MigrationsError {
    IO(std::io::Error),
    Mustache(mustache::Error), // None(std::option::NoneError) // comes from the mustache parsing string function
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

// impl From<std::option::NoneError> for MigrationsError {
//   fn from(err: std::option::NoneError) -> MigrationsError {
//       MigrationsError::None(err)
//   }
// }

#[derive(Debug)]
pub struct MigrationStep {
    path: PathBuf,
    content: mustache::Template,
    runner: crate::reserved::Runner,
}

#[derive(Debug)]
pub struct MigrationCandidate {
    date_time: chrono::NaiveDateTime,
    steps: HashMap<Direction, MigrationStep>,
}

// https://rust-lang-nursery.github.io/rust-cookbook/file/dir.html
// This should take an *absolute* path
pub fn migrations_in(
    p: &Path,
) -> Result<impl Iterator<Item = MigrationCandidate>, MigrationsError> {
    Ok(WalkDir::new(p)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            match extract_timestamp(entry.path().to_path_buf()) {
                Ok(timestamp) => {
                    let path_buf = entry.path().to_path_buf();
                    if entry.file_type().is_dir() {
                        match parts_in_migration_dir(path_buf.clone()).ok()? {
                            Some(parts) => Some(MigrationCandidate {
                                date_time: timestamp,
                                steps: parts,
                            }),
                            _ => None {},
                        }
                    } else {
                        match part_from_migration_file(path_buf.clone()).ok()? {
                            Some(parts) => Some(MigrationCandidate {
                                date_time: timestamp,
                                steps: parts,
                            }),
                            _ => None {},
                        }
                    }
                }
                Err(_) => None {}, // err contains a string reason why from timestamp parser, ignore it
            }
        }))
}

fn mustache_template_from(p: PathBuf) -> Result<mustache::Template, MigrationsError> {
    let mut f = File::open(p)?;
    let mut buffer = String::new();
    f.read_to_string(&mut buffer)?;
    Ok(mustache::compile_str(&buffer)?)
}

fn part_from_migration_file(
    p: PathBuf,
) -> Result<Option<HashMap<Direction, MigrationStep>>, MigrationsError> {
    let parts = p
        .to_str()
        .unwrap()
        .split(|x| x == std::path::MAIN_SEPARATOR || x == '_' || x == '.');

    // TODO: warn if more than one runner found?
    let runner: Option<crate::reserved::Runner> = parts
        .filter_map(|p| runner_reserved_word_from_str(&p))
        .take(1)
        .next();

    let t = mustache_template_from(p.clone())?;

    match runner {
        Some(r) => Ok(Some(
            vec![(
                Direction::Change,
                MigrationStep {
                    content: t,
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

fn runner_reserved_word_from_str(s: &&str) -> Option<crate::reserved::Runner> {
    let reserved_words = crate::reserved::reserved_words();
    let mut runner_reserved_words = reserved_words
        .iter()
        .filter(|word| match word {
            crate::reserved::ReservedWord::Runner(_) => true,
            _ => false,
        })
        .filter_map(|word| match word {
            crate::reserved::ReservedWord::Runner(r) => Some(r.clone()),
            _ => None {},
        });

    runner_reserved_words.find(|word| word.exts.contains(s))
}

// Returns tuples [(path, direction, runner)]
// matching against OsStr is a bit awkward, but no big deal
fn parts_in_migration_dir(
    p: PathBuf,
) -> Result<Option<HashMap<Direction, MigrationStep>>, MigrationsError> {
    fn has_proper_name(p: PathBuf) -> Option<(Direction, MigrationStep)> {
        let (up, down) = (OsStr::new("up"), OsStr::new("down"));
        // TODO: warn on `change` direction in a directory
        let direction = match p.file_stem().unwrap_or(OsStr::new("")) {
            up => Some(Direction::Up),
            down => Some(Direction::Down),
            _ => None {},
        };
        let runner = match p.extension() {
            Some(e) => runner_reserved_word_from_str(&e.to_str().unwrap()),
            None => None {},
        };
        let template = mustache_template_from(p.clone()).ok()?;
        match (direction, runner) {
            (Some(d), Some(r)) => Some((
                d,
                MigrationStep {
                    content: template,
                    path: p,
                    runner: r,
                },
            )),
            _ => None {},
        }
    }
    let entries = fs::read_dir(p)?
        .filter_map(|res| res.map(|e| e.path()).ok())
        .filter_map(|p| {
            println!("looking into {:?}", p);
            has_proper_name(p)
        })
        .collect();
    println!("entries is {:#?}", entries);
    Ok(Some(entries))
}

fn extract_timestamp(p: PathBuf) -> Result<chrono::NaiveDateTime, &'static str> {
    // Search for "SEPARATOR\d{14}_[a-Z0-9\.]" (dir separator, 14 digits, underscore)
    // Note: cannot use FORMAT_STR.len() here because %Y is 2 chars, but wants 4
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
                Ok(ndt) => {
                    println!("..found {:?}", ndt);
                    Ok(ndt)
                }
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
        // the files in a timestamped dir
        let p1 = PathBuf::from("migrations/20210119200000_new_year_new_migration.es-postgres");
        match extract_timestamp(p1) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Error: {:?}", e)),
        }
    }

    #[test]
    fn test_part_from_migration_file() -> Result<(), String> {
        // requires a real file or directory, will try to
        // build the template after reading the file
        let path = PathBuf::from("test/fixtures/example-1-simple-mixed-migrations/migrations/20200904205000_get_es_health.es-docker.curl");
        match part_from_migration_file(path.clone()) {
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
    fn test_parts_in_migration_dir() -> Result<(), String> {
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
        match migrations_in(path.clone()) {
            Err(e) => Err(format!("Error: {:?}", e)),
            Ok(migrations) => {
                let m: Vec<MigrationCandidate> = migrations.collect();
                assert_eq!(m.len(), 3);
                Ok(())
            }
        }
    }
}
