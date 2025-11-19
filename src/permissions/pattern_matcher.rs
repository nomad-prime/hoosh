use crate::tools::bash::BashCommandParser;
use glob::Pattern;

/// Trait for pattern matching logic specific to each tool type
pub trait PatternMatcher: Send + Sync {
    /// Check if a pattern matches a target string
    fn matches(&self, pattern: &str, target: &str) -> bool;
}

pub struct BashPatternMatcher;

impl PatternMatcher for BashPatternMatcher {
    fn matches(&self, pattern: &str, target: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.contains('|') {
            return self.matches_multicommand(pattern, target);
        }

        self.matches_single(pattern, target)
    }
}

impl BashPatternMatcher {
    fn matches_multicommand(&self, pattern: &str, target: &str) -> bool {
        let pattern_commands: Vec<&str> = pattern
            .split('|')
            .map(|p| {
                p.trim()
                    .trim_end_matches(":*")
                    .trim_end_matches('*')
                    .trim_end_matches(":<<") // <--- Fix: Also trim heredoc suffix
            })
            .filter(|s| !s.is_empty())
            .collect();

        let target_commands = BashCommandParser::extract_base_commands(target);

        pattern_commands.iter().all(|pattern_cmd| {
            target_commands
                .iter()
                .any(|target_cmd| target_cmd == pattern_cmd)
        })
    }

    fn matches_single(&self, pattern: &str, target: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        // <--- Fix: Handle "cat:<<" and "*:<<" patterns
        if let Some(prefix) = pattern.strip_suffix(":<<") {
            // If pattern is "*:<<", just check for heredoc presence
            if prefix == "*" {
                return target.contains("<<");
            }

            // Otherwise check if command starts with prefix AND has heredoc
            let clean_target = target.trim();

            if clean_target.starts_with(prefix) {
                let rest = &clean_target[prefix.len()..];
                // Ensure we matched a full word
                let valid_word_boundary = rest.is_empty() || rest.starts_with(' ');

                return valid_word_boundary && target.contains("<<");
            }
            return false;
        }

        // Handle "cargo build:*"
        if let Some(prefix) = pattern.strip_suffix(":*") {
            let clean_target = target.trim();

            // Logic: Does the target start with the prefix?
            // Prefix: "cargo build"
            // Target: "cargo build --release" -> Match
            // Target: "cargo install" -> No Match

            // We need to be careful about boundaries.
            // "cargo b" should not match "cargo build" if the pattern was partial.
            // But since we generate patterns from full tokens, starts_with is usually okay
            // IF we ensure a boundary (space or end of string).

            if clean_target.starts_with(prefix) {
                let rest = &clean_target[prefix.len()..];
                // Ensure we matched a full word (matches "cargo build" or "cargo build " but not "cargo builder")
                return rest.is_empty() || rest.starts_with(' ');
            }
            false
        } else {
            pattern == target
        }
    }
}

/// Pattern matcher for file paths using glob patterns
pub struct FilePatternMatcher;

