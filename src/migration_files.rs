extern crate mustache;
use crate::migrations::Direction;
use crate::reserved::{words, Word};
use regex;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub const FORMAT_STR: &str = "%Y%m%d%H%M%S";

type MigrationPart = (PathBuf, Direction, Word);

#[derive(Debug)]
pub struct MigrationStep {
    content: mustache::Template,
    path: PathBuf,
    runner: crate::reserved::Word,
}

#[derive(Debug)]
pub struct MigrationCandidate {
    date_time: chrono::NaiveDateTime,
    steps: HashMap<Direction, MigrationStep>,
}

// https://rust-lang-nursery.github.io/rust-cookbook/file/dir.html
pub fn migrations_in(p: &Path) -> Result<Vec<MigrationCandidate>, std::io::Error> {
    let mut found = Vec::new();
    let mut migrations = Vec::new();

    for entry in WalkDir::new(p).into_iter().filter_map(Result::ok) {
        match extract_timestamp(entry.path().to_path_buf()) {
            Ok(timestamp) => {
                let path_buf = entry.path().to_path_buf();
                if entry.file_type().is_dir() {
                    let parts = parts_in_migration_dir(path_buf.clone())?;
                    found.push((timestamp, path_buf, parts))
                } else {
                    match part_from_migration_file(path_buf.clone()) {
                        Some(parts) => found.push((timestamp, path_buf, parts)),
                        _ => {}
                    }
                }
            }
            Err(_) => continue, // err contains a string reason why from timestamp parser, ignore it
        }
    }

    for (timestamp, _, parts) in found.iter() {
        let ms: HashMap<Direction, MigrationStep> = parts
            .iter()
            .map(|part| {
                let direction = part.1.clone();
                let path = part.0.clone();
                let runner = part.2.clone();
                let template = mustache::compile_path(path.clone());
                (
                    direction,
                    MigrationStep {
                        path,
                        content: template.unwrap(),
                        runner,
                    },
                )
            })
            .into_iter()
            .collect();

        migrations.push(MigrationCandidate {
            date_time: *timestamp,
            steps: ms,
        });
    }
    Ok(migrations)
}

fn part_from_migration_file(p: PathBuf) -> Option<Vec<MigrationPart>> {
    let parts: Vec<&str> = p
        .to_str()
        .expect("fofofofof")
        .split(|x| x == std::path::MAIN_SEPARATOR || x == '_' || x == '.')
        .collect();

    match crate::reserved::words().into_iter()
        .filter(|word| word.kind == crate::reserved::Kind::Runner)
        .find(|word| parts.contains(&word.word) ) {
          Some(runner) => Some(vec![(p, Direction::Change, runner)]),
          None => None {}
        }
}

// Returns tuples [(path, direction, runner)]
// matching against OsStr is a bit awkward, but no big deal
fn parts_in_migration_dir(p: PathBuf) -> Result<Vec<MigrationPart>, io::Error> {
    fn has_proper_name(p: PathBuf) -> Option<MigrationPart> {
        let (up, down) = (OsStr::new("up"), OsStr::new("down"));
        let direction = match p.file_stem().unwrap_or(OsStr::new("")) {
            up => Some(Direction::Up),
            down => Some(Direction::Down),
            // change => Some(Direction::Change), # "change" in a dir doesn't really make sense, but maybe
            _ => None {},
        };
        let reserved_words = crate::reserved::words();
        let mut runner_reserved_words = reserved_words
            .iter()
            .filter(|word| word.kind == crate::reserved::Kind::Runner);
        let runner = runner_reserved_words.find(|word| match p.extension() {
            Some(ext) => ext == word.word,
            None => false,
        });
        match (direction, runner) {
            (Some(d), Some(r)) => Some((p, d, r.clone())),
            _ => None {},
        }
    }
    let entries = fs::read_dir(".")?
        .filter_map(|res| res.map(|e| e.path()).ok())
        .filter_map(|p| has_proper_name(p))
        .collect();
    Ok(entries)
}

fn extract_timestamp(p: PathBuf) -> Result<chrono::NaiveDateTime, &'static str> {
    // Search for "SEPARATOR\d{14}_" (dir separator, 14 digits, underscore)
    // Note: cannot use FORMAT_STR.len() here because %Y is 2 chars, but wants 4
    let re = regex::Regex::new(
        format!(
            r#"{}(\d{{14}})_"#,
            regex::escape(format!("{}", std::path::MAIN_SEPARATOR).as_str())
        )
        .as_str(),
    ).unwrap();
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
    fn test_part_from_migration_file() -> Result<(), &'static str> {
      match part_from_migration_file(PathBuf::from("/foo/bar/baz.mysql")) {
        Some(p) => {
          assert_eq!(p.len(), 1);
          assert_eq!(p.first().unwrap().2.word, "mysql");
          Ok(())
        },
        None => Err("didn't match mysql runner properly")
      }    
    }

    #[test]
    fn test_the_fixture_returns_correct_results() -> Result<(), &'static str> {
        let result = migrations_in(Path::new("test/fixtures")).expect("migrations_in failed");
        assert_eq!(result.len(), 3);
        println!("{:?}", result);
        Ok(())
    }
}
