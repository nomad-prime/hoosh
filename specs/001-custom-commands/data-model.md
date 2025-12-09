# Data Model: Custom Commands

**Feature**: Custom Commands
**Branch**: 001-custom-commands
**Date**: 2025-12-09

## Overview

This document defines the data structures and relationships for the custom commands feature. All structures follow Hoosh's architectural principles: modular organization, explicit error handling, and async-first design.

## Core Entities

### 1. CommandMetadata

Represents the YAML frontmatter metadata extracted from a custom command markdown file.

**Location**: `src/commands/custom/metadata.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMetadata {
    /// Brief description of what the command does (required)
    pub description: String,

    /// Optional list of handoff configurations
    #[serde(default)]
    pub handoffs: Vec<Handoff>,

    /// Optional tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
}
```

**Fields**:
- `description`: User-facing description shown in help/list outputs (required)
- `handoffs`: List of potential handoff actions (optional, defaults to empty vec)
- `tags`: Categorization labels for organizing commands (optional)

**Validation Rules**:
- `description` MUST NOT be empty after trimming whitespace
- All handoff entries MUST have non-empty `agent` and `label` fields

### 2. Handoff

Defines a handoff action that can be triggered from a custom command.

**Location**: `src/commands/custom/metadata.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handoff {
    /// Display label for the handoff action
    pub label: String,

    /// Target agent name to hand off to
    pub agent: String,

    /// Prompt/message to send when handing off
    pub prompt: String,

    /// Whether to automatically send (default: false)
    #[serde(default)]
    pub send: bool,
}
```

**Fields**:
- `label`: Human-readable text shown in UI (e.g., "Build Technical Plan")
- `agent`: Internal agent identifier (e.g., "speckit.plan")
- `prompt`: Template or instruction sent to target agent
- `send`: Auto-execute flag (false = show UI, true = immediate execution)

**Validation Rules**:
- `label` MUST NOT be empty
- `agent` MUST NOT be empty
- `prompt` MUST NOT be empty

### 3. ParsedCommand

Represents a fully parsed custom command ready for registration.

**Location**: `src/commands/custom/parser.rs`

```rust
#[derive(Debug, Clone)]
pub struct ParsedCommand {
    /// Command name derived from filename (without .md extension)
    pub name: String,

    /// Metadata extracted from YAML frontmatter
    pub metadata: CommandMetadata,

    /// Markdown body content (trimmed)
    pub body: String,
}
```

**Fields**:
- `name`: Command identifier used for invocation (e.g., "analyze" for `/analyze`)
- `metadata`: Structured frontmatter data
- `body`: Markdown content after frontmatter, used as command prompt

**Derivation Rules**:
- `name`: File stem (filename without .md extension) converted to lowercase
- `metadata`: Deserialized from YAML frontmatter section
- `body`: Extracted after closing `---` delimiter, whitespace trimmed

### 4. CustomCommandWrapper

Adapts `ParsedCommand` to implement the `Command` trait for registry integration.

**Location**: `src/commands/custom/wrapper.rs`

```rust
use async_trait::async_trait;
use crate::commands::{Command, CommandContext, CommandResult};

pub struct CustomCommandWrapper {
    command: ParsedCommand,
}

#[async_trait]
impl Command for CustomCommandWrapper {
    fn name(&self) -> &str {
        &self.command.name
    }

    fn description(&self) -> &str {
        &self.command.metadata.description
    }

    fn aliases(&self) -> Vec<&str> {
        Vec::new()  // No aliases for custom commands in MVP
    }

    fn usage(&self) -> &str {
        &format!("/{} [args...]", self.command.name)
    }

    async fn execute(
        &self,
        args: Vec<String>,
        context: &mut CommandContext,
    ) -> Result<CommandResult> {
        // Execute custom command logic
        // Details in implementation section below
    }
}
```

**Purpose**: Bridges parsed command data with Hoosh's command execution system

### 5. CustomCommandManager

Manages lifecycle of custom commands: discovery, loading, and registration.

**Location**: `src/commands/custom/manager.rs`

```rust
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

pub struct CustomCommandManager {
    /// Directory containing custom command files
    commands_dir: PathBuf,

    /// Cached loaded commands
    loaded_commands: Vec<ParsedCommand>,
}

impl CustomCommandManager {
    /// Create manager and ensure commands directory exists
    pub fn new() -> Result<Self>;

    /// Get the commands directory path (.hoosh/commands)
    fn commands_dir() -> Result<PathBuf>;

    /// Load all command files from directory
    pub fn load_commands(&mut self) -> Result<()>;

    /// Register loaded commands with command registry
    pub fn register_commands(&self, registry: &mut CommandRegistry) -> Result<()>;

    /// Get list of loaded command names
    pub fn list_commands(&self) -> Vec<&str>;
}
```

**Responsibilities**:
- Ensure `.hoosh/commands` directory exists (create if missing - FR-002)
- Scan directory for `.md` files
- Parse each file using `parser::parse_command_file()`
- Handle errors gracefully (log warnings, continue with valid files)
- Register commands with `CommandRegistry`

## Data Flow

### 1. Startup / Initialization

```
main.rs
  └─> session.rs::initialize()
      └─> CustomCommandManager::new()
          ├─> Check .hoosh/commands exists
          ├─> Create directory if missing
          └─> Return manager instance

      └─> manager.load_commands()
          ├─> Scan .hoosh/commands for *.md files
          ├─> For each file:
          │   ├─> parse_command_file(path)
          │   │   ├─> Read file content
          │   │   ├─> Extract frontmatter (gray_matter)
          │   │   ├─> Deserialize YAML -> CommandMetadata
          │   │   ├─> Validate metadata
          │   │   └─> Return ParsedCommand
          │   ├─> On success: add to loaded_commands
          │   └─> On error: log warning, continue
          └─> Return loaded count

      └─> manager.register_commands(registry)
          ├─> For each loaded command:
          │   ├─> Wrap in CustomCommandWrapper
          │   ├─> Check for name conflicts
          │   │   ├─> If conflicts with built-in: log warning, skip
          │   │   └─> Else: registry.register(wrapper)
          └─> Return registration count
```

