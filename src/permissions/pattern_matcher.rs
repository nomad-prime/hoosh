use glob::Pattern;

/// Trait for pattern matching logic specific to each tool type
pub trait PatternMatcher: Send + Sync {
    /// Check if a pattern matches a target string
    fn matches(&self, pattern: &str, target: &str) -> bool;
}

/// Pattern matcher for bash commands
/// Supports:
/// - "*" matches everything
/// - "command:*" matches commands starting with "command"
/// - "exact command" matches exactly
pub struct BashPatternMatcher;

impl PatternMatcher for BashPatternMatcher {
    fn matches(&self, pattern: &str, target: &str) -> bool {
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
