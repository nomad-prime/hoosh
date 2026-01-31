<!--
Sync Impact Report:
Version: 0.0.0 → 1.0.0
Rationale: Initial constitution creation (MAJOR bump from placeholder)

Modified Principles:
- NEW: I. Test-First Development
- NEW: II. Trait-Based Design & Dependency Injection
- NEW: III. Single Responsibility Principle
- NEW: IV. Flat Module Structure
- NEW: V. Clean Code Practices

Added Sections:
- Core Principles (5 principles)
- Code Organization Standards
- Testing Requirements
- Governance

Templates Requiring Updates:
- ✅ plan-template.md (Constitution Check section references this file)
- ⚠️ spec-template.md (may need principle references - review recommended)
- ⚠️ tasks-template.md (may need task categorization aligned with principles - review recommended)

Follow-up TODOs:
- RATIFICATION_DATE: Set to today (2026-01-23) as initial version
- Review spec-template.md and tasks-template.md for alignment with new principles
-->

# Hoosh Development Constitution

## Core Principles

### I. Test-First Development (NON-NEGOTIABLE)

Testing is mandatory and must be comprehensive:

- **Unit tests MUST be written** for all business logic, independent components, and isolated functionality
- **Integration tests MUST be written** for component interactions, backend communication, and multi-module workflows
- **Test organization**: Use separate test files (`module_expanded_tests.rs`) for complex modules to maintain clarity
- **Test naming**: Use descriptive names that describe behavior being verified, not implementation details (e.g., `agent_handles_simple_response` not `test_internal_state`)
- **Mock objects**: Create realistic mocks that simulate actual dependencies using traits
- **Test speed**: Unit tests must use millisecond-scale sleeps to keep test suites fast
- **Test coverage focus**: Happy path, tool execution, error handling, state & events

**Rationale**: Tests ensure correctness, enable refactoring with confidence, and serve as executable documentation. Fast, comprehensive tests are essential for maintaining code quality as the project scales.

### II. Trait-Based Design & Dependency Injection

Code MUST be designed for testability and modularity through traits:

- **All backend integrations MUST use traits** (e.g., `LlmBackend`, `MessageSender`, `ConfigProvider`)
- **Traits MUST be async-compatible** using `#[async_trait::async_trait]` for async methods
- **Traits MUST include `Send + Sync` bounds** for multi-threaded contexts
- **Factory pattern MUST be used** for runtime polymorphism (e.g., `create_backend()` returns `Box<dyn Trait>`)
- **Dependencies MUST be injected** via constructors, not hardcoded or globally accessed
- **Builder pattern SHOULD be used** for complex test setup to improve readability

**Rationale**: Traits enable testing with mocks, allow swapping implementations (e.g., different AI backends), and enforce clear contracts between components. Dependency injection makes code modular and testable.

### III. Single Responsibility Principle

Each module, struct, and function MUST have one clear responsibility:

- **One concern per module**: Each file should handle a single domain concept (e.g., `config.rs` for configuration, `chat_handler.rs` for chat logic)
- **Small, focused functions**: Functions should do one thing well with descriptive verb names (e.g., `create_client`, `parse_response`)
- **No god objects**: Avoid large structs that handle multiple concerns
- **Separation of concerns**: Business logic separate from I/O, parsing separate from validation

**Rationale**: Single responsibility improves code readability, testability, and maintainability. Smaller, focused components are easier to reason about and change independently.

### IV. Flat Module Structure

Module hierarchy MUST be kept as flat as possible:

- **Avoid nested modules**: Do NOT create deep module hierarchies (modules within modules)
- **Use `mod.rs` for module declarations**: Group related functionality at the same level
- **Top-level organization**: Use clear top-level modules (e.g., `backends/`, `cli/`, `config/`, `tui/`)
- **Re-export through `lib.rs`**: Expose public APIs through `lib.rs` for clean external access
- **Keep `main.rs` minimal**: Entry point should only handle CLI initialization

**Rationale**: Flat structures reduce cognitive overhead, make imports simpler, and prevent over-engineering. Deep hierarchies often signal premature abstraction.

### V. Clean Code Practices

Code MUST be readable, maintainable, and idiomatic:

- **NO obvious comments**: Do NOT add comments stating the obvious; code should be self-documenting
- **Descriptive naming**: Use clear, intention-revealing names
  - Structs/Traits: PascalCase (e.g., `LlmBackend`, `ChatMessage`)
  - Functions: snake_case with descriptive verbs (e.g., `create_client`, `parse_response`)
  - Files: snake_case (e.g., `together_ai.rs`, `chat_handler.rs`)
  - Constants: SCREAMING_SNAKE_CASE (e.g., `DEFAULT_MODEL`, `API_VERSION`)
