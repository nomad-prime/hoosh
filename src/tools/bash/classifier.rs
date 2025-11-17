use super::BashCommandParser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRisk {
    Safe,
    NeedsReview,
}

pub struct BashCommandClassifier;

impl BashCommandClassifier {
    /// Classify a bash command based on the base commands it contains
    ///
    /// Examples:
    /// - `find . -name "*.rs"` -> Safe
    /// - `cat file | grep error` -> Safe
    /// - `find | xargs sed` -> NeedsReview (sed not whitelisted)
    /// - `cargo build` -> NeedsReview (cargo not whitelisted)
    /// - `cat <<EOF` -> NeedsReview (heredocs can create arbitrary content)
    pub fn classify(command: &str) -> CommandRisk {
        // Heredocs are never safe - they can create arbitrary content
        if Self::contains_heredoc(command) {
            return CommandRisk::NeedsReview;
        }

        let base_commands = BashCommandParser::extract_base_commands(command);

        if base_commands.iter().all(|c| Self::is_whitelisted(c)) {
            CommandRisk::Safe
        } else {
            CommandRisk::NeedsReview
        }
    }

    /// Check if command contains a heredoc
    fn contains_heredoc(command: &str) -> bool {
        command.contains("<<") || command.contains("<<<")
    }

    fn is_whitelisted(cmd: &str) -> bool {
        matches!(
            cmd,
            // File/directory reading
            "ls" | "pwd" | "cat" | "head" | "tail" | "less" | "more"
            | "find" | "tree" | "stat" | "file"

            // Text processing (read-only)
            | "grep" | "egrep" | "fgrep"
            | "wc" | "sort" | "uniq" | "diff"

            // System info
            | "echo" | "which" | "type"
            | "whoami" | "hostname" | "date"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_single_command() {
        assert_eq!(BashCommandClassifier::classify("ls -la"), CommandRisk::Safe);
        assert_eq!(
            BashCommandClassifier::classify("find . -name '*.rs'"),
            CommandRisk::Safe
        );
        assert_eq!(
            BashCommandClassifier::classify("cat README.md"),
            CommandRisk::Safe
        );
    }

    #[test]
    fn test_safe_pipeline() {
        assert_eq!(
            BashCommandClassifier::classify("cat file.txt | grep error"),
            CommandRisk::Safe
        );
        assert_eq!(
            BashCommandClassifier::classify("find . -name '*.md' | head -3"),
            CommandRisk::Safe
        );
        assert_eq!(
            BashCommandClassifier::classify("cat Cargo.toml | grep version | wc -l"),
            CommandRisk::Safe
        );
    }

    #[test]
    fn test_needs_review_single_command() {
        assert_eq!(
            BashCommandClassifier::classify("cargo build"),
            CommandRisk::NeedsReview
        );
        assert_eq!(
            BashCommandClassifier::classify("sed -i 's/test/TEST/g' file.txt"),
            CommandRisk::NeedsReview
        );
        assert_eq!(
            BashCommandClassifier::classify("rm file.txt"),
            CommandRisk::NeedsReview
        );
    }

    #[test]
    fn test_needs_review_mixed_pipeline() {
        assert_eq!(
            BashCommandClassifier::classify("find . -name '*.md' | xargs sed -i 's/test/TEST/g'"),
            CommandRisk::NeedsReview
        );
        assert_eq!(
            BashCommandClassifier::classify("cat file.txt | sed 's/foo/bar/'"),
            CommandRisk::NeedsReview
        );
    }

    #[test]
    fn test_needs_review_complex_chain() {
        assert_eq!(
            BashCommandClassifier::classify("cargo build && cargo test"),
            CommandRisk::NeedsReview
        );
        assert_eq!(
            BashCommandClassifier::classify("ls -la; cargo build; echo done"),
            CommandRisk::NeedsReview
        );
    }

    #[test]
    fn test_whitelist_coverage() {
        let safe_commands = vec![
            "ls", "pwd", "cat", "head", "tail", "find", "grep", "wc", "sort", "echo", "which",
            "date",
        ];

        for cmd in safe_commands {
            assert!(BashCommandClassifier::is_whitelisted(cmd));
            assert_eq!(BashCommandClassifier::classify(cmd), CommandRisk::Safe);
        }
    }

    #[test]
    fn test_heredoc_needs_review() {
        // Heredocs should always need review, even with safe commands
        assert_eq!(
            BashCommandClassifier::classify("cat <<EOF\nHello\nEOF"),
            CommandRisk::NeedsReview
        );
        assert_eq!(
            BashCommandClassifier::classify("cat <<'EOF'\nHello\nEOF"),
            CommandRisk::NeedsReview
        );
        assert_eq!(
            BashCommandClassifier::classify("echo <<EOF\ntest\nEOF"),
            CommandRisk::NeedsReview
        );
    }

    #[test]
    fn test_herestring_needs_review() {
        assert_eq!(
            BashCommandClassifier::classify("cat <<< \"some text\""),
            CommandRisk::NeedsReview
        );
    }
}
