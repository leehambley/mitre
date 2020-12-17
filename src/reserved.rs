use std::fmt;

#[derive(Debug, PartialEq, Eq)]
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

#[derive(Debug)]
pub struct Word {
    pub word: &'static str,
    pub reason: &'static str,
    pub kind: Kind,
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
