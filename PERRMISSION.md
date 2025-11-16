Got it! Then just give me a clean ticket for fixing the pattern matching security issue:

# Ticket: Fix Bash Pattern Matching Security Bypass

## Summary
Fix critical security vulnerability in `BashPatternMatcher` where approving a multi-command pipeline incorrectly allows individual dangerous commands to execute without approval.

## Problem

The current `BashPatternMatcher` uses OR logic for multi-command patterns. When a user approves a pipeline like `find . | head -3 | xargs sed`, the pattern `find:*|head:*|xargs:*|sed:*` is stored. However, this pattern incorrectly matches `sed` alone:

```rust
// User approves: find . | head -3 | xargs sed
// Pattern stored: "find:*|head:*|xargs:*|sed:*"

// Later, LLM executes:
matcher.matches("find:*|head:*|xargs:*|sed:*", "sed -i 's/secret/leaked/' config.txt")
// Returns: true ❌ WRONG! Should require approval again.
```

**Root cause:** In `pattern_matcher.rs`, line with `.any()`:

```rust
if pattern.contains('|') {
    return pattern
        .split('|')
        .any(|p| self.matches_single(p.trim(), target));  // ❌ OR logic
}
```

This treats the `|` as OR when it should mean "ALL commands must be present" (AND logic).

## Solution

Change pattern matching to require ALL commands in a multi-command pattern to be present in the target.

**File: `src/permissions/pattern_matcher.rs`**

Replace the `BashPatternMatcher` implementation:

```rust
use crate::tools::bash::BashCommandParser;

pub struct BashPatternMatcher;

impl PatternMatcher for BashPatternMatcher {
    fn matches(&self, pattern: &str, target: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        // Multi-command patterns require ALL commands present
        if pattern.contains('|') {
            return self.matches_multicommand(pattern, target);
        }

        self.matches_single(pattern, target)
    }
}

impl BashPatternMatcher {
    /// Match multi-command pattern - ALL commands must be present
    fn matches_multicommand(&self, pattern: &str, target: &str) -> bool {
        // Extract command names from pattern (strip :* suffix)
        let pattern_commands: Vec<&str> = pattern
            .split('|')
            .map(|p| p.trim().trim_end_matches(":*").trim_end_matches('*'))
            .filter(|s| !s.is_empty())
            .collect();
        
        // Extract commands from target
        let target_commands = BashCommandParser::extract_base_commands(target);
        
        // ALL pattern commands must exist in target
        pattern_commands.iter().all(|pattern_cmd| {
            target_commands.iter().any(|target_cmd| target_cmd == pattern_cmd)
        })
    }
    
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
```

## Tests

Add these tests to the existing `tests` module in `pattern_matcher.rs`:

```rust
#[test]
fn test_multicommand_requires_all_commands() {
    let matcher = BashPatternMatcher;
    let pattern = "find:*|head:*|xargs:*|sed:*";
    
    // Should match when ALL commands present
    assert!(matcher.matches(
        pattern,
        "find . -name '*.md' | head -3 | xargs sed -i 's/test/TEST/g'"
    ));
    
    // Should NOT match when only subset present
    assert!(!matcher.matches(pattern, "sed -i 's/test/TEST/g' file.txt"));
    assert!(!matcher.matches(pattern, "find . -name '*.md'"));
    assert!(!matcher.matches(pattern, "find . | head -3"));
    assert!(!matcher.matches(pattern, "xargs sed -i 's/foo/bar/'"));
    assert!(!matcher.matches(pattern, "head -3 | xargs sed -i 's/a/b/'"));
}

#[test]
fn test_security_bypass_prevented() {
    let matcher = BashPatternMatcher;
    
    // User approved pipeline, should NOT allow solo dangerous command
    let approved_pipeline = "find:*|head:*|xargs:*|sed:*";
    let dangerous_solo = "sed -i 's/password=old/password=leaked/' config.txt";
    
    assert!(!matcher.matches(approved_pipeline, dangerous_solo));
}

#[test]
fn test_three_command_pipeline() {
    let matcher = BashPatternMatcher;
    let pattern = "cat:*|grep:*|head:*";
    
    // All three present - match
    assert!(matcher.matches(pattern, "cat Cargo.toml | grep version | head -1"));
    
    // Only two present - no match
    assert!(!matcher.matches(pattern, "cat Cargo.toml | grep version"));
    
    // Only one present - no match
    assert!(!matcher.matches(pattern, "grep version"));
}

#[test]
fn test_order_independence() {
    let matcher = BashPatternMatcher;
    let pattern = "cat:*|grep:*|wc:*";
    
    // Order in target doesn't matter
    assert!(matcher.matches(pattern, "cat file.txt | grep error | wc -l"));
    assert!(matcher.matches(pattern, "wc -l file.txt | cat | grep something"));
}

#[test]
fn test_single_command_patterns_unchanged() {
    let matcher = BashPatternMatcher;
    
    // Single command patterns should work as before
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
```

