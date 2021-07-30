use log::{debug, error, info, trace, warn};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::string::String;

use crate::config::Configuration;
use crate::migrations::built_in_migrations::BuiltInMigrations;
use crate::migrations::{Direction, Migration, MigrationStep};
use crate::migrations::{MigrationSteps, RunnerAndConfiguration, FORMAT_STR};
use crate::reserved::{flags, runner_by_name, Flag};

use super::{Error, MigrationList};

/// List all migrations known in the given migrations_directory on the Configuration
///
/// The file discovery will ignore a lot of files, hidden folders,
/// anything ignored by git's local or global ignores and some other
/// rules as described [here](https://docs.rs/ignore/0.4.17/ignore/struct.WalkBuilder.html#ignore-rules).
///
/// Order of the returned migrations is guaranteed by sorting the YYYYMMDDHHMMSS timestamp in ascending order (ordinal).
///
/// Runners must *run* these in chronological order to maintain the library
/// guarantees. Concurrency probably isn't super wise, either. Although we can always
/// revisit that later with a possible flag on the filenames that a migration is concurrency safe
/// if we ever need to.
///
/// Ideally provide an absolute path. When giving a relative path in the config (e.g ".") the relative
/// path should be appended to the (ideally) absolute path.
pub fn from_disk(config: &Configuration) -> MigrationFinder {
    MigrationFinder {
        config: config.clone(),
    }
}

pub struct MigrationFinder {
    config: Configuration,
}

impl<'a> MigrationList for MigrationFinder {
    fn all<'b>(&'b mut self) -> Result<Box<(dyn Iterator<Item = Migration> + 'b)>, Error> {
        let mut m = self.built_in_migrations();
        m.extend(self.migrations_in_dir()?);
        Ok(Box::new(m.into_iter()))
    }
}