impl PatternMatcher for FilePatternMatcher {
    fn matches(&self, pattern: &str, target: &str) -> bool {
        Pattern::new(pattern)
            .ok()
            .map(|p| p.matches(target))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_pattern_matcher_wildcard() {
        let matcher = BashPatternMatcher;
        assert!(matcher.matches("*", "any command"));
        assert!(matcher.matches("*", ""));
    }

    #[test]
    fn test_bash_pattern_matcher_heredoc() {
        let matcher = BashPatternMatcher;

        // Test exact command match
        assert!(matcher.matches("cat:<<", "cat <<EOF\nhello\nEOF"));

        // Test wildcard heredoc match
        assert!(matcher.matches("*:<<", "cat <<EOF\nhello\nEOF"));
        assert!(matcher.matches("*:<<", "grep pattern <<EOF\nhello\nEOF"));

        // Test mismatch
        assert!(!matcher.matches("cat:<<", "grep <<EOF")); // Wrong command
        assert!(!matcher.matches("cat:<<", "cat file.txt")); // No heredoc
    }

    #[test]
    fn test_bash_pattern_matcher_prefix() {
        let matcher = BashPatternMatcher;
        assert!(matcher.matches("cargo:*", "cargo build"));
        assert!(matcher.matches("cargo:*", "cargo test --release"));
        assert!(!matcher.matches("cargo:*", "npm build"));
    }

    #[test]
    fn test_bash_pattern_matcher_exact() {
        let matcher = BashPatternMatcher;
        assert!(matcher.matches("echo hello", "echo hello"));
        assert!(!matcher.matches("echo hello", "echo world"));
    }

    #[test]
    fn test_bash_pattern_matcher_compound() {
        let matcher = BashPatternMatcher;
        assert!(matcher.matches("cargo:*|npm:*", "cargo build && npm install"));
        assert!(!matcher.matches("cargo:*|npm:*", "cargo build"));
        assert!(!matcher.matches("cargo:*|npm:*", "npm install"));
        assert!(!matcher.matches("cargo:*|npm:*", "rustc --version"));
    }

    #[test]
    fn test_bash_pattern_matcher_compound_with_spaces() {
        let matcher = BashPatternMatcher;
        let pattern = "cat:* | grep:* | wc:*";
        assert!(matcher.matches(pattern, "cat file.txt | grep pattern | wc -l"));
        assert!(!matcher.matches(pattern, "cat file.txt"));
        assert!(!matcher.matches(pattern, "grep pattern"));
        assert!(!matcher.matches(pattern, "rm file.txt"));
    }

    #[test]
    fn test_bash_pattern_matcher_compound_three_commands() {
        let matcher = BashPatternMatcher;
        let pattern = "cat:*|grep:*|head:*";
        assert!(matcher.matches(pattern, "cat Cargo.toml | grep version | head -1"));
        assert!(!matcher.matches(pattern, "cat Cargo.toml"));
        assert!(!matcher.matches(pattern, "grep -E pattern"));
        assert!(!matcher.matches(pattern, "head -3"));
        assert!(!matcher.matches(pattern, "tail -n 5"));
    }

    #[test]
    fn test_multicommand_requires_all_commands() {
        let matcher = BashPatternMatcher;
        let pattern = "find:*|head:*|xargs:*";

        assert!(matcher.matches(
            pattern,
            "find . -name '*.md' | head -3 | xargs sed -i 's/test/TEST/g'"
        ));

        assert!(!matcher.matches(pattern, "xargs sed -i 's/test/TEST/g'"));
        assert!(!matcher.matches(pattern, "find . -name '*.md'"));
        assert!(!matcher.matches(pattern, "find . | head -3"));
        assert!(!matcher.matches(pattern, "head -3 | xargs sed -i 's/a/b/'"));
    }

    #[test]
    fn test_security_bypass_prevented() {
        let matcher = BashPatternMatcher;
        let pattern = "find:*|head:*|xargs:*";

        assert!(!matcher.matches(pattern, "xargs rm -rf /"));
        assert!(!matcher.matches(pattern, "find . -name '*.txt'"));
        assert!(!matcher.matches(pattern, "head -n 10 file.txt"));
    }

    #[test]
    fn test_multicommand_all_three_present() {
        let matcher = BashPatternMatcher;
        let pattern = "cat:*|grep:*|head:*";

        assert!(matcher.matches(pattern, "cat Cargo.toml | grep version | head -1"));

        assert!(!matcher.matches(pattern, "cat Cargo.toml | grep version"));

        assert!(!matcher.matches(pattern, "grep version"));
    }

    #[test]
    fn test_order_independence() {
        let matcher = BashPatternMatcher;
        let pattern = "cat:*|grep:*|wc:*";

        assert!(matcher.matches(pattern, "cat file.txt | grep error | wc -l"));
        assert!(matcher.matches(pattern, "wc -l file.txt | cat | grep something"));
    }

    #[test]
    fn test_single_command_patterns_unchanged() {
        let matcher = BashPatternMatcher;

        assert!(matcher.matches("cargo:*", "cargo build"));
        assert!(matcher.matches("cargo:*", "cargo test --release"));
        assert!(!matcher.matches("cargo:*", "npm build"));

        assert!(matcher.matches("sed:*", "sed -i 's/a/b/' file.txt"));
        assert!(matcher.matches("sed:*", "sed 's/foo/bar/'"));
    }

    #[test]
    fn test_wildcard_unchanged() {
        let matcher = BashPatternMatcher;
        assert!(matcher.matches("*", "any command"));
        assert!(matcher.matches("*", "sed dangerous stuff"));
        assert!(matcher.matches("*", ""));
    }

    #[test]
    fn test_file_pattern_matcher_glob() {
        let matcher = FilePatternMatcher;
        assert!(matcher.matches("/src/**", "/src/main.rs"));
        assert!(matcher.matches("/src/**", "/src/lib/mod.rs"));
        assert!(!matcher.matches("/src/**", "/tests/test.rs"));
    }

    #[test]
    fn test_file_pattern_matcher_exact() {
        let matcher = FilePatternMatcher;
        assert!(matcher.matches("/config.toml", "/config.toml"));
        assert!(!matcher.matches("/config.toml", "/src/config.toml"));
    }

    #[test]
    fn test_file_pattern_matcher_wildcard() {
        let matcher = FilePatternMatcher;
        assert!(matcher.matches("*", "anything"));
        assert!(matcher.matches("*", "/any/path"));
    }

    #[test]
    fn test_pattern_matching_security_no_prefix_bypass() {
        let matcher = BashPatternMatcher;

        // Should match
        assert!(matcher.matches("cargo:*", "cargo build"));
        assert!(matcher.matches("cargo:*", "cargo test --release"));
        assert!(matcher.matches("npm:*", "npm install"));

        // Should NOT match - different command entirely
        assert!(!matcher.matches("cargo:*", "cargoship"));
        assert!(!matcher.matches("cargo:*", "cargowithanything"));

        // Edge cases
        assert!(!matcher.matches("ls:*", "lsof"));
        assert!(!matcher.matches("cat:*", "catch"));
    }
}