Update existing test that is now broken:

```rust
#[test]
fn test_bash_pattern_matcher_compound() {
    let matcher = BashPatternMatcher;
    
    // Pattern with multiple commands requires ALL to be present
    assert!(matcher.matches("cargo:*|npm:*", "cargo build && npm install"));
    assert!(!matcher.matches("cargo:*|npm:*", "cargo build"));  // Changed: now requires both
    assert!(!matcher.matches("cargo:*|npm:*", "npm install"));  // Changed: now requires both
    assert!(!matcher.matches("cargo:*|npm:*", "rustc --version"));
}

#[test]
fn test_bash_pattern_matcher_compound_with_spaces() {
    let matcher = BashPatternMatcher;
    let pattern = "cat:* | grep:* | wc:*";
    
    // All three must be present
    assert!(matcher.matches(pattern, "cat file.txt | grep pattern | wc -l"));
    assert!(!matcher.matches(pattern, "cat file.txt"));  // Changed
    assert!(!matcher.matches(pattern, "grep pattern"));  // Changed
    assert!(!matcher.matches(pattern, "rm file.txt"));
}

#[test]
fn test_bash_pattern_matcher_compound_three_commands() {
    let matcher = BashPatternMatcher;
    let pattern = "cat:*|grep:*|head:*";
    
    // All three must be present
    assert!(matcher.matches(pattern, "cat Cargo.toml | grep version | head -1"));
    assert!(!matcher.matches(pattern, "cat Cargo.toml"));  // Changed
    assert!(!matcher.matches(pattern, "grep -E pattern"));  // Changed
    assert!(!matcher.matches(pattern, "head -3"));  // Changed
    assert!(!matcher.matches(pattern, "tail -n 5"));
}
```

## Manual Testing

1. Start hoosh in a test project
2. Ask LLM to run: `find . -name "*.md" | head -3 | xargs sed -i.bak 's/test/TEST/g'`
3. Choose option 2 (approve and don't ask again)
4. Verify pattern `find:*|head:*|xargs:*|sed:*` is stored in `.hoosh/permissions.json`
5. Ask LLM to run: `sed -i 's/foo/bar/' some_file.txt`
6. **Expected:** Permission dialog appears (security fix working)
7. **Before fix:** Would execute without asking (security bypass)

## Success Criteria

- [ ] All new tests pass
- [ ] All existing tests pass (after updating the 3 broken ones)
- [ ] Manual testing confirms security fix works
- [ ] `cargo clippy` clean
- [ ] `cargo fmt` applied

## Notes

This is a breaking change in pattern matching semantics:
- **Before:** `find:*|sed:*` matched if ANY command present (OR)
- **After:** `find:*|sed:*` matches only if ALL commands present (AND)

Since you're not in production yet, no migration needed. Users will just need to re-approve some patterns after this fix.
