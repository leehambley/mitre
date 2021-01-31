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
    name: &'static str,
    meaning: &'static str,
}

pub enum ReservedWord {
    Runner(Runner),
    Flag(Flag),
}

pub fn reserved_words() -> Vec<ReservedWord> {
    vec![
    ReservedWord::Runner(Runner{
      name: "MariaDB",
      desc: "MariaDB by the MariaDB Foundation",
      exts: vec!["sql"]
    }),
    ReservedWord::Runner(Runner{
      name: "cURL",
      desc: "cURL",
      exts: vec!["curl"]
    }),
    ReservedWord::Runner(Runner{
      name: "bash3",
      desc: "GNU Bash 3",
      exts: vec!["sh", "bash3"]
    }),
    ReservedWord::Runner(Runner{
      name: "bash4",
      desc: "GNU Bash 4",
      exts: vec!["sh", "bash4"]
    }),
    ReservedWord::Runner(Runner{
      name: "python3",
      desc: "Python 3",
      exts: vec!["py", "py3"]
    }),
    ReservedWord::Runner(Runner{
      name: "rails",
      desc: "Ruby on Rails (5.x or above)",
      exts: vec!["rb"]
    }),
    ReservedWord::Runner(Runner{
      name: "kafka",
      desc: "Kafka (11.x or above)",
      exts: vec!["kafka"]
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

pub fn words() -> Vec<Word> {
    return vec![
        Word {
            word: "bash3",
            reason: "Used as an extension to activate the Bash (v3) runner.",
            kind: Kind::Runner,
        },
        Word {
            word: "bash4",
            reason: "Used as an extension to activate the Bash (v4) runner.",
            kind: Kind::Runner,
        },
        Word {
            word: "python3",
            reason: "Used an extension to activate the Python (v3) runner.",
            kind: Kind::Runner,
        },
        Word {
            word: "curl",
            reason: "Used as an extension to activate the cURL runner.",
            kind: Kind::Runner,
        },
        Word {
            word: "rails",
            reason: "Used as an extension to activate the (Ruby on) Rails runner.",
            kind: Kind::Runner,
        },
        Word {
          word: "mysql",
          reason: "Runs mysql migrations (SQL in a .sql file)",
          kind: Kind::Runner,
        },
        Word {
            word: "sh",
            reason: "Used as an extension to activate the POSIX sh runner.",
            kind: Kind::Runner,
        },
        Word {
            word: "kafka",
            reason: "Used as an extension to activate the Kafka runner.",
            kind: Kind::Runner,
        },
        Word {
            word: "data",
            reason: "Indicates this is a data migration (advisory only)",
            kind: Kind::Flag,
        },
        Word {
            word: "long",
            reason: "Indicates this may be long running (advisory only, e.g changing an index)",
            kind: Kind::Flag,
        },
        Word {
            word: "risky",
            reason: "Indicates this is risky migration (advisory only, e.g renaming a column)",
            kind: Kind::Flag,
        },
        Word {
          word: "redis",
          reason: "Indicates this is a Redis runner, files should be formatted in the way you could pipe them to redis-cli",
          kind: Kind::Runner,
      },
        Word {
            word: "up",
            reason: "Used to indicate upward (forward) migrations",
            kind: Kind::Direction,
        },
        Word {
            word: "change",
            reason: "Used to indicate change migrations (no implied direction)",
            kind: Kind::Direction,
        },
        Word {
            word: "down",
            reason: "Used to indicate down (backwards) migrations",
            kind: Kind::Direction,
        },
    ];
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_words() {
        assert!(words().iter().any(|v| v.word == "curl"));
    }
}
