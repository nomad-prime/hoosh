<!--
Sync Impact Report:
- Version change: Initial version → 1.0.0
- Principles established: 5 core principles
- Sections added: Development Workflow, Governance
- Templates requiring updates:
  ✅ plan-template.md - Constitution Check section already references this file
  ✅ spec-template.md - Requirements sections align with principles
  ✅ tasks-template.md - Task structure supports testing and modularity principles
- Follow-up TODOs:
  - Ratification date set to project creation (estimated from git history)
  - No deferred placeholders
-->

# Hoosh Constitution

## Core Principles

### I. Modularity First

All code MUST follow modular organization principles:
- Use `mod.rs` files for module declarations
- Group related functionality in logical modules (e.g., `backends/`, `cli/`, `config/`, `tui/`)
- Re-export public APIs through `lib.rs`
- Keep `main.rs` minimal - CLI entry point only
- Each module MUST have a clear, single responsibility

**Rationale**: Modular architecture enables independent development, testing, and maintenance. It reduces coupling and improves code discoverability.

### II. Explicit Error Handling

Error handling MUST be explicit and informative:
- Use `anyhow::Result<T>` for fallible operations
- Provide context with `.context()` for all error propagation
- Custom error types via `thiserror` for domain-specific errors
- NO silent failures or unwraps in production code
- Errors MUST include actionable information for debugging

**Rationale**: Robust error handling ensures reliability and improves developer experience through clear failure diagnostics.

### III. Async-First Architecture

All I/O operations MUST be asynchronous:
- Use `tokio` runtime for async execution
- Trait methods requiring I/O MUST be async
- Shared state across async contexts MUST use `Arc<T>`
- Handle cancellation appropriately with `tokio::select!` when needed
- Backend integrations MUST support async/await patterns

**Rationale**: Asynchronous architecture ensures responsiveness, especially critical for AI backend interactions and TUI responsiveness.

### IV. Testing Discipline

Testing MUST focus on behavior, not implementation:
- Test names MUST describe behavior being verified (e.g., `agent_handles_simple_response`)
- Tests MUST cover: happy paths, tool execution, error handling, state/events
- Use realistic mocks that simulate actual dependencies
- Organize complex test suites in separate files (e.g., `core_expanded_tests.rs`)
- Integration tests MUST verify component interactions
- Unit tests MUST run fast (millisecond sleeps only)

**Rationale**: Behavioral testing creates resilient test suites that survive refactoring and clearly document expected system behavior.

### V. Simplicity and Clarity

Code MUST prioritize clarity over cleverness:
- NO obvious comments (code should be self-documenting)
- Descriptive naming: PascalCase for types, snake_case for functions/files, SCREAMING_SNAKE_CASE for constants
- Avoid premature abstraction - prefer explicit code over DRY when clarity benefits
- Dependencies MUST be minimal and well-maintained
- Use builder patterns for complex object construction in tests

**Rationale**: Simple, clear code reduces cognitive load, eases maintenance, and accelerates onboarding.

## Development Workflow

### Code Organization Standards

- **Structs/Enums**: PascalCase (e.g., `LlmBackend`, `ChatMessage`)
- **Traits**: PascalCase with descriptive behavior (e.g., `MessageSender`, `ConfigProvider`)
- **Functions**: snake_case with descriptive verbs (e.g., `create_client`, `parse_response`)
- **Files**: snake_case (e.g., `together_ai.rs`, `chat_handler.rs`)
- **Constants**: SCREAMING_SNAKE_CASE (e.g., `DEFAULT_MODEL`, `API_VERSION`)

### Refactoring Protocol

When refactoring existing modules:
1. Create new module alongside existing (e.g., `client_v2.rs` next to `client.rs`)
2. Update imports gradually with tests passing at each step
3. Remove old implementation ONLY after complete migration
4. Use feature flags for gradual rollout if needed

### Dependency Management

- Pin major versions in `Cargo.toml`
- Run `cargo audit` to check security vulnerabilities
- Justify all new dependencies - prefer standard library or existing deps
- Remove unused dependencies promptly

### Performance Requirements

- Use `tokio::spawn` for CPU-intensive tasks to avoid blocking
- Profile performance-critical paths with `cargo bench`
- Use `tokio-console` for async debugging in development
- Memory management: `Arc<T>` for multi-threaded, `Box<dyn Trait>` for trait objects

## Governance

### Constitution Authority

This constitution supersedes all other practices and documentation. When conflicts arise between this constitution and other project documentation (README, AGENTS.md, ARCHITECTURE.md), the constitution takes precedence.

### Amendment Process

1. Proposed changes MUST be documented with rationale
2. Version MUST be incremented following semantic versioning:
   - **MAJOR**: Backward incompatible governance changes or principle removals/redefinitions
   - **MINOR**: New principles added or materially expanded guidance
   - **PATCH**: Clarifications, wording fixes, non-semantic refinements
3. All dependent templates (plan-template.md, spec-template.md, tasks-template.md) MUST be updated to reflect amendments
4. Amendment commits MUST include Sync Impact Report

### Compliance Review

- All pull requests MUST verify compliance with core principles
- Complexity that violates principles MUST be explicitly justified in plan.md
- Use AGENTS.md for runtime development guidance (language-specific patterns, testing strategies)
- Use ARCHITECTURE.md for system design documentation (architectural decisions, component interactions)
- Do not comment Code

### Versioning Policy

- Constitution version tracked in this file
- Ratification date MUST NOT change after initial adoption
- Last Amended date MUST update on every content change (excluding typos)
- ISO 8601 date format: YYYY-MM-DD

**Version**: 1.0.0 | **Ratified**: 2024-09-29 | **Last Amended**: 2025-12-09
