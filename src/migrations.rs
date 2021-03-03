use crate::config::{Configuration, RunnerConfiguration};
/// Migration look-up functions. Explore the filesystem looking for
/// files matching the migration naming rules.
use crate::reserved::{runner_by_name, Runner};
use core::cmp::Ordering;
use rust_embed::RustEmbed;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

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
    /// IO Error such as insufficient permissions
    /// to open a file, or the file has been removed
    /// whilst we were working with it, etc, etc.
    Io(io::Error),
    /// We could not make a template out of the file
    /// somehow (syntax error is a good candidate)
    Mustache(mustache::Error),
    /// The migration parsed was not UTF-8, this can only happen
    /// for built-in migrations where we parse from an [u8]
    Utf8Error(std::str::Utf8Error),
}

impl From<io::Error> for MigrationsError {
    fn from(err: io::Error) -> MigrationsError {
        MigrationsError::Io(err)
    }
}
impl From<std::str::Utf8Error> for MigrationsError {
    fn from(err: std::str::Utf8Error) -> MigrationsError {
        MigrationsError::Utf8Error(err)
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

type MigrationSteps = HashMap<Direction, MigrationStep>;

type RunnerAndConfiguration = (Runner, RunnerConfiguration);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    pub date_time: chrono::NaiveDateTime,
    pub steps: MigrationSteps,
    pub built_in: bool,
    pub runner_and_config: RunnerAndConfiguration, // runners are compiled-in
}
impl<'a> PartialOrd for Migration {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.date_time.cmp(&other.date_time))
    }
}
impl<'a> Ord for Migration {
    fn cmp(&self, other: &Self) -> Ordering {
        self.date_time.cmp(&other.date_time)
    }
}

/// List all migrations known in the given context.
///
/// The file discovery will ignore a lot of files, hidden folders,
/// anything ignored by git's local or global ignores and some other
/// rules as described [here](https://docs.rs/ignore/0.4.17/ignore/struct.WalkBuilder.html#ignore-rules).
///
/// Order of the returned migrations is guaranteed by sorting the YYYYMMDDHHMMSS timestamp in ascending order (ordinal).AsRef
///
/// Runners must *run* these in chronological order to maintain the library
/// guarantees.
///
/// Ideally provide an absolute path. When giving a relative path in the config (e.g ".") the relative
/// path should be appended to the (ideally) absolute path.
pub fn migrations(c: &Configuration) -> Result<Vec<Migration>, MigrationsError> {
    let mf = MigrationFinder::new(c);
    let mut m = mf.built_in_migrations()?;
    m.extend(mf.migrations_in_dir(&c.migrations_directory)?);
    Ok(m)
}

struct MigrationFinder<'a> {
    config: &'a Configuration,
}

impl<'a> MigrationFinder<'a> {
    fn new(c: &'a Configuration) -> MigrationFinder {
        return MigrationFinder { config: c };
    }

    fn migrations_in_dir<P: AsRef<Path> + std::fmt::Debug>(
        &self,
        p: &P,
    ) -> Result<Vec<Migration>, MigrationsError> {
        let mut migrations: Vec<Migration> = vec![];
        for entry in ignore::Walk::new(p) {
            match entry {
                Ok(e) => match e.metadata() {
                    Ok(m) => match m.is_file() {
                        true => migrations.extend(self.migration_from_file(&e.path())?),
                        false => match m.is_dir() {
                            true => {
                                migrations.extend(self.migration_from_dir(&e.path())?);
                            }
                            false => {
                                debug!("{:?} is neither file nor directory (socket or symlink?)", p)
                            }
                        },
                    },
                    Err(e) => warn!("entry metadata err {}", e),
                },
                Err(e) => warn!("directory traversal err {}", e),
            }
        }

        // Migration implements Ord to consider only the date_time
        migrations.sort();

        Ok(migrations)
    }

    // Given a file this will return a single step. Standalone step
    // files are considered to be irreversible "change" migrations
    fn migation_step_from_file(
        &self,
        path: &'a Path,
        d: Direction,
    ) -> Result<MigrationSteps, MigrationsError> {
        let source = {
            let mut file = File::open(path)?;
            let mut buffer = String::new();
            file.read_to_string(&mut buffer)?;
            buffer
        };
        let content = mustache::compile_str(&source)?;

        let mut hm = HashMap::new();
        hm.insert(
            d,
            MigrationStep {
                content,
                source,
                path: PathBuf::from(path),
            },
        );
        Ok(hm)
    }

