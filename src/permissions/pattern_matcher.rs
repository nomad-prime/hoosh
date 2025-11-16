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
            return pattern
                .split('|')
                .any(|p| self.matches_single(p.trim(), target));
        }

        self.matches_single(pattern, target)
    }
}

impl BashPatternMatcher {
    fn matches_single(&self, pattern: &str, target: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        if let Some(prefix) = pattern.strip_suffix(":*") {
            target.starts_with(prefix)
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
        assert!(matcher.matches("cargo:*|npm:*", "cargo build"));
        assert!(matcher.matches("cargo:*|npm:*", "npm install"));
        assert!(!matcher.matches("cargo:*|npm:*", "rustc --version"));
    }

    #[test]
    fn test_bash_pattern_matcher_compound_with_spaces() {
        let matcher = BashPatternMatcher;
        assert!(matcher.matches("cat:* | grep:* | wc:*", "cat file.txt"));
        assert!(matcher.matches("cat:* | grep:* | wc:*", "grep pattern"));
        assert!(matcher.matches("cat:* | grep:* | wc:*", "wc -l output.txt"));
        assert!(!matcher.matches("cat:* | grep:* | wc:*", "rm file.txt"));
    }

    #[test]
    fn test_bash_pattern_matcher_compound_three_commands() {
        let matcher = BashPatternMatcher;
        let pattern = "cat:*|grep:*|head:*";
        assert!(matcher.matches(pattern, "cat Cargo.toml"));
        assert!(matcher.matches(pattern, "grep -E pattern"));
        assert!(matcher.matches(pattern, "head -3"));
        assert!(!matcher.matches(pattern, "tail -n 5"));
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
}
