//! Reserved words.
//! 🛈 Be mindful to perform
//! case insensitive comparisons on runner names, the configuration file for example
//! is not required to be capitalized any particular way.

use colored::*;

#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Hash)]
/// Runners contain a name and a list of file extensions which they know how to handle. Multiple runners
/// may support the same file-names, the selecting factor is whether a [`crate::config::RunnerConfiguration`] exists
/// for that combination of runner name and file extension when attempting to apply migrations.
pub struct RunnerMeta<'a> {
    pub name: RunnerName<'a>,
    pub desc: &'static str,
    pub exts: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Flags contain no logic, simply a meaning and a name. The supported flags are `[risky, long, data]`
pub struct Flag {
    pub name: &'static str,
    pub meaning: &'static str,
}

impl std::fmt::Display for Flag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.name {
            "data" => write!(f, "{}", "data".blue()),
            "risky" => write!(f, "{}", "risky".bright_red().on_white()),
            "long" => write!(f, "{}", "long".white().bold().on_red()),
            name => write!(f, "{}", name),
        }
    }
}

/// Reserved words are either of type [Runner] or [Flag]
/// the runner type is for the runners and state storage engines that are supported.
/// The flag type is for the data-flags such as long-running, data, risky, etc which
/// may be used to annotate migrations which maybe shouldn't be run right away.
/// Words may be reserved even if there is no associated implementation (yet.)
pub enum ReservedWord<'a> {
    Runner(RunnerMeta<'a>),
    Flag(Flag),
}

// Prefer `const` here because we can't use `static &str` in
// pattern matches, but const we can. Probably related to elision
// of statics into inlines.

type RunnerName<'a> = &'a str;

/// Const GNU "Bash3". **Currently not supported.**
pub const BASH_3: RunnerName = "Bash3";
/// Const GNU "Bash4". **Currently not supported.**
pub const BASH_4: RunnerName = "Bash4";
/// Const "HTTP".
pub const HTTP: RunnerName = "HTTP";
/// Const "Elasticsearch". **Currently not supported.**
pub const ELASTICSEARCH: RunnerName = "Elasticsearch";
/// Const "Kafka". **Currently not supported.**
pub const KAFKA: RunnerName = "Kafka";
/// Const "MariaDB". Reserve this along side MySQL
pub const MARIA_DB: RunnerName = "MariaDB";
/// Const "MySQL". Prefered over MariaDB due to commonness of usage.
pub const MYSQL: RunnerName = "MySQL";
/// Const "Python3". No Python 2 support is planned. **Currently not supported.**
pub const PYTHON_3: RunnerName = "Python3";
/// Const "Rails". Target the latest version of Rails. **Currently not supported.**
pub const RAILS: RunnerName = "Rails";
/// Const "Redis". **Currently not supported.**
pub const REDIS: RunnerName = "Redis";
/// Const "PostgreSQL". **Currently not supported.**
pub const POSTGRESQL: RunnerName = "Postgres";

/// Return all words in a `Vec<ReservedWord>` of enums.
pub fn words<'a>() -> Vec<ReservedWord<'a>> {
    vec![
    ReservedWord::Runner(RunnerMeta {
        name: MARIA_DB,
        desc: "Synonym of MySQL, please prefer MySQL keyword in general",
        exts: vec!["sql"],
    }),
    ReservedWord::Runner(RunnerMeta {
      name: MYSQL,
      desc: "MySQL by Oracle",
      exts: vec!["sql"],
    }),
    ReservedWord::Runner(RunnerMeta {
        name: REDIS,
        desc: "The screaming fast in-memory object store",
        exts: vec!["redis"],
    }),
    ReservedWord::Runner(RunnerMeta {
      name: HTTP,
      desc: "HTTP",
      exts: vec!["get", "post", "delete", "patch"],
    }),
    ReservedWord::Runner(RunnerMeta {
      name: ELASTICSEARCH,
      desc: ELASTICSEARCH,
      exts: vec!["es"],
    }),
    ReservedWord::Runner(RunnerMeta {
      name: BASH_3,
      desc: "GNU Bash 3",
      exts: vec!["sh", "bash3"],
    }),
    ReservedWord::Runner(RunnerMeta {
      name: POSTGRESQL,
      desc: "PostgreSQL",
      exts: vec!["sql"],
    }),
    ReservedWord::Runner(RunnerMeta {
      name: BASH_4,
      desc: "GNU Bash 4",
      exts: vec!["sh", "bash4"],
    }),
      ReservedWord::Runner(RunnerMeta {
        name: RAILS,
        desc: "Ruby on Rails (5.x or above)",
        exts: vec!["rb"],
    }),
      ReservedWord::Runner(RunnerMeta {
        name: PYTHON_3,
        desc: "Python 3",
        exts: vec!["py", "py3"],
    }),
    ReservedWord::Runner(RunnerMeta {
      name: KAFKA,
      desc: "Kafka",
      exts: vec!["kafka"],
    }),
    ReservedWord::Runner(RunnerMeta {
      name: POSTGRESQL,
      desc: "PostgreSQL",
      exts: vec!["sql"],
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

pub fn flags() -> impl Iterator<Item = Flag> {
    words().into_iter().filter_map(|word| match word {
        ReservedWord::Flag(f) => Some(f),
        _ => None {},
    })
}

/// Given a list like "a,b", returns the matching Flags{}
pub fn flags_from_str_flags(s: &str) -> Vec<Flag> {
    s.split(',')
        .filter_map(|p| {
            words().into_iter().find_map(|w| match w {
                ReservedWord::Flag(f) => match f.name == p {
                    true => Some(f),
                    _ => None,
                },
                _ => None,
            })
        })
        .collect()
}

/// Filters the reserved words to return only the runner [`words`].
pub fn runners<'a>() -> impl Iterator<Item = RunnerMeta<'a>> {
    words().into_iter().filter_map(|word| match word {
        ReservedWord::Runner(r) => Some(r),
        _ => None {},
    })
}

/// Maybe return a [`Runner`] by **strictly** matching including case sensitivity
// TODO: rename to runner_meta_by_name
pub fn runner_by_name(s: RunnerName) -> Option<RunnerMeta> {
    runners().find(|r| r.name.to_lowercase() == s.to_lowercase())
}

#[cfg(test)]
mod tests {

    use itertools::Itertools;

    use super::*;

    #[test]
    fn test_words() {
        assert!(runners().any(|v| v.name == "HTTP"));
    }

    #[test]
    fn test_flags_from_string_flags() {
        let flags: Vec<Flag> = words()
            .into_iter()
            .filter_map(|w| match w {
                ReservedWord::Flag(f) => Some(f),
                _ => None,
            })
            .collect();
        let flags_str = flags
            .clone()
            .into_iter()
            .filter_map(|f| Some(f.name))
            .join(",");

        assert_eq!(flags, flags_from_str_flags(&flags_str));
    }
}
