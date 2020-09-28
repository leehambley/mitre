extern crate pretty_env_logger;
#[macro_use]
use super::reserved;
use chrono::NaiveDateTime;
use log::{info, trace, warn};
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

#[cfg(test)]
use chrono::NaiveDate;

// See https://docs.rs/chrono/0.3.1/chrono/format/strftime/index.html
pub static FORMAT_STR: &str = "%Y%m%d%H%M%S";

#[derive(Debug)]
pub struct Parsed {
  pub path: std::path::PathBuf,
  pub date_time: NaiveDateTime,
  pub flags: Vec<reserved::Word>,
}

pub fn parse(p: &Path) -> Option<Parsed> {
  let reserved_words = reserved::words();
  let mut runner_reserved_words = reserved_words
    .iter()
    .filter(|word| word.kind == reserved::Kind::Runner);

  if runner_reserved_words.any(|word| match p.extension() {
    Some(ext) => {
      trace!("{:?} == {} ({})", ext, word.word, ext == word.word);
      return ext == word.word;
    }
    _ => {
      warn!(
        "{} file extension is a reserved word, skipping {}",
        p.display(),
        word.word
      );
      return false;
    }
  }) {
    warn!(
      "{} file extension ({:?}) is a reserved word",
      p.display(),
      p.extension()
    );
    return None;
  }

  let file_name = match p.file_name() {
    // p is a std::path::Path
    Some(file_name) => match file_name.to_str() {
      Some(file_name) => file_name,
      None => {
        trace!("{} couldn't be turned into a str", p.display());
        return None; // open file deleted, no namy anymore?
      }
    },
    None => {
      debug!(
        "{} has no filename anymore (file removed or renamed?)",
        p.display()
      );
      return None;
    } // open file deleted, no namy anymore?,
  };
  let parts: Vec<&str> = file_name.split('_').collect();

  let dt = match NaiveDateTime::parse_from_str(parts[0], FORMAT_STR) {
    Ok(date_time) => date_time,
    Err(e) => {
      debug!(
        "{} could not parse a date from the file extension ({})",
        p.display(),
        e
      );
      return None;
    }
  };

  // get basename, check for timestamp at beignning
  // if so check for one or more reserved words
  // otherwise return nothing
  eprintln!("found some parsed {:?}", p.to_path_buf());
  Some(Parsed {
    path: p.to_path_buf(),
    date_time: dt,
    flags: vec![],
  })
}

#[cfg(test)]
mod tests {

  use super::*;

  #[test]
  fn test_paths_with_no_extension_are_none() {
    match parse(std::path::Path::new("./foo/bar")) {
      Some(_) => panic!("shoud have been none"),
      None => assert!(true),
    }
  }

  #[test]
  fn test_paths_with_no_reserved_word_extension_are_some() {
    match parse(std::path::Path::new("./foo/bar.es-docker")) {
      Some(_) => assert!(true),
      None => panic!("shoud have been none"),
    }
  }

  #[test]
  fn test_paths_with_no_timestamp_are_none() {
    match parse(std::path::Path::new("./foo/bar.curl")) {
      Some(_) => panic!("shoud have been none"),
      None => assert!(true),
    }
  }

  #[test]
  fn test_parses_the_timestamp_correctly() -> Result<(), &'static str> {
    match parse(std::path::Path::new("./foo/20200716120300_bar.curl")) {
      Some(parsed) => {
        assert_eq!(
          parsed.date_time,
          NaiveDate::from_ymd(2020, 7, 16).and_hms(12, 03, 00)
        );
        Ok(())
      }
      None => Err("expected to parse"),
    }
  }

  #[test]
  fn test_includes_the_given_path_in_the_response() {
    let p = std::path::Path::new("./foo/20200716120300_bar.curl");

    match parse(p) {
      Some(parsed) => assert_eq!(parsed.path, p),
      None => panic!("expected path to be parsable"),
    }
  }

  #[test]
  fn test_includes_the_given_path_in_the_response_when_is_a_dir() {
    let p = std::path::Path::new("./foo/20200716120300_bar.curl/");

    match parse(p) {
      Some(parsed) => assert_eq!(parsed.path, p),
      None => panic!("expected path to be parsable"),
    }
  }

  #[test]
  fn test_returns_result_error_if_no_timestamp_in_the_filename() {
    let some_datetime = NaiveDate::from_ymd(2016, 7, 8).and_hms(9, 10, 11);
    let some_timestamp = some_datetime.format(FORMAT_STR);

    let mut path = PathBuf::new();
    path.push("foo");
    path.push("bar");
    path.push(format!(
      "{}_some_thing_here.curl",
      some_timestamp.to_string()
    ));

    match parse(path.as_path()) {
      Some(_) => assert!(true),
      None => panic!("expected path to be parsable"),
    }
  }

  // unsupportted runner
  // use of reserved word out of place
  // dot separated parts not at end of filename
}
