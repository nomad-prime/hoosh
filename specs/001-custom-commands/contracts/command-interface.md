# Contract: Custom Command Interface

**Feature**: Custom Commands
**Date**: 2025-12-09

## Overview

This contract defines the interface between custom commands and Hoosh's command system. Custom commands integrate with the existing `Command` trait and `CommandRegistry`.

## Command Trait Interface

Custom commands MUST implement the `Command` trait defined in `src/commands/registry.rs`:

```rust
#[async_trait]
pub trait Command: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn aliases(&self) -> Vec<&str>;
    fn usage(&self) -> &str;
    async fn execute(
        &self,
        args: Vec<String>,
        context: &mut CommandContext,
    ) -> Result<CommandResult>;
}
```

## Implementation Contract

### Name

- **Returns**: Command name without leading slash
- **Source**: Derived from filename (without .md extension)
- **Example**: File `analyze.md` → name() returns `"analyze"`
- **Constraints**: Must be valid filename (OS-enforced), lowercase

### Description

- **Returns**: Brief description of command purpose
- **Source**: `description` field from YAML frontmatter
- **Example**: `"Analyze codebase for technical debt"`
- **Constraints**: Non-empty after trimming whitespace

### Aliases

- **Returns**: Empty vector (no aliases in MVP)
- **Future**: Could support aliases from frontmatter

### Usage

- **Returns**: Usage string showing command syntax
- **Format**: `"/{name} [args...]"`
- **Example**: `"/analyze [directory]"`

### Execute

- **Parameters**:
  - `args`: Command arguments (whitespace-split after command name)
  - `context`: Mutable reference to `CommandContext` with conversation, tools, etc.

- **Returns**: `Result<CommandResult>` where `CommandResult` is:
  ```rust
  pub enum CommandResult {
      Success(String),      // Command executed successfully
      Exit,                 // Request to exit Hoosh
      ClearConversation,    // Request to clear conversation
  }
  ```

- **Behavior**:
  1. Extract command body from `ParsedCommand`
  2. Substitute `$ARGUMENTS` placeholder with joined args
  3. Add processed body to conversation as user message
  4. Return `CommandResult::Success` with confirmation message

- **Error Handling**: Return `Err(anyhow::Error)` with context on failure

## Registration Contract

### Registration Order

1. **Built-in commands** registered FIRST
2. **Custom commands** registered SECOND (after built-ins)

**Implication**: Built-in commands take precedence in case of name conflicts

### Conflict Resolution

**IF** custom command name conflicts with built-in:
- Log warning message
- Skip registration of custom command
- Built-in command remains active

**Example**:
```rust
if registry.commands.contains_key(&name) {
    eprintln!("Warning: Custom command '{}' conflicts with built-in, skipping", name);
    continue;
}
```

### Registry API

```rust
pub struct CommandRegistry {
    // ... fields ...
}

impl CommandRegistry {
    pub fn register(&mut self, command: Arc<dyn Command>) -> Result<()>;
    pub async fn execute(&self, input: &str, context: &mut CommandContext) -> Result<CommandResult>;
}
```

**Registration**: Custom commands wrapped in `Arc<CustomCommandWrapper>` and passed to `registry.register()`

## Command Context Contract

Custom commands receive `CommandContext` with following fields:

```rust
pub struct CommandContext {
    pub conversation: Option<Arc<tokio::sync::Mutex<Conversation>>>,
    pub tool_registry: Option<Arc<ToolRegistry>>,
    pub agent_manager: Option<Arc<AgentDefinitionManager>>,
    pub command_registry: Option<Arc<CommandRegistry>>,
    pub working_directory: String,
    pub permission_manager: Option<Arc<PermissionManager>>,
    // ... other fields
}
```

**Required Fields for Custom Commands**:
- `conversation`: Used to add custom command body as user message

**Optional Fields**:
- All others may be `None` depending on execution context

**Access Pattern**:
```rust
let conversation = context.conversation
    .as_ref()
    .ok_or_else(|| anyhow!("No conversation available"))?;
```

## Argument Handling Contract

### Argument Parsing

**Input**: Raw user input string (e.g., `/analyze src/ --verbose`)

**Parsing**:
1. Strip leading `/`
2. Split by whitespace
3. First token = command name
4. Remaining tokens = args vector

**Example**:
```
Input: "/analyze src/ --verbose"
Command name: "analyze"
Args: ["src/", "--verbose"]
```

### Argument Substitution

**Placeholder**: `$ARGUMENTS` in command body

