//! Reserved words.
//! 🛈 Be mindful to perform
//! case insensitive comparisons on runner names, the configuration file for example
//! is not required to be capitalized any particular way.

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// Runners contain a name and a list of file extensions which they know how to handle. Multiple runners
/// may support the same file-names, the selecting factor is whether a [`crate::config::RunnerConfiguration`] exists
/// for that combination of runner name and file extension when attempting to apply migrations.
pub struct Runner {
    pub name: &'static str,
    pub desc: &'static str,
    pub exts: Vec<&'static str>,
}

#[derive(Debug, Clone)]
/// Flags contain no logic, simply a meaning and a name. The supported flags are `[risky, long, data]`
pub struct Flag {
    pub name: &'static str,
    pub meaning: &'static str,
}

/// Reserved words are either of type [Runner] or [Flag]
/// the runner type is for the runners and state storage engines that are supported.
/// The flag type is for the data-flags such as long-running, data, risky, etc which
/// may be used to annotate migrations which maybe shouldn't be run right away.
/// Words may be reserved even if there is no associated implementation (yet.)
pub enum ReservedWord {
    Runner(Runner),
    Flag(Flag),
}

// Prefer `const` here because we can't use `static &str` in
// pattern matches, but const we can. Probably related to elision
// of statics into inlines.

/// Const GNU "Bash3". **Currently not supported.**
pub const BASH_3: &str = "Bash3";
/// Const GNU "Bash4". **Currently not supported.**
pub const BASH_4: &str = "Bash4";
/// Const "cURL". Copyright <https://curl.se/>. **Currently not supported.**
pub const CURL: &str = "cURL";
/// Const "Kafka". **Currently not supported.**
pub const KAFKA: &str = "Kafka";
/// Const "MariaDB". Prefer this name over MySQL due to Oracle. This package supports MariaDB and MySQL insofar as they are interoperable.
pub const MARIA_DB: &str = "MariaDB";
/// Const "Python3". No Python 2 support is planned. **Currently not supported.**
pub const PYTHON_3: &str = "Python3";
/// Const "Rails". Target the latest version of Rails. **Currently not supported.**
pub const RAILS: &str = "Rails";
/// Const "Redis". **Currently not supported.**
pub const REDIS: &str = "Redis";
/// Const "PostgreSQL". **Currently not supported.**
pub const POSTGRESQL: &str = "Postgres";

/// Return all words in a `Vec<ReservedWord>` of enums.
pub fn words() -> Vec<ReservedWord> {
    vec![
    ReservedWord::Runner(Runner {
      name: MARIA_DB,
      desc: "MariaDB by the MariaDB Foundation",
      exts: vec!["sql"],
  }),
    ReservedWord::Runner(Runner {
      name: REDIS,
      desc: "The screaming fast in-memory object store",
      exts: vec!["redis"],
  }),
    ReservedWord::Runner(Runner {
      name: CURL,
      desc: "cURL",
      exts: vec!["curl"],
  }),
    ReservedWord::Runner(Runner {
      name: BASH_3,
      desc: "GNU Bash 3",
      exts: vec!["sh", "bash3"],
  }),
    ReservedWord::Runner(Runner {
      name: POSTGRESQL,
      desc: "PostgreSQL",
      exts: vec![".sql"],
    }),
    ReservedWord::Runner(Runner {
      name: BASH_4,
      desc: "GNU Bash 4",
      exts: vec!["sh", "bash4"],
  }),
    ReservedWord::Runner(Runner {
      name: RAILS,
      desc: "Ruby on Rails (5.x or above)",
      exts: vec!["rb"],
  }),
    ReservedWord::Runner(Runner {
      name: PYTHON_3,
      desc: "Python 3",
      exts: vec!["py", "py3"],
  }),
    ReservedWord::Runner(Runner {
      name: KAFKA,
      desc: "Kafka",
      exts: vec!["kafka"],
    }),
    ReservedWord::Flag(Flag{
      name: "data",
      meaning: "This is a data migration affecting data only, not structure." 
    }),
    ReservedWord::Flag(Flag{
      name: "long",
      meaning: "This is a long-running migration, apps may want to boot without those, and run them out-of-hours." 
    }),
    ReservedWord::Flag(Flag{
      name: "risky",
      meaning: "This is a risky migration, maybe should be run outside peak times with more human observation" 
    })
  ]
}

/// Filters the reserved words to return only the runner [`words`].
pub fn runners() -> impl Iterator<Item = Runner> {
    words().into_iter().filter_map(|word| match word {
        ReservedWord::Runner(r) => Some(r),
        _ => None {},
    })
}

/// Maybe return a [`Runner`] by **strictly** matching including case sensitivity
pub fn runner_by_name(s: &String) -> Option<Runner> {
    runners().find(|r| r.name == s)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_words() {
        assert!(runners().any(|v| v.name == "cURL"));
    }
}
