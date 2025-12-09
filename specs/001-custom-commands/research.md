# Research: Custom Commands Feature

**Feature**: Custom Commands
**Branch**: 001-custom-commands
**Date**: 2025-12-09

## Decisions

### 1. YAML Frontmatter Parsing Library

**Decision**: Use `gray_matter` crate for parsing markdown frontmatter

**Rationale**:
- Mature Rust port of widely-used JavaScript gray-matter library
- Multi-format support (YAML, JSON, TOML) for future extensibility
- Excellent integration with serde for type-safe deserialization
- Clean separation of frontmatter and markdown body
- Active maintenance as of 2025
- Matches patterns used in Claude Code

**Alternatives Considered**:
- `fronma`: Simpler but less feature-rich; lacks multi-format support
- `pulldown-cmark-frontmatter`: Good integration with existing pulldown-cmark usage, but less mature
- Manual parsing: Too error-prone and doesn't handle edge cases well

### 2. Command Directory Location

**Decision**: Use `.hoosh/commands` in current working directory (project-local)

**Rationale**:
- Aligns with user's requirement from spec
- Project-specific commands make sense for workflow automation
- Matches pattern of `.claude/commands/` that user referenced
- Allows different projects to have different custom commands
- No conflicts with global configuration in `~/.config/hoosh/`

**Alternatives Considered**:
- Global `~/.hoosh/commands`: Would apply to all projects, less flexible
- Mixed approach: Too complex for MVP

### 3. Command Loading Strategy

**Decision**: Load commands once at startup (not dynamic reloading)

**Rationale**:
- Simpler implementation for MVP
- Matches assumption documented in spec.md
- Avoids file watching complexity and potential race conditions
- Performance: no runtime overhead of file monitoring
- Users can restart Hoosh to reload (acceptable for MVP)

**Alternatives Considered**:
- File watching with hot reload: Added complexity, out of scope for MVP
- Reload command (e.g., `/reload-commands`): Could be future enhancement

### 4. Error Handling for Malformed Commands

**Decision**: Log warnings for invalid command files but continue loading valid ones

**Rationale**:
- Resilient: one bad file doesn't break all custom commands
- User-friendly: clear error messages indicate which file has issues
- Follows Hoosh's existing error handling patterns with `anyhow::Context`
- Aligns with FR-009: provide clear error messages

**Pattern**:
```rust
match parse_command_file(&path) {
    Ok(command) => commands.push(command),
    Err(e) => {
        eprintln!("Warning: Failed to load command from {}: {}",
                  path.display(), e);
    }
}
```

### 5. Command Name Derivation

**Decision**: Derive command name from filename (without .md extension)

**Rationale**:
- Simple and intuitive: `analyze.md` becomes `/analyze`
- Matches Claude Code convention
- No duplication: filename already unique in directory
- Avoids need for name field in frontmatter

**Validation**: Command names must be valid filesystem names (enforced by OS)

### 6. Precedence for Name Conflicts

**Decision**: Built-in commands take precedence over custom commands

**Rationale**:
- Prevents users from accidentally overriding core functionality
- Safer: users can't break essential commands like `/exit`, `/help`
- Documented in spec as FR-010
- Warn user if custom command conflicts with built-in

**Implementation**: Register built-in commands first, skip custom command if name exists

## Best Practices Research

### YAML Frontmatter Format

Following Claude Code conventions from `.claude/commands/`:

```markdown
---
description: Brief description of what the command does
handoffs:
  - label: Display label for handoff button
    agent: target_agent_name
    prompt: Prompt to send when handing off
    send: boolean (optional, default: false)
tags:
  - category1
  - category2
---

## Actual markdown content here

This becomes the command body/prompt.
```

### Optional Field Handling

Using Serde attributes following Hoosh's existing config patterns:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMetadata {
    pub description: String,              // Required

    #[serde(default)]
    pub handoffs: Vec<Handoff>,           // Optional, defaults to empty vec

    #[serde(default)]
    pub tags: Vec<String>,                // Optional, defaults to empty vec
}
```

### Error Context Pattern

Following Hoosh's existing error handling in `agent_definition/mod.rs`:

```rust
fs::read_to_string(file_path)
    .with_context(|| {
        format!("Failed to read command file: {}", file_path.display())
    })?
```

## Dependencies Required

### New Dependencies

Add to `Cargo.toml`:

```toml
[dependencies]
gray_matter = "0.2"
```

### Existing Dependencies (Already Available)

- `serde`: For deserialization
- `serde_yaml`: For YAML parsing (transitively through gray_matter)
- `anyhow`: For error handling
- `walkdir`: For directory traversal (already in use)

## Integration Points

### 1. Command Registry

Custom commands will integrate with existing `CommandRegistry` at `/Users/arminghajarjazi/Projects/nomad-prime/hoosh/src/commands/registry.rs`:

- Implement `Command` trait for custom command wrapper
- Register after built-in commands
- Use existing `CommandContext` for execution

### 2. Startup Sequence

Modify session initialization to:
1. Check for `.hoosh/commands` directory (create if missing - FR-002)
2. Scan for `.md` files
3. Parse each file with frontmatter parser
4. Register valid commands with `CommandRegistry`
5. Log errors for invalid files

### 3. Help/List Commands

Extend existing `/help` command to display custom commands:
- Separate section for "Custom Commands"
- Show description from frontmatter
- Indicate source file location

## Performance Considerations

### Command Loading

- **Timing**: One-time cost at startup
- **Expected Scale**: <100 custom command files typical
- **File Size**: Limit to 1MB per file (reasonable for markdown)
- **Impact**: Negligible startup delay (<50ms for 10 commands)

### Memory Usage

- **Per Command**: ~1-2KB (metadata + body)
- **Total**: <100KB for typical usage (50 commands)
- **Optimization**: Commands loaded once, cached in memory

## Security Considerations

### File System Access

- Read-only access to `.hoosh/commands` directory
- No execution of arbitrary code (commands are markdown prompts)
- Follow existing permission patterns from Hoosh

### Input Validation

- Validate YAML structure via serde
- Limit file size to prevent DoS (1MB max)
- Sanitize command names (enforced by filesystem)
- No special characters in derived command names

## Testing Strategy

### Unit Tests

1. **Parser Tests**:
   - Valid frontmatter parsing
   - Missing frontmatter handling
   - Malformed YAML detection
   - Optional field defaults
   - Edge cases (empty body, large files, etc.)

2. **Command Loading Tests**:
   - Directory creation
   - Multi-file loading
   - Error resilience (partial failures)
   - Name conflict detection

### Integration Tests

1. **End-to-End Flow**:
   - Create command file
   - Start Hoosh
   - Verify command available
   - Execute custom command
   - Verify behavior

2. **Error Scenarios**:
   - Invalid YAML syntax
   - Missing directory
   - Permission issues
   - Name conflicts

## References

- Gray Matter Crate: https://crates.io/crates/gray_matter
- Serde Documentation: https://serde.rs/
- Hoosh Architecture: `/Users/arminghajarjazi/Projects/nomad-prime/hoosh/ARCHITECTURE.md`
- Hoosh Agent Patterns: `/Users/arminghajarjazi/Projects/nomad-prime/hoosh/AGENTS.md`
- Claude Code Commands Reference: `.claude/commands/` directory
