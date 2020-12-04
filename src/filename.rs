extern crate env_logger;
use super::reserved;

use chrono::NaiveDateTime;
use log::{info, warn};
use std::fmt::Debug;
use std::path::Path;

#[cfg(test)]
use chrono::NaiveDate;
#[cfg(test)]
use std::path::PathBuf;

// See https://docs.rs/chrono/0.3.1/chrono/format/strftime/index.html
pub static FORMAT_STR: &str = "%Y%m%d%H%M%S";

#[derive(Debug)]
pub struct Parsed {
    pub path: std::path::PathBuf,
    pub date_time: NaiveDateTime,
    pub flags: Vec<reserved::Word>,
}

#[derive(Debug)]
pub enum ProblemReason {
    // Must contain some extension, callers do not care about this
    // error, we are using Result<T,E> as a more informative Some()
    // to pass info up, incase they would care.
    #[allow(dead_code)]
    HasNoExtension,
    // Extensions must not be reserved words
    NoRunnerReservedWord,
    // Format must be FORMAT_STR
    TimestampDidNotParse,
    // File was renamed/removed whilst we had it open, now it has no name
    HasNoName,
}

// These are not errors per-se, they are problem, which make this
// filename ineligible for use for some reason. Preferred Result<Parsed,Problem>
// over Some() vs. None() to make it easier to inspect (and ignore) reasons
// further up the stack.
#[derive(Debug)]
pub struct Problem {
    reason: ProblemReason,
    hint: String,
}

pub fn parse(p: &Path) -> Result<Parsed, Problem> {
    let reserved_words = reserved::words();
    let mut runner_reserved_words = reserved_words
        .iter()
        .filter(|word| word.kind == reserved::Kind::Runner);

    match p.extension() {
        Some(_) => {}
        None => {
            return Err(Problem {
                reason: ProblemReason::HasNoExtension,
                hint: format!("filename {} has no extension", p.display(),),
            })
        }
    }

    let contains_any_runner_reserved_word = runner_reserved_words.any(|word| match p.extension() {
        Some(ext) => {
            if ext == word.word {
                println!("{:?} matches {}", ext, word.word);
            }
            true
        }
        _ => {
            info!(
                "{} file extension is a reserved word, skipping {}",
                p.display(),
                word.word
            );
            false
        }
    });

    println!(
        "contains any reserved word {:?}",
        contains_any_runner_reserved_word,
    );

    if !contains_any_runner_reserved_word {
        warn!(
            "{} file extension ({:?}) is a reserved word",
            p.display(),
            p.extension()
        );
        return Err(Problem {
            reason: ProblemReason::NoRunnerReservedWord,
            hint: format!(
                "filename {} contains {:?}",
                p.display(),
                p.extension().expect("COULD NOT GET EXTENSON")
            ),
        });
    }

    let file_name = match p.file_name() {
        Some(filename) => match filename.to_str() {
            Some(filename) => filename,
            None => panic!("to_str a filename failed (oom?)"),
        },
        _ => {
            return Err(Problem {
                reason: ProblemReason::HasNoName,
                hint: "no filename for opened file, moved or renamed?".to_string(),
            })
        }
    };

    let parts: Vec<&str> = file_name.split('_').collect();

    let dt = match NaiveDateTime::parse_from_str(parts[0], FORMAT_STR) {
        Ok(date_time) => date_time,
        Err(e) => {
            return Err(Problem {
                reason: ProblemReason::TimestampDidNotParse,
                hint: format!(
                    "{} could not parse a date from the file extension ({})",
                    p.display(),
                    e
                ),
            });
        }
    };

    Ok(Parsed {
        path: p.to_path_buf(),
        date_time: dt,
        flags: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_paths_with_no_extension_or_timestamp_are_err() {
        match parse(std::path::Path::new("./foo/bar")) {
            Ok(_) => panic!("shoud have been err"),
            Err(e) => match e.reason {
                ProblemReason::HasNoExtension => assert!(true),
                _ => panic!("wrong reason code {:?}", e.reason),
            },
        }
    }

    #[test]
    fn test_paths_with_reserved_word_extension_are_ok() {
        match parse(std::path::Path::new("./foo/20160708091011_bar.curl")) {
            Ok(_) => assert!(true),
            Err(e) => panic!("should not have been {:?}", e),
        }
    }

    fn test_paths_with_no_runner_reserved_word_extension() {
        match parse(std::path::Path::new("./foo/20160708091011_bar.curl")) {
            Ok(_) => assert!(true),
            Err(e) => panic!("should not have been {:?}", e),
        }
    }

    #[test]
    fn test_paths_with_no_timestamp_are_err() {
        match parse(std::path::Path::new("./foo/bar.curl.my-conf")) {
            Ok(_) => panic!("shoud have been none"),
            Err(_) => assert!(true),
        }
    }

    #[test]
    fn test_parses_the_timestamp_correctly() -> Result<(), &'static str> {
        match parse(std::path::Path::new(
            "./foo/20200716120300_bar.curl.my-conf",
        )) {
            Ok(parsed) => {
                assert_eq!(
                    parsed.date_time,
                    NaiveDate::from_ymd(2020, 7, 16).and_hms(12, 03, 00)
                );
                Ok(())
            }
            Err(e) => panic!("expected to parse {:?}", e),
        }
    }

    #[test]
    fn test_includes_the_given_path_in_the_response() {
        let p = std::path::Path::new("./foo/20200716120300_bar.docker-es");

        match parse(p) {
            Ok(parsed) => assert_eq!(parsed.path, p),
            Err(e) => panic!("expected path to be parsable, {:?}", e),
        }
    }

    #[test]
    fn test_includes_the_given_path_in_the_response_when_is_a_dir() {
        let p = std::path::Path::new("./foo/20200716120300_bar.curl/");

        match parse(p) {
            Ok(parsed) => assert_eq!(parsed.path, p),
            Err(e) => panic!("expected path to be parsable {:?}", e),
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
            Ok(_) => assert!(true),
            Err(e) => panic!("expected path to be parsable {:?}", e),
        }
    }

    // unsupportted runner
    // use of reserved word out of place
    // dot separated parts not at end of filename
}
