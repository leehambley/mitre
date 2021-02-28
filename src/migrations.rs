extern crate mustache;

use super::reserved::{runner_by_name, runners, Runner};
use crate::config::{Configuration, RunnerConfiguration};
use rust_embed::RustEmbed;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use walkdir::WalkDir;

pub const FORMAT_STR: &str = "%Y%m%d%H%M%S";

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

#[derive(Debug)]
pub enum MigrationsError {
    Io(io::Error),
    Mustache(mustache::Error),
}

impl From<io::Error> for MigrationsError {
    fn from(err: io::Error) -> MigrationsError {
        MigrationsError::Io(err)
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
    pub runner_and_config: Option<(Runner, RunnerConfiguration)>, // runners are compiled-in
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
pub fn migrations(c: &Configuration) -> Result<Vec<Migration>, MigrationsError> {
    let mut m = built_in_migrations();
    m.extend(MigrationFinder::new(c).migrations_in_migrations_dir());
    Ok(m)
}

struct MigrationFinder<'a> {
    config: &'a Configuration,
    found: Vec<Migration>,
}

impl<'a> MigrationFinder<'a> {
    fn new(c: &'a Configuration) -> MigrationFinder {
        return MigrationFinder {
            config: c,
            found: vec![],
        };
    }
    // List files
    fn migrations_in_migrations_dir(&mut self) -> Vec<Migration> {
        self.find_migrations_in_dir(&self.config.migrations_directory);
        self.found.clone()
    }
    fn find_migrations_in_dir<P: AsRef<Path>>(&mut self, p: &P) {
        for entry in fs::read_dir(p).expect("TODO: err handling") {
            info!("exploring {:?}", entry);
            match entry {
                Ok(e) => match e.metadata() {
                    Ok(m) => match m.is_file() {
                        true => self.find_migration_from_file(&e.path()),
                        false => self.find_migrations_in_dir(&e.path()),
                    },
                    Err(e) => warn!("entry metadata err {}", e),
                },
                Err(e) => warn!("dir traversal err {}", e),
            }
        }
    }
    fn find_migration_from_file(&mut self, p: &Path) {
        trace!("checking if {:?} looks like a migration", p);
        let ts = match extract_timestamp(p.to_path_buf()) {
            Ok(ts) => Some(ts),
            Err(_e) => None {},
        };
        // 20201208210038_hello_world.foo.bar
        // ^^^^^^^^^^^^^^ timestamp
        //                ^^^^^^^^^^^^^^^ stem
        //                                ^^^ ext
        //                            ^^^ config name
        let stem = p.file_stem().map(|s| PathBuf::from(s));
        let config_name = match &stem {
            Some(stem) => stem.extension().map(|ext| ext.to_str()).flatten(),
            None => None {},
        };
        let ext = p.extension().map(|ext| ext.to_str()).flatten();

        match (ts, config_name, ext) {
            (Some(ts), Some(cn), Some(e)) => {
                info!("found good candidate {:?} {:?}, {:?}", ts, cn, e);
                let _r = self.is_configured_runner(cn, e);
            }
            _ => debug!("no good candidate {:?}", p),
        }
    }
    /// Returns
    fn is_configured_runner(
        &self,
        config_name: &str,
        ext: &str,
    ) -> Result<(Runner, RunnerConfiguration), ()> {
        match self.config.configured_runners.get(config_name) {
            Some(config) => match runner_by_name(config_name) {
                Some(runner) => match runner.exts.iter().find(|e| e == &&ext) {
                    Some(_) => Ok((runner, config.clone())),
                    None => {
                        warn!("runner {} does not support ext {}", runner.name, ext);
                        Err(())
                    }
                },
                None => {
                    warn!("no such runner {} in this version of Mitre", config_name);
                    Err(())
                }
            },
            None => {
                warn!("no configuration found for runner {}", config_name);
                Err(())
            }
        }
    }
}

// https://rust-lang-nursery.github.io/rust-cookbook/file/dir.html
// This should take an *absolute* path
fn migrations_in(c: &Configuration) -> Vec<Migration> {
    trace!(
        "beginning to search for migrations in {:?}",
        c.migrations_directory
    );
    WalkDir::new(&c.migrations_directory)
        .into_iter()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            trace!("migation search in {:?}", &entry);
            match extract_timestamp(entry.path().to_path_buf()) {
                Ok(timestamp) => {
                    let path_buf = entry.path().to_path_buf();
                    if entry.file_type().is_dir() {
                        match parts_in_migration_dir(path_buf).ok()? {
                            Some(parts) => Some(Migration {
                                date_time: timestamp,
                                steps: parts,
                                built_in: false,
                                runner_and_config: None {},
                            }),
                            _ => None {},
                        }
                    } else {
                        let mut f = File::open(&path_buf).ok()?;
                        let mut buffer = String::new();
                        f.read_to_string(&mut buffer).ok()?;
                        match part_from_migration_file(path_buf, &buffer).ok()? {
                            Some(parts) => Some(Migration {
                                date_time: timestamp,
                                steps: parts,
                                built_in: false,
                                runner_and_config: None {},
                            }),
                            _ => None {},
                        }
                    }
                }
                Err(_) => None {}, // err contains a string reason why from timestamp parser, ignore it
            }
        })
        .collect()
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
                        runner_and_config: None {},
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

    // hello_world.foo.bar
    // ^^^^^^^^^^^^^^^ stem
    //                 ^^^ ext
    //             ^^^ config name
    let stem = p.file_stem().map(|s| PathBuf::from(s));
    let config_name = match &stem {
        Some(stem) => stem.extension().map(|ext| ext.to_str()),
        None => None {},
    };
    let ext = p.extension();

    // match (config_name, ext) {
    //   // Structurally we found two dot parts in the filename, see example above,
    //   // now we need to check the config has that key, and
    //   (Some(config_name), Some(ext)) => {
    //     self.runn
    //   }
    //   _ => {
    //     warn!("Did not find both runner and config name file parts in {:?}", p);
    //   }
    // }

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
        // TODO: warn on `change` direction in a directory
        let stem_str = p.file_stem().unwrap().to_str().unwrap();
        let direction = match stem_str {
            "up" => Some(Direction::Up),
            "down" => Some(Direction::Down),
            _ => None {}, // something else, never-mind
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
        let mut f = File::open(&path).map_err(|e| format!("Could not open path {:?}", e))?;
        let mut buffer = String::new();
        f.read_to_string(&mut buffer)
            .map_err(|e| format!("Could not read path {:?}", e))?;

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
    fn test_the_new_thing_finds_all_the_fixtures_correctly() -> Result<(), String> {
        let path = PathBuf::from("./test/fixtures/example-1-simple-mixed-migrations");
        let config = Configuration::new(Some(path));
        MigrationFinder::new(&config).migrations_in_migrations_dir();
        Ok(())
    }

    #[test]
    fn test_the_fixture_returns_correct_results() -> Result<(), String> {
        let path = PathBuf::from("./test/fixtures/example-1-simple-mixed-migrations");
        let config = Configuration::new(Some(path));

        match migrations(&config) {
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
