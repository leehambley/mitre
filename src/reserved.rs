use std::fmt;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Kind {
    Runner,
    Flag,
    Direction,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub struct Word {
    pub word: &'static str,
    pub reason: &'static str,
    pub kind: Kind,
}

#[derive(Debug, Clone)]
pub struct Runner {
    pub name: &'static str,
    pub desc: &'static str,
    pub exts: Vec<&'static str>,
}

#[derive(Debug, Clone)]
pub struct Flag {
    pub name: &'static str,
    pub meaning: &'static str,
}

pub enum ReservedWord {
    Runner(Runner),
    Flag(Flag),
}

/// Static constantts for runner names. Be mindful to perform
/// case insensitive comparisons, the configuration file for example
/// is not required to be capitalized any particular way.
pub static BASH_3: &str = "Bash3";
pub static BASH_4: &str = "Bash4";
pub static CURL: &str = "cURL";
pub static KAFKA: &str = "Kafka";
pub static MARIA_DB: &str = "MariaDB";
pub static PYTHON_3: &str = "Python3";
pub static RAILS: &str = "Rails";
pub static REDIS: &str = "Redis";

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

pub fn runners() -> impl Iterator<Item = Runner> {
    words().into_iter().filter_map(|word| match word {
        ReservedWord::Runner(r) => Some(r),
        _ => None {},
    })
}

/// Strictly matching including case sensitivity
pub fn runner_by_name(s: Option<&String>) -> Option<Runner> {
    match s {
        Some(ss) => runners().find(|r| r.name == ss),
        None => None {},
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_words() {
        assert!(runners().any(|v| v.name == "cURL"));
    }
}