- **Error handling**: Use `anyhow::Result` for fallible operations, `thiserror` for domain-specific errors
- **Idiomatic Rust**: Follow Rust 2024 edition conventions
  - Use `Arc<T>` for shared state in multi-threaded contexts
  - Use `Box<dyn Trait>` for trait objects
  - Use `String` for owned strings, `&str` for borrowed
  - Prefer `async fn` over `-> impl Future`
- **Refactoring strategy**: Create new modules alongside existing ones (e.g., `client_v2.rs`), update gradually, remove old implementation only after complete migration

**Rationale**: Clean code reduces bugs, speeds up development, and makes onboarding easier. Consistent conventions reduce cognitive load.

## Code Organization Standards

### Module Structure

- **Use `mod.rs` files** for module declarations
- **Group related functionality** in modules (e.g., `backends/`, `cli/`, `config/`)
- **Re-export public APIs** through `lib.rs`
- **Keep `main.rs` minimal** - just CLI entry point

### Configuration Pattern

All configuration MUST follow the standard pattern:

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct AppConfig {
    pub default_backend: String,
    pub backends: HashMap<String, BackendConfig>,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        // Load from TOML file with default fallback
    }
}
```

### CLI Structure

Use `clap` with derive API for all CLI interfaces:

```rust
#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    // Subcommands here
}
```

### Async Patterns

- **Use `tokio::main`** for async main function
- **Prefer `async fn`** over `-> impl Future`
- **Use `Arc`** for shared state across async contexts
- **Handle cancellation** with `tokio::select!` when appropriate
- **Spawn CPU-intensive tasks** with `tokio::spawn` to avoid blocking
- **Use `tokio-console`** for async debugging in development

## Testing Requirements

### Test Organization

- **Unit tests**: Test individual components in isolation using `#[tokio::test]`
- **Integration tests**: Test component interactions with realistic mocks
- **Separate test files**: Use `#[path = "module_expanded_tests.rs"]` for complex modules
- **Builder pattern in tests**: Use helper functions like `create_test_agent()` for setup

### Test Coverage Guidelines

Tests MUST cover:

1. **Happy path**: Simple response handling, multiple conversation turns, builder pattern configuration
2. **Tool execution**: Agent handles responses with tool calls, tool calls are executed, results added to conversation
3. **Error handling**: Missing response content, backend errors propagate correctly, invalid tool call responses
4. **State & events**: Event emission during operation, token usage tracking, context manager integration, multiple agents operate independently

### Mock Objects

Mocks MUST:

- Implement the same traits as real dependencies
- Simulate realistic behavior (not just return static data)
- Use `Arc<AtomicUsize>` for call counting when needed
- Be reusable across multiple test cases

## Governance

### Constitutional Authority

- This constitution supersedes all other development practices
- All code reviews MUST verify compliance with these principles
- Complexity that violates principles MUST be justified in the implementation plan
- Use `CLAUDE.md` for detailed runtime development guidance (this constitution defines non-negotiable principles; `CLAUDE.md` provides practical patterns)

### Amendment Process

- Constitutional amendments require:
  1. Documentation of the proposed change with rationale
  2. Update of this constitution file with version bump
  3. Propagation of changes to all dependent templates
  4. Approval before implementation begins
- Version increments follow semantic versioning:
  - **MAJOR**: Backward incompatible governance/principle removals or redefinitions
  - **MINOR**: New principle/section added or materially expanded guidance
  - **PATCH**: Clarifications, wording, typo fixes, non-semantic refinements

### Compliance Review

- All PRs MUST include test coverage for new functionality
- All PRs MUST verify no principle violations
- Integration tests MUST pass before merge
- `cargo clippy` warnings MUST be addressed
- `cargo fmt` MUST be run before commit

### Quality Gates

Before any feature is merged:

1. ✅ All tests pass (`cargo test`)
2. ✅ No clippy warnings (`cargo clippy`)
3. ✅ Code is formatted (`cargo fmt`)
4. ✅ Test coverage meets guidelines (happy path + error handling + integration)
5. ✅ No principle violations or violations are explicitly justified

**Version**: 1.0.0 | **Ratified**: 2026-01-23 | **Last Amended**: 2026-01-23
