use super::reserved;
use chrono::{NaiveDate, NaiveDateTime};
use std::path::{Path, PathBuf};

// See https://docs.rs/chrono/0.3.1/chrono/format/strftime/index.html
pub const FORMAT_STR: &'static str = "%Y%m%d%H%M%S";

pub struct Parsed<'a> {
  path: &'a Path,
  date_time: NaiveDateTime,
  flags: Vec<reserved::Word>,
}

pub fn parse(p: &Path) -> Option<Parsed> {
  if !reserved::words()
    .iter()
    .filter(|word| word.kind == reserved::Kind::Runner)
    .any(|word| match p.extension() {
      Some(ext) => ext == word.word,
      _ => false,
    })
  {
    return None;
  }

  let file_name = match p.file_name() {
    None => return None,
    Some(file_name) => file_name.to_str().unwrap(),
  };

  let parts: Vec<&str> = file_name.split("_").collect();
  let dt = match NaiveDateTime::parse_from_str(parts[0], FORMAT_STR) {
    Ok(date_time) => date_time,
    Err(_) => return None,
  };

  // get basename, check for timestamp at beignning
  // if so check for one or more reserved words
  // otherwise return nothing

  return Some(Parsed {
    path: p,
    date_time: dt,
    flags: vec![],
  });
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
  fn test_paths_with_no_timestamp_are_none() {
    match parse(std::path::Path::new("./foo/bar.curl")) {
      Some(_) => panic!("shoud have been none"),
      None => assert!(true),
    }
  }

  #[test]
  fn test_parses_the_timestamp_correctly() {
    match parse(std::path::Path::new("./foo/20200716120300_bar.curl")) {
      Some(parsed) => assert_eq!(
        parsed.date_time,
        NaiveDate::from_ymd(2020, 7, 16).and_hms(12, 03, 00)
      ),
      None => panic!("expected path to be parsable"),
    }
  }

  #[test]
  fn test_includes_the_given_path_in_the_response() {
    let p = std::path::Path::new("./foo/bar.curl");

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
