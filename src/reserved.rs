pub struct Word {
    pub word: &'static str,
    pub reason: &'static str,
}

pub fn words() -> Vec<Word> {
    return vec![
        Word {
            word: "bash3",
            reason: "Used as an extension to activate the Bash (v3) runner.",
        },
        Word {
            word: "bash4",
            reason: "Used as an extension to activate the Bash (v4) runner.",
        },
        Word {
            word: "curl",
            reason: "Used as an extension to activate the cURL runner.",
        },
        Word {
            word: "rails",
            reason: "Used as an extension to activate the (Ruby on) Rails runner.",
        },
        Word {
            word: "sh",
            reason: "Used as an extension to activate the POSIX sh runner.",
        },
        Word {
            word: "kafka",
            reason: "Used as an extension to activate the Kafka runner.",
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
