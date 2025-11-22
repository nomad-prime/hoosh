use glob::Pattern;

use crate::tools::bash::BashCommandPatternRegistry;

/// Trait for pattern matching logic specific to each tool type
pub trait PatternMatcher: Send + Sync {
    /// Check if a pattern matches a target string
    fn matches(&self, pattern: &str, target: &str) -> bool;
}

pub struct BashPatternMatcher {
    registry: BashCommandPatternRegistry,
}

impl BashPatternMatcher {
    pub fn new() -> Self {
        Self {
            registry: BashCommandPatternRegistry::new(),
        }
    }
}

impl Default for BashPatternMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PatternMatcher for BashPatternMatcher {
    fn matches(&self, pattern: &str, target: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        // Delegate to registry
        if self.registry.matches_pattern(pattern, target) {
            return true;
        }

        // Fallback: exact match
        pattern == target
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
        let matcher = BashPatternMatcher::new();
        assert!(matcher.matches("*", "any command"));
        assert!(matcher.matches("*", ""));
    }

    #[test]
    fn test_bash_pattern_matcher_heredoc() {
        let matcher = BashPatternMatcher::new();

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
        let matcher = BashPatternMatcher::new();
        assert!(matcher.matches("cargo:*", "cargo build"));
        assert!(matcher.matches("cargo:*", "cargo test --release"));
        assert!(!matcher.matches("cargo:*", "npm build"));
    }

    #[test]
    fn test_bash_pattern_matcher_exact() {
        let matcher = BashPatternMatcher::new();
        assert!(matcher.matches("echo hello", "echo hello"));
        assert!(!matcher.matches("echo hello", "echo world"));
    }

    #[test]
    fn test_bash_pattern_matcher_compound() {
        let matcher = BashPatternMatcher::new();
        assert!(matcher.matches("cargo:*|npm:*", "cargo build && npm install"));
        assert!(!matcher.matches("cargo:*|npm:*", "cargo build"));
        assert!(!matcher.matches("cargo:*|npm:*", "npm install"));
        assert!(!matcher.matches("cargo:*|npm:*", "rustc --version"));
    }

    #[test]
    fn test_bash_pattern_matcher_compound_with_spaces() {
        let matcher = BashPatternMatcher::new();
        let pattern = "cat:* | grep:* | wc:*";
        assert!(matcher.matches(pattern, "cat file.txt | grep pattern | wc -l"));
        assert!(!matcher.matches(pattern, "cat file.txt"));
        assert!(!matcher.matches(pattern, "grep pattern"));
        assert!(!matcher.matches(pattern, "rm file.txt"));
    }

    #[test]
    fn test_bash_pattern_matcher_compound_three_commands() {
        let matcher = BashPatternMatcher::new();
        let pattern = "cat:*|grep:*|head:*";
        assert!(matcher.matches(pattern, "cat Cargo.toml | grep version | head -1"));
        assert!(!matcher.matches(pattern, "cat Cargo.toml"));
        assert!(!matcher.matches(pattern, "grep -E pattern"));
        assert!(!matcher.matches(pattern, "head -3"));
        assert!(!matcher.matches(pattern, "tail -n 5"));
    }

    #[test]
    fn test_multicommand_requires_all_commands() {
        let matcher = BashPatternMatcher::new();
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
        let matcher = BashPatternMatcher::new();
        let pattern = "find:*|head:*|xargs:*";

        assert!(!matcher.matches(pattern, "xargs rm -rf /"));
        assert!(!matcher.matches(pattern, "find . -name '*.txt'"));
        assert!(!matcher.matches(pattern, "head -n 10 file.txt"));
    }

    #[test]
    fn test_multicommand_all_three_present() {
        let matcher = BashPatternMatcher::new();
        let pattern = "cat:*|grep:*|head:*";

        assert!(matcher.matches(pattern, "cat Cargo.toml | grep version | head -1"));

        assert!(!matcher.matches(pattern, "cat Cargo.toml | grep version"));

        assert!(!matcher.matches(pattern, "grep version"));
    }

    #[test]
    fn test_order_independence() {
        let matcher = BashPatternMatcher::new();
        let pattern = "cat:*|grep:*|wc:*";

        assert!(matcher.matches(pattern, "cat file.txt | grep error | wc -l"));
        assert!(matcher.matches(pattern, "wc -l file.txt | cat | grep something"));
    }

    #[test]
    fn test_single_command_patterns_unchanged() {
        let matcher = BashPatternMatcher::new();

        assert!(matcher.matches("cargo:*", "cargo build"));
        assert!(matcher.matches("cargo:*", "cargo test --release"));
        assert!(!matcher.matches("cargo:*", "npm build"));

        assert!(matcher.matches("sed:*", "sed -i 's/a/b/' file.txt"));
        assert!(matcher.matches("sed:*", "sed 's/foo/bar/'"));
    }

    #[test]
    fn test_wildcard_unchanged() {
        let matcher = BashPatternMatcher::new();
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
        let matcher = BashPatternMatcher::new();

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