### 2. Command Execution

```
User types: /analyze some arguments

TUI input handler
  └─> CommandRegistry::execute("/analyze some arguments", context)
      ├─> Parse command name and args
      ├─> Lookup "analyze" in registry
      ├─> Find CustomCommandWrapper
      └─> wrapper.execute(args, context)
          ├─> Extract command body
          ├─> Substitute $ARGUMENTS placeholder with args
          ├─> Add to conversation as user message
          ├─> Trigger agent response
          └─> Return CommandResult::Success
```

### 3. Listing Commands

```
User types: /help

HelpCommand::execute()
  ├─> List built-in commands
  └─> List custom commands
      ├─> Iterate registry.get_custom_commands()
      ├─> For each custom command:
      │   ├─> Display name (e.g., /analyze)
      │   ├─> Display description
      │   └─> Optionally show source file
      └─> Format and return output
```

## File Structure

### Command File Format

```markdown
---
description: Analyze codebase for technical debt
handoffs:
  - label: Generate Report
    agent: report_generator
    prompt: Create detailed technical debt report
    send: false
tags:
  - analysis
  - code-quality
---

## Analysis Command

Please analyze the codebase in the following directory:

**Arguments**: $ARGUMENTS

Focus on:
1. Code complexity metrics
2. Potential refactoring opportunities
3. Technical debt indicators

Provide a structured analysis with specific recommendations.
```

**Components**:
1. **Frontmatter** (lines 1-11): YAML between `---` delimiters
2. **Body** (lines 13+): Markdown content used as command prompt

### Directory Structure

```
.hoosh/
└── commands/
    ├── analyze.md          # Custom command: /analyze
    ├── review-pr.md        # Custom command: /review-pr
    ├── generate-docs.md    # Custom command: /generate-docs
    └── refactor-plan.md    # Custom command: /refactor-plan
```

## State Management

### Immutable State

- **CommandMetadata**: Immutable after parsing (owned data)
- **ParsedCommand**: Immutable after creation
- **CustomCommandWrapper**: Immutable wrapper around ParsedCommand

### Shared State

- **CustomCommandManager**: Owned by session, passed to registry during init
- **CommandRegistry**: Wrapped in `Arc<CommandRegistry>`, shared across async contexts

**Rationale**: Follows Hoosh's existing pattern (see `CommandContext::command_registry: Option<Arc<CommandRegistry>>`)

## Error Handling

### Error Types

```rust
use anyhow::{Context, Result, anyhow};

// File I/O errors
fs::read_to_string(path)
    .with_context(|| format!("Failed to read command file: {}", path.display()))?

// Parsing errors
parse_frontmatter(content)
    .context("Failed to parse YAML frontmatter")?

// Validation errors
anyhow::bail!("Command file '{}' has empty description", path.display())
```

### Error Propagation

- **parse_command_file()**: Returns `Result<ParsedCommand>` with detailed context
- **load_commands()**: Logs errors per file, continues loading others
- **register_commands()**: Returns `Result<usize>` with count of registered commands

**Pattern**: Match Hoosh's existing error handling using `anyhow::Context`

## Validation Rules

### File-Level Validation

1. **File Extension**: MUST be `.md`
2. **File Size**: MUST be ≤1MB (prevents performance issues)
3. **File Permissions**: MUST be readable
4. **Frontmatter Presence**: MUST have YAML frontmatter

### Metadata Validation

1. **Description**: MUST be non-empty after trimming
2. **Handoffs**: Each MUST have non-empty `label`, `agent`, `prompt`
3. **Tags**: No validation (any string array allowed)

### Name Validation

1. **Derived from filename**: Implicitly validated by filesystem
2. **Lowercase conversion**: Applied automatically
3. **Uniqueness**: Checked during registration (later wins)
4. **Conflict with built-ins**: Logged warning, custom command skipped

## Performance Characteristics

### Memory Usage

- **Per Command**: ~1-2KB (metadata + body string)
- **50 Commands**: ~50-100KB total
- **Caching**: Commands loaded once at startup, held in memory

### Loading Time

- **Per Command**: <1ms parse time (gray_matter + serde_yaml)
- **50 Commands**: <50ms total load time
- **Directory Scan**: <5ms for typical .hoosh/commands directory

### Execution Time

- **Command Lookup**: O(1) hash map lookup in registry
- **Body Substitution**: O(n) where n = body length (typically <5KB)
- **Total Overhead**: <1ms per custom command execution

## Future Extensibility

### Potential Enhancements

1. **Dynamic Reloading**: File watcher to reload commands without restart
2. **Command Parameters**: Structured parameters beyond $ARGUMENTS
3. **Command Aliases**: Support for alternative names
4. **Command Categories**: Organize by tags in help output
5. **Command Templates**: Placeholder syntax for more complex substitutions

### Design Accommodations

- Modular `CommandMetadata`: Easy to add new optional fields
- Trait-based `Command`: CustomCommandWrapper can evolve implementation
- Separate parser module: Can switch frontmatter library if needed

## References

- Hoosh Command Architecture: `src/commands/registry.rs`
- Existing Command Example: `src/commands/help_command.rs`
- Agent Definition Pattern: `src/agent_definition/mod.rs` (similar file loading pattern)
- Hoosh Constitution: `.specify/memory/constitution.md`