**Substitution**:
```rust
let args_str = args.join(" ");
let processed_body = command.body.replace("$ARGUMENTS", &args_str);
```

**Example**:

Command body:
```markdown
Analyze the directory: $ARGUMENTS
```

With args `["src/", "--verbose"]`:
```markdown
Analyze the directory: src/ --verbose
```

**No arguments**:
```markdown
Analyze the directory:
```

## Error Contract

### Error Types

Custom commands MUST return errors following Hoosh's error handling patterns:

```rust
use anyhow::{Context, Result, anyhow};

// File errors
Err(e) => e.context("Failed to read command file")

// Validation errors
anyhow::bail!("Command body is empty")

// Missing context
context.conversation.as_ref()
    .ok_or_else(|| anyhow!("No conversation available"))?
```

### Error Messages

Error messages MUST include:
1. **What failed**: Clear description of operation
2. **Context**: File path, command name, or relevant details
3. **Actionable guidance**: What user should do to fix (when applicable)

**Example**:
```
Error: Failed to load custom command from '.hoosh/commands/analyze.md'

Caused by:
    Invalid YAML frontmatter at line 5: expected mapping

Fix: Ensure YAML frontmatter is properly formatted:
---
description: Your description here
---
```

## Execution Flow Contract

### Standard Execution Flow

```
1. User Input: /commandname arg1 arg2
2. CommandRegistry::execute()
   ↓
3. Parse command name and args
   ↓
4. Lookup command in registry (HashMap)
   ↓
5. CustomCommandWrapper::execute(args, context)
   ↓
6. Process command body (substitute $ARGUMENTS)
   ↓
7. Add to conversation as user message
   ↓
8. Trigger agent response (handled by session)
   ↓
9. Return CommandResult::Success
```

### Async Execution

All command execution is async:

```rust
#[async_trait]
pub trait Command: Send + Sync {
    async fn execute(...) -> Result<CommandResult>;
}
```

**Requirements**:
- Command implementations MUST be `Send + Sync`
- Execute method returns `Future`
- Compatible with tokio runtime

## Lifecycle Contract

### Load Time (Startup)

1. `CustomCommandManager::new()` - Create manager, ensure directory exists
2. `manager.load_commands()` - Parse all .md files
3. `manager.register_commands(registry)` - Register with CommandRegistry

**Timing**: During Hoosh initialization, before TUI starts

### Runtime

- Commands are **immutable** after registration
- No dynamic reloading (MVP)
- Commands remain in memory until Hoosh exits

### Cleanup

- No explicit cleanup required
- Commands deallocated when Hoosh exits
- No persistent state to clean up

## Testing Contract

### Unit Test Requirements

Custom command implementation MUST have tests for:

1. **Valid command parsing**: Correctly parse frontmatter and body
2. **Missing frontmatter**: Return appropriate error
3. **Malformed YAML**: Return parse error with context
4. **Empty description**: Validation error
5. **Argument substitution**: $ARGUMENTS replaced correctly
6. **No arguments**: $ARGUMENTS replaced with empty string

### Integration Test Requirements

End-to-end tests MUST verify:

1. **Command registration**: Custom command appears in registry
2. **Command execution**: Invocation adds message to conversation
3. **Name conflicts**: Built-in takes precedence
4. **Error resilience**: One bad file doesn't break all commands

## Backwards Compatibility

### Future-Proofing

The frontmatter format supports optional fields via `#[serde(default)]`:

```rust
#[derive(Deserialize)]
struct CommandMetadata {
    pub description: String,  // Required - no default

    #[serde(default)]
    pub handoffs: Vec<Handoff>,  // Optional - defaults to []

    #[serde(default)]
    pub new_field: Option<NewType>,  // Future field - defaults to None
}
```

**Contract**: Existing command files MUST continue working when new optional fields are added to `CommandMetadata`.

## Security Contract

### Sandboxing

Custom commands execute within Hoosh's existing permission system:

- No direct file system access (beyond reading command files at startup)
- No arbitrary code execution
- Command body is treated as user input to agent
- Agent tool execution governed by existing permission manager

### Input Validation

- **File size limit**: 1MB maximum per command file
- **Filename validation**: OS-enforced (valid filesystem names)
- **YAML validation**: serde_yaml enforces structure
- **No script injection**: Command body is markdown, not code

## References

- Command Trait: `src/commands/registry.rs:126-138`
- CommandContext: `src/commands/registry.rs:19-32`
- CommandResult: `src/commands/registry.rs:13-17`
- Async Trait Pattern: Used throughout Hoosh (async_trait crate)