    // This is used when a directory is expected to contain "up" or "down" migrations
    // lots of overlap with migration_from_file.
    // Path will always be a dirname
    fn migration_from_dir(&self, dir: &Path) -> Result<Vec<Migration>, MigrationsError> {
        // trace!("checking if {:?} looks like a migration", dir);

        // Oh well, the safety dance
        // Ah yes, the safety dance
        // Oh well, the safety dance
        // Oh well, the safety dance
        // Oh yes, the safety dance
        // Oh, the safety dance, yeah
        // Well, it's the safety dance
        // It's the safety dance
        // Well, it's the safety dance
        // Well, it's the safety dance
        // Oh, it's the safety dance
        // Oh, it's the safety dance
        if !fs::metadata(dir)?.file_type().is_dir() {
            panic!("this method is only usable for directories")
        }

        // 20201208210038_hello_world.foo/{up,down}.bar
        // ^^^^^^^^^^^^^^ timestamp
        //                            ^^^ ext
        //
        // files in the directory will be checked right after
        let _date_time = extract_timestamp(dir).ok();
        let _config_name = dir.extension().map(|ext| ext.to_str());

        // We're not interested in recursing here, simple dir read is fine
        let dir_files: Vec<PathBuf> = fs::read_dir(dir)?
            .into_iter()
            .filter_map(|r| r.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .filter_map(|path| match (path.file_stem(), path.extension()) {
                (Some(stem), Some(_)) => match stem.to_str()? {
                    "up" => Some(path),
                    "down" => Some(path),
                    _ => None,
                },
                _ => None,
            })
            .collect();

        if dir_files.len() > 0 {
            error!("dir files {:?}", dir_files);
        }

        Ok(vec![])
    }

    fn built_in_migrations(&self) -> Result<Vec<Migration>, MigrationsError> {
        Ok(BuiltInMigrations::iter()
            .filter_map(|file: Cow<'static, str>| {
                // Reminder the .ok()? here is because we are in filter_map
                // it has to return Option<T> to _filter_.

                // Get a pathbuf and the bytes of the file and
                // verifies the UTf-8 encoding of the source
                // the question marks here jump us out of the
                // filter map if we have problems.
                let path = PathBuf::from(file.as_ref());
                let bytes = BuiltInMigrations::get(file.as_ref())?;
                let source = String::from(std::str::from_utf8(&bytes).ok()?);

                // compile the template and search for a date_time
                // these are built-in migrations, no excuse for not
                // having both of these working so simply.
                //
                // The question mark breaks us out of this loop
                let content = mustache::compile_str(&source).ok()?;
                let date_time = extract_timestamp(&path).ok()?;

                // Dissect the filename... (this block is duplicated in migration_from_file)
                // but I prefer duplication over abstraction here.
                //
                // 20201208210038_hello_world.foo.bar
                // ^^^^^^^^^^^^^^ timestamp
                //                ^^^^^^^^^^^^^^^ stem
                //                                ^^^ ext
                //                            ^^^ config name
                let stem = path.file_stem().map(|s| PathBuf::from(s));
                let ext = path.extension().map(|ext| ext.to_str()).flatten();
                let config_name = match &stem {
                    Some(stem) => stem.extension().map(|ext| ext.to_str()).flatten(),
                    None => None {},
                };

                match (config_name, ext) {
                    (Some(cn), Some(e)) => {
                        info!(
                            "built-in migration candidate {:?} {:?}, {:?}",
                            date_time, cn, e
                        );
                        match self.is_configured_runner(cn, e) {
                            Ok(runner_and_config) => {
                                let mut steps: MigrationSteps = HashMap::new();
                                steps.insert(
                                    Direction::Change,
                                    MigrationStep {
                                        content,
                                        source,
                                        path: path,
                                    },
                                );
                                Some(Migration {
                                    built_in: true,
                                    date_time,
                                    runner_and_config,
                                    steps,
                                })
                            }
                            Err(e) => {
                                error!("configuration for built-in runner not provided ({})", e);
                                None {}
                            }
                        }
                    }
                    _ => {
                      warn!("no config name found, a migration has something in the filename which isn't in the configuration");
                      None{}
                    }
                }
            })
            .collect())
    }

    // Vec may be empty if we didn't find anything. We may have good filename candidates
    // just because of random formatting, but we only return an errornous result incase
    // we were three-for-three finding filename traits, and we didn't find a corresponding
    // configuration
    fn migration_from_file(&self, p: &'a Path) -> Result<Vec<Migration>, MigrationsError> {
        // trace!("checking if {:?} looks like a migration", p);
        // 20201208210038_hello_world.foo.bar
        // ^^^^^^^^^^^^^^ timestamp
        //                ^^^^^^^^^^^^^^^ stem
        //                                ^^^ ext
        //                            ^^^ config name
        let date_time = extract_timestamp(p).ok();
        let stem = p.file_stem().map(|s| PathBuf::from(s));
        let ext = p.extension().map(|ext| ext.to_str()).flatten();
        let config_name = match &stem {
            Some(stem) => stem.extension().map(|ext| ext.to_str()).flatten(),
            None => None {},
        };

        match (date_time, config_name, ext) {
            (Some(date_time), Some(cn), Some(e)) => {
                debug!(
                    "found migration candidate {:?} {:?}, {:?}",
                    date_time, cn, e
                );
                match self.is_configured_runner(cn, e) {
                    Ok(runner_and_config) => {
                        match self.migation_step_from_file(p, Direction::Change) {
                            Ok(steps) => Ok(vec![Migration {
                                built_in: false,
                                date_time,
                                runner_and_config,
                                steps,
                            }]),
                            Err(e) => Err(e),
                        }
                    }
                    Err(e) => {
                        warn!("{:?} was formatted like a Mitre migration, but no suitable config could be found: {}", p, e);
                        Ok(vec![])
                    } // but do nothing, this is a search, after-all
                }
            }
            _ => {
                // debug!("no good candidate {:?}", p);
                Ok(vec![])
            }
        }
    }

