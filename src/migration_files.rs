extern crate mustache;
use crate::migrations::Direction;
use crate::reserved::{Word, words};
use regex;
use std::collections::HashMap;
use std::fs;
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub const FORMAT_STR: &str = "%Y%m%d%H%M%S";

#[derive(Debug)]
pub struct MigrationStep {
  path: PathBuf,
  content: mustache::Template,
}

#[derive(Debug)]
pub struct MigrationCandidate {
  date_time: chrono::NaiveDateTime,
  steps: HashMap<Direction, MigrationStep>
}

// https://rust-lang-nursery.github.io/rust-cookbook/file/dir.html
pub fn migrations_in(p: &Path) -> Result<Vec<MigrationCandidate>, std::io::Error> {

    let mut candiates: Vec<MigrationCandidate> = Vec::new();

    let mut dirs: Vec<(PathBuf,chrono::NaiveDateTime)> = Vec::new();
    let mut files: Vec<(PathBuf,chrono::NaiveDateTime)> = Vec::new();

    for entry in WalkDir::new(p)
            .into_iter()
            .filter_map(Result::ok) {
                match extract_timestamp(entry.path().to_path_buf()) {
                    Ok(timestamp) => {
                        println!("{:?}", entry.path());
                        if entry.file_type().is_dir() {
                            dirs.push((entry.path().to_path_buf(), timestamp))
                        } else {
                            files.push((entry.path().to_path_buf(), timestamp))
                        }
                    }
                    Err(_) => continue, // err contains a string reason why from timestamp parser, ignore it
            }
    }

    println!("files is {:?} (len {})", files, files.len());

    for (path, timestamp) in files.iter() {
      let mut paths = HashMap::<Direction, PathBuf>::new();
      paths.insert(Direction::Up, path.to_path_buf());
      let steps = HashMap::new();
      candiates.push(
        MigrationCandidate{date_time: *timestamp, steps }
      );
    }

    // For files we check if they also contain a valid runner
    // and config "dot parts" in the filename
    // e.g 20201208210038_get_es_health.es-postgres.data.long.risky.curl

    // For directories we check if the directory contains files
    // which have valid runner "dot parts" in _their_ filenames
    // e.g 
    //      ./some/dir/20201208210038_get_es_health
    //                 \- up.sql
    //                 \- down.sql

    // TODO: Finish me
println!("{:?}", candiates);

    return Ok(candiates);
}

 // https://codereview.stackexchange.com/a/98547
  fn basename<'a>(path: &'a str) -> std::borrow::Cow<'a, str> {
    let mut pieces = path.rsplit(std::path::MAIN_SEPARATOR);
    match pieces.next() {
        Some(p) => p.into(),
        None => path.into(),
    }
  }

// Returns tuples [(path, direction, runner)] 
// matching against OsStr is a bit awkward, but no big deal
type MigrationPartInDir = (PathBuf, Direction, Word);
fn parts_in_migration_dir(p: PathBuf) -> Result<(), io::Error> {
  fn has_proper_name(p: PathBuf) -> Option<MigrationPartInDir> {
    let (up, down, change) = (OsStr::new("up"), OsStr::new("down"), OsStr::new("change"));
    let direction = match p.file_stem().unwrap_or(OsStr::new("")) {
      up => Some(Direction::Up),
      down => Some(Direction::Down),
      change => Some(Direction::Change),
      _ => None{},
    };
    let mut runner_reserved_words = crate::reserved::words()
        .iter()
        .filter(|word| word.kind == crate::reserved::Kind::Runner);
    let runner = runner_reserved_words.find(|word| match p.extension() {
          Some(ext) => ext == word.word,
          None => false,
        });
    match (direction, runner) {
      (Some(d), Some(r)) => Some((p, d, *r)),
      _ => None {},
    }
  }
  let mut entries = fs::read_dir(".")?
  .map(|res| res.map(|e| e.path() ).unwrap() )
  .map(|p| has_proper_name(p) ).collect();

  Ok(entries);
}

fn extract_timestamp(p: PathBuf) -> Result<chrono::NaiveDateTime, &'static str> {
  // Search for "SEPARATOR\d{14}_" (dir separator, 14 digits, underscore)
  // Note: cannot use FORMAT_STR.len() here because %Y is 2 chars, but wants 4
  let re = regex::Regex::new(format!(r#"{}(\d{{14}})_"#, regex::escape(format!("{}", std::path::MAIN_SEPARATOR).as_str())).as_str()).unwrap();
  println!("re: {:?} path: {:?}", re, p);
  match re.captures(p.to_str().expect("path to_str failed")) {
    None => Err("pattern did not match"),
    Some(c) => {
      match c.get(1) {
        Some(m) => match chrono::NaiveDateTime::parse_from_str(m.as_str(), FORMAT_STR) {
          Ok(ndt) => Ok(ndt),
          Err(_) => Err("timestamp did not parse"),
        },
        None => Err("no capture group")
      }
    }
  }

  // // https://codereview.stackexchange.com/a/98547
  // fn basename<'a>(path: &'a str) -> std::borrow::Cow<'a, str> {
  //   let mut pieces = path.rsplit(std::path::MAIN_SEPARATOR);
  //   match pieces.next() {
  //       Some(p) => p.into(),
  //       None => path.into(),
  //   }
  // }

  //   // Whether a file, or a directory (with migration files) the `basename`
  //   // that ./we/accept/deep/nested/TIMESTAMP_fooo_files/
  //   match basename(p
  //       .to_str()
  //       .ok_or_else(|| "could not call to_str")?
  //   )
  //       .split(|x| x == std::path::MAIN_SEPARATOR || x == '_' || x == '.')
  //       .collect::<Vec<&str>>()
  //       .first() 
  //   {
  //       Some(first_part) => match chrono::NaiveDateTime::parse_from_str(first_part, FORMAT_STR) {
  //           Ok(ndt) => Ok(ndt),
  //           Err(_) => Err("timestamp did not parse"),
  //       },
  //       None => Err("could not get first part"),
  //   }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_basename() -> Result<(),()> {
      let filename = "test/fixtures/example-1-simple-mixed-migrations/migrations/20200904205000_get_es_health.es-docker.curl";
      assert_eq!(basename(filename), "20200904205000_get_es_health.es-docker.curl");
      Ok(())
    }

    #[test]
    fn text_extract_timestamp() -> Result<(), &'static str> {
      let p = PathBuf::from("test/fixtures/example-1-simple-mixed-migrations/migrations/20200904205000_get_es_health.es-docker.curl");
      extract_timestamp(p)?;

      let p = PathBuf::from("test/fixtures/example-1-simple-mixed-migrations/migrations/example.curl");
      match extract_timestamp(p) {
        Ok(_) => Err("should not have extracted anything"),
        _ => Ok(())
      }

    }

    #[test]
    fn test_the_fixture_returns_correct_results() -> Result<(), &'static str> {
      println!("current dir is {:?}", std::env::current_dir().expect("blah"));
      let result = migrations_in(Path::new("test/fixtures")).expect("migrations_in failed");
      assert_eq!(result.len(), 3);
      Ok(())
    }
}
