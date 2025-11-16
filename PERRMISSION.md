# Ticket: Add Command Risk Classification to Bash Tool

## Summary
Implement a whitelist-based command risk classification system for the bash tool to automatically approve safe read-only commands without prompting the user, matching Claude Code's behavior.

## Background
Currently, Hoosh asks for permission on every bash command execution. Claude Code has a more intelligent system that:
- Automatically runs safe read-only commands (like `find`, `ls`, `grep`)
- Only prompts for commands that could modify the system or files
- Properly handles command chains (e.g., `cat file | grep pattern` is safe, `find | xargs sed` needs approval)

## Goals
1. Implement command risk classification that identifies safe vs needs-review commands
2. Auto-approve commands where all base commands are whitelisted
3. Maintain existing permission system for non-whitelisted commands
4. Keep the architecture clean and testable

## Technical Approach

### 1. Create `bash/classifier.rs`

```rust
/// Risk level for bash commands based on their potential for system modification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRisk {
    /// All commands in the chain are whitelisted read-only operations
    Safe,
    /// At least one command is not whitelisted and needs user approval
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
    pub fn classify(command: &str) -> CommandRisk {
        use crate::tools::bash::BashCommandParser;
        
        let base_commands = BashCommandParser::extract_base_commands(command);
        
        // If ALL commands are whitelisted, it's safe
        if base_commands.iter().all(|c| Self::is_whitelisted(c)) {
            CommandRisk::Safe
        } else {
            CommandRisk::NeedsReview
        }
    }
    
    /// Check if a command is on the whitelist of safe read-only operations
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
```

### 2. Update `bash/mod.rs`

Add the new module:
```rust
mod parser;
mod tool;
mod classifier;

pub use parser::BashCommandParser;
pub use tool::BashTool;
pub use classifier::{BashCommandClassifier, CommandRisk};
```

### 3. Update `bash/tool.rs`

Modify `describe_permission` to mark safe commands:

```rust
fn describe_permission(&self, target: Option<&str>) -> ToolPermissionDescriptor {
    use super::{BashCommandParser, BashCommandClassifier};

    let target_str = target.unwrap_or("*");

    // Check if command is entirely safe
    if BashCommandClassifier::classify(target_str) == CommandRisk::Safe {
        // Mark as read-only so it can be auto-approved
        return ToolPermissionBuilder::new(self, target_str)
            .into_read_only()
            .with_approval_title(" Bash Command ")
            .with_approval_prompt(format!("Can I run \"{}\"", target_str))
            .with_persistent_approval("don't ask me again for bash in this project".to_string())
            .with_suggested_pattern("*".to_string())
            .with_pattern_matcher(Arc::new(BashPatternMatcher))
            .build()
            .expect("Failed to build BashTool permission descriptor")
    }

    // Existing logic for commands that need approval
    let base_commands = BashCommandParser::extract_base_commands(target_str);
    let suggested_pattern = BashCommandParser::suggest_pattern(&base_commands);

    // ... rest of existing code unchanged
}
```

### 4. Add Tests in `bash/classifier.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_single_command() {
        assert_eq!(BashCommandClassifier::classify("ls -la"), CommandRisk::Safe);
        assert_eq!(BashCommandClassifier::classify("find . -name '*.rs'"), CommandRisk::Safe);
        assert_eq!(BashCommandClassifier::classify("cat README.md"), CommandRisk::Safe);
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
        assert_eq!(BashCommandClassifier::classify("cargo build"), CommandRisk::NeedsReview);
        assert_eq!(BashCommandClassifier::classify("sed -i 's/test/TEST/g' file.txt"), CommandRisk::NeedsReview);
        assert_eq!(BashCommandClassifier::classify("rm file.txt"), CommandRisk::NeedsReview);
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
        // Ensure all whitelisted commands are recognized
        let safe_commands = vec![
            "ls", "pwd", "cat", "head", "tail", "find", "grep",
            "wc", "sort", "echo", "which", "date"
        ];
        
        for cmd in safe_commands {
            assert!(BashCommandClassifier::is_whitelisted(cmd));
            assert_eq!(BashCommandClassifier::classify(cmd), CommandRisk::Safe);
        }
    }
}
```

## Integration Points

The permission checking code (likely in your conversation loop or permission manager) should check for read-only commands and auto-approve:

```rust
// Wherever you check permissions
let descriptor = tool.describe_permission(Some(target));

// Auto-approve safe read-only commands
if descriptor.is_read_only() {
    return Ok(true);  // Auto-approve
}

// Otherwise, go through normal permission flow
// ... existing permission check logic
```

## Testing Plan

1. **Unit tests** - Test the classifier with various command combinations (added above)
2. **Integration test** - Test that safe commands don't trigger permission dialogs
3. **Manual testing** - Try the examples from Claude Code:
    - `find . -name "*.md" -type f` should not ask
    - `find . -name "*.md" | head -3 | xargs sed` should ask
    - `cat Cargo.toml | grep version` should not ask

## Success Criteria

- [ ] Safe read-only commands execute without permission prompts
- [ ] Commands with any non-whitelisted component still require approval
- [ ] All tests pass
- [ ] No regression in existing permission system behavior
- [ ] Code follows style guide (trait-based, well-tested, minimal dependencies)


### Quick Fix after

also when I start the project I write these into permissions.json, if read is not allowed, opening the project is not possible anyway, so we should not really add them to permission file and keep it lean

so this initial permission write after init_permission

```json
{
  "version": 1,
  "allow": [
    {
      "operation": "read_file",
      "pattern": "*"
    },
    {
      "operation": "list_directory",
      "pattern": "*"
    },
    {
      "operation": "write_file",
      "pattern": "*"
    },
    {
      "operation": "edit_file",
      "pattern": "*"
    },
    {
      "operation": "task",
      "pattern": "*"
    },
    {
      "operation": "glob",
      "pattern": "*"
    }
  ],
  "deny": []
}
```

should be 

```json
{
  "version": 1,
  "allow": [],
  "deny": []
}
```