impl<'a> MigrationFinder {
    fn migrations_in_dir(&self) -> Result<Vec<Migration>, Error> {
        let mut migrations: Vec<Migration> = vec![];
        for entry in ignore::Walk::new(&self.config.migrations_directory) {
            match entry {
                Ok(e) => match e.metadata() {
                    Ok(m) => match m.is_file() {
                        true => migrations.extend(self.migration_from_file(&e.path())?),
                        false => match m.is_dir() {
                            true => {
                                migrations.extend(self.migration_from_dir(&e.path())?);
                            }
                            false => {
                                debug!(
                                    "{:?} is neither file nor directory (socket or symlink?)",
                                    self.config.migrations_directory
                                )
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
    fn migation_step_from_file(&self, path: &Path, d: Direction) -> Result<MigrationSteps, Error> {
        let source = {
            let mut file = File::open(path)?;
            let mut buffer = String::new();
            file.read_to_string(&mut buffer)?;
            buffer
        };
        let mut hm = HashMap::new();
        hm.insert(
            d,
            MigrationStep {
                source,
                path: PathBuf::from(path),
            },
        );
        Ok(hm)
    }

    //
    fn flags_from_filename(&self, p: Option<&str>) -> Vec<Flag> {
        match p {
            Some(str) => str
                .split(|x| x == std::path::MAIN_SEPARATOR || x == '.')
                .filter_map(|p| flags().find(|r| r.name == p))
                .collect(),
            None => vec![],
        }
    }

    // This is used when a directory is expected to contain "up" or "down" migrations
    // lots of overlap with migration_from_file.
    // Path will always be a dirname
    fn migration_from_dir(&self, dir: &Path) -> Result<Vec<Migration>, Error> {
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
        let date_time = match extract_timestamp(dir) {
            Ok(dt) => dt,
            Err(_) => {
                return Ok(vec![]);
            }
        };
        let config_name = dir.extension().map(|ext| ext.to_str()).flatten();

        // We're not interested in recursing here, simple dir read is fine
        let mut steps: MigrationSteps = HashMap::new();
        let mut runner_and_config: Option<RunnerAndConfiguration> = None;
        let _ = fs::read_dir(dir)?
            .into_iter()
            .filter_map(|r| r.ok())
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .filter_map(|path| {
                match (
                    path.file_stem(),
                    path.extension().map(|e| e.to_str()).flatten(),
                    config_name,
                ) {
                    (Some(stem), Some(ext), Some(cn)) => match stem.to_str()? {
                        "up" => match self.is_configured_runner(cn, ext) {
                            Ok(rac) => {
                                runner_and_config = Some(rac);
                                Some((path.clone(), Direction::Up))
                            }
                            Err(e) => {
                                warn!("No runner configured for {:?}: {}", path, e);
                                None {}
                            }
                        },
                        "down" => Some((path.clone(), Direction::Down)),
                        _ => None,
                    },
                    _ => None,
                }
            })
            .filter_map(|(path, direction)| {
                let source = {
                    let mut file = File::open(path.clone()).ok()?;
                    let mut buffer = String::new();
                    file.read_to_string(&mut buffer).ok()?;
                    buffer
                };

                steps.insert(direction, MigrationStep { path, source });
                Some(()) // redundant, just for filter_map and ? above
            })
            .collect::<()>(); // force us to do the work, else is lazy

        // TODO: check up and down have the *same* extension

        match runner_and_config {
            Some(rac) => Ok(vec![Migration {
                built_in: false,
                date_time,
                configuration_name: rac.configuration_name,
                flags: self.flags_from_filename(dir.to_str()),
                steps,
            }]),
            None => Ok(vec![]),
        }
    }

    fn built_in_migrations(&self) -> Vec<Migration> {
        BuiltInMigrations::iter()
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
                let date_time = extract_timestamp(&path).ok()?;

                // Dissect the filename... (this block is duplicated in migration_from_file)
                // but I prefer duplication over abstraction here.
                //
                // 20201208210038_hello_world.foo.bar
                // ^^^^^^^^^^^^^^ timestamp
                //                ^^^^^^^^^^^^^^^ stem
                //                                ^^^ ext
                //                            ^^^ config name
                let stem = path.file_stem().map(PathBuf::from);
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
                                        source,
                                        path: path.clone(),
                                    },
                                );
                                Some(Migration {
                                    date_time,
                                    steps,
                                    built_in: true,
                                    flags: self.flags_from_filename(path.to_str()),
                                    configuration_name: runner_and_config.configuration_name,
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
            .collect()
    }

    // Vec may be empty if we didn't find anything. We may have good filename candidates
    // just because of random formatting, but we only return an errornous result incase
    // we were three-for-three finding filename traits, and we didn't find a corresponding
    // configuration
    fn migration_from_file(&self, p: &'a Path) -> Result<Vec<Migration>, Error> {
        // trace!("checking if {:?} looks like a migration", p);
        // 20201208210038_hello_world.foo.bar
        // ^^^^^^^^^^^^^^ timestamp
        //                ^^^^^^^^^^^^^^^ stem
        //                                ^^^ ext
        //                            ^^^ config name
        let date_time = extract_timestamp(p).ok();
        let stem = p.file_stem().map(PathBuf::from);
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
                                configuration_name: runner_and_config.configuration_name,
                                flags: self.flags_from_filename(p.to_str()),
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
            "checking for runner {:?} {:?} in {:?}",
            config_name,
            ext,
            self.config.configured_runners
        );
        match self.config.get(config_name) {
            Some(config) => match runner_by_name(&config._runner) {
                Some(runner) => match runner.exts.iter().find(|e| e == &&ext) {
                    Some(_) => Ok(RunnerAndConfiguration {
                        runner,
                        runner_configuration: config.clone(),
                        configuration_name: String::from(config_name),
                    }),
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

    #[test]
    fn test_fixture_1_returns_correct_results() -> Result<(), String> {
        let path = PathBuf::from("./test/fixtures/example-1-simple-mixed-migrations/mitre.yml");
        let config = match Configuration::from_file(&path) {
            Ok(config) => config,
            Err(e) => Err(format!("couldn't make config {}", e))?,
        };

        match from_disk(&config.clone()).all() {
            Err(e) => Err(format!("Error: {:?}", e)),
            Ok(migrations) => {
                assert_eq!(migrations.collect::<Vec<Migration>>().len(), 3); // built-in migrations are being deprecated
                Ok(())
            }
        }
    }
}
