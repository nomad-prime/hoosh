use glob::Pattern;
use std::path::PathBuf;

use crate::tools::bash::{BashCommandParser, BashCommandPatternRegistry};

/// Trait for pattern matching logic specific to each tool type
pub trait PatternMatcher: Send + Sync {
    /// Check if a pattern matches a target string
    fn matches(&self, pattern: &str, target: &str) -> bool;
}

pub struct BashPatternMatcher {
    registry: BashCommandPatternRegistry,
    /// Optional working directory used to resolve the synthetic `cd:outside`
    /// pattern (which means "any cd that leaves cwd"). When absent, the
    /// pattern only matches by exact string.
    working_dir: Option<PathBuf>,
}

impl BashPatternMatcher {
    pub fn new() -> Self {
        Self {
            registry: BashCommandPatternRegistry::new(),
            working_dir: None,
        }
    }

    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    fn cd_leaves_working_dir(&self, target: &str) -> bool {
        let Some(ref cwd) = self.working_dir else {
            return false;
        };
        let canonical_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.clone());
        for arg in BashCommandParser::extract_cd_targets(target) {
            if arg.is_empty() || arg == "-" || arg.contains('$') || arg.contains('`') {
                return true;
            }
            let resolved = if std::path::Path::new(&arg).is_absolute() {
                PathBuf::from(&arg)
            } else {
                cwd.join(&arg)
            };
            let canonical = resolved.canonicalize().unwrap_or(resolved);
            if !canonical.starts_with(&canonical_cwd) {
                return true;
            }
        }
        false
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

        // Synthetic pattern from BashTool — matches whenever the target's
        // `cd` argument leaves the configured working directory.
        if pattern == "cd:outside" {
            return self.cd_leaves_working_dir(target);
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
        // Use & for chain patterns (&&, ||, ;)
        assert!(matcher.matches("cargo:*&npm:*", "cargo build && npm install"));
        assert!(!matcher.matches("cargo:*&npm:*", "cargo build"));
        assert!(!matcher.matches("cargo:*&npm:*", "npm install"));
        assert!(!matcher.matches("cargo:*&npm:*", "rustc --version"));
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
    fn test_multicommand_allows_trusted_subset_rejects_untrusted() {
        let matcher = BashPatternMatcher::new();
        let pattern = "find:*|head:*|xargs:*";

        // Full pipeline of trusted commands.
        assert!(matcher.matches(
            pattern,
            "find . -name '*.md' | head -3 | xargs sed -i 's/test/TEST/g'"
        ));
        // A subset of the trusted commands is still within trust (safe direction).
        assert!(matcher.matches(pattern, "find . | head -3"));
        assert!(matcher.matches(pattern, "head -3 | xargs sed -i 's/a/b/'"));

        // A single (non-compound) command does NOT match a multi-command
        // pipeline rule — it routes to the single-command matcher instead.
        assert!(!matcher.matches(pattern, "xargs sed -i 's/test/TEST/g'"));
        assert!(!matcher.matches(pattern, "find . -name '*.md'"));

        // An untrusted command anywhere in the pipeline rejects the whole thing.
        assert!(!matcher.matches(pattern, "find . | head -3 | rm -rf x"));
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
    fn test_multicommand_subset_matches_single_does_not() {
        let matcher = BashPatternMatcher::new();
        let pattern = "cat:*|grep:*|head:*";

        assert!(matcher.matches(pattern, "cat Cargo.toml | grep version | head -1"));
        // Subset of the trusted set is within trust.
        assert!(matcher.matches(pattern, "cat Cargo.toml | grep version"));
        // A lone command routes to the single-command matcher, which does not
        // honor a multi-command pipeline rule.
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