    // This method looks for a configuration name (key) in the configured runners
    // where that configuration name has a _runner which supports a given file
    // extension, according to the
    fn is_configured_runner(
        &self,
        config_name: &str,
        ext: &str,
    ) -> Result<RunnerAndConfiguration, String> {
        trace!(
            "{} config name is, configured runners is {:#?}",
            config_name,
            self.config.configured_runners
        );
        match self.config.configured_runners.get(config_name) {
            Some(config) => match runner_by_name(&config._runner) {
                Some(runner) => match runner.exts.iter().find(|e| e == &&ext) {
                    Some(_) => Ok((runner, config.clone())),
                    None => Err(format!(
                        "runner {} does not support ext {}",
                        runner.name, ext
                    )),
                },
                None => Err(format!(
                    "no such runner {} in this version of Mitre",
                    &config._runner
                )),
            },
            None => Err(format!("no configuration found for runner {}", config_name)),
        }
    }
}

fn extract_timestamp(p: &Path) -> Result<chrono::NaiveDateTime, &'static str> {
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
        extract_timestamp(&p)?;

        let p = PathBuf::from(
            "test/fixtures/example-1-simple-mixed-migrations/migrations/example.curl",
        );
        match extract_timestamp(&p) {
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
        match extract_timestamp(&p1) {
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
        match extract_timestamp(&p1) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Error: {:?}", e)),
        }
    }

    // #[test]
    // fn test_step_from_migration_file() -> Result<(), String> {
    //     // requires a real file or directory, will try to
    //     // build the template after reading the file
    //     let path = PathBuf::from("test/fixtures/example-1-simple-mixed-migrations/migrations/20200904205000_get_es_health.es-docker.curl");
    //     let mut f = File::open(&path).map_err(|e| format!("Could not open path {:?}", e))?;
    //     let mut buffer = String::new();
    //     f.read_to_string(&mut buffer)
    //         .map_err(|e| format!("Could not read path {:?}", e))?;

    //     match part_from_migration_file(path.clone(), &buffer) {
    //         Err(e) => Err(format!("Error: {:?}", e)),
    //         Ok(part) => match part {
    //             None => Err("no matches".to_string()),
    //             Some(p) => match p.get(&Direction::Change) {
    //                 None => Err("steps doesn't have a Change direction step".to_string()),
    //                 Some(change) => {
    //                     assert_eq!(change.runner.name, "cURL");
    //                     assert_eq!(change.path, path);
    //                     // TODO: no test here for the Mustache contents, probably OK
    //                     Ok(())
    //                 }
    //             },
    //         },
    //     }
    // }

    // #[test]
    // fn test_steps_in_migration_dir() -> Result<(), String> {
    //     let path = PathBuf::from("test/fixtures/example-1-simple-mixed-migrations/migrations/20210119200000_new_year_new_migration.es-postgres");
    //     match parts_in_migration_dir(path.clone()) {
    //         Err(e) => Err(format!("Error: {:?}", e)),
    //         Ok(part) => match part {
    //             None => Err("no matches".to_string()),
    //             Some(p) => {
    //                 match p.get(&Direction::Up) {
    //                     None => Err("steps doesn't have an Up direction step".to_string()),
    //                     Some(up) => {
    //                         assert_eq!(up.runner.name, "MariaDB");
    //                         assert_eq!(up.path, path.join("up.sql"));
    //                         // TODO: no test here for the Mustache contents, probably OK
    //                         Ok(())
    //                     }
    //                 }
    //             }
    //         },
    //     }
    // }

    // #[test]
    // fn test_the_new_thing_finds_all_the_fixtures_correctly() -> Result<(), String> {
    //     let path = PathBuf::from("./test/fixtures/example-1-simple-mixed-migrations");
    //     let config = Configuration::new(Some(path));
    //     MigrationFinder::new(&config).migrations_in_migrations_dir();
    //     Ok(())
    // }

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

    // #[test]
    // fn test_build_in_migrations() -> Result<(), String> {
    //     let migrations = built_in_migrations();
    //     assert_eq!(migrations.len(), 1);
    //     Ok(())
    // }
}
