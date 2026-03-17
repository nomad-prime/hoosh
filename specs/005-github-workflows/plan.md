# Implementation Plan: GitHub Workflow Triggers

**Branch**: `005-github-workflows` | **Date**: 2026-03-15 | **Spec**: `specs/005-github-workflows/spec.md`

---

## Summary

Add a GitHub webhook receiver to the daemon that detects @hoosh mentions in issue and PR events, sets up an isolated sandbox (clone + branch), and hands the event context to the agent. The agent then uses the pre-installed `gh` CLI for all GitHub operations. This eliminates the need to maintain a direct GitHub API integration (`PrProvider`) for webhook-triggered workflows.

---

## Technical Context

**Language/Version**: Rust 2024 edition (matches `Cargo.toml:4`)
**Primary Dependencies**:
- `axum 0.7` (existing — new webhook route added)
- `hmac 0.12` + `sha2 0.10` + `hex 0.4` (new — webhook signature verification)
- `serde_json` (existing — payload deserialization)
- `tokio 1.0` (existing — async runtime)
- All existing daemon dependencies (sandbox, executor, store, agent)

**Storage**: Extends existing `~/.hoosh/daemon/tasks/<task-id>.json` with `trigger` field
**Testing**: `cargo test` (unit + integration)
**Target Platform**: Linux/macOS server (wherever daemon runs)
**Project Type**: Single project (extends existing daemon module)
**Performance Goals**: Webhook response < 100ms (task queued async); signature check is synchronous
**Constraints**: Must not block webhook response on sandbox clone; async task dispatch
**Scale/Scope**: One daemon instance, low-frequency webhook events (not high throughput)

---

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Test-First Development | ✅ PASS | Unit tests for webhook parsing, signature verification, mention detection; integration tests for full dispatch flow |
| II. Trait-Based Design | ✅ PASS | Parsing functions (`parse_github_event`, `mentions_handle`) are pure with no external deps — directly unit-testable without a trait. `verify_signature` is likewise a pure function. No trait needed; testability is satisfied by function-level isolation |
| III. Single Responsibility | ✅ PASS | Webhook route → event parser → task dispatch are separate concerns |
| IV. Flat Module Structure | ✅ PASS | New files in `src/daemon/`: `webhook.rs`, `github_event.rs` — no deep nesting |
| V. Clean Code Practices | ✅ PASS | No obvious comments; idiomatic Rust error handling with `anyhow` |

**Gate Result**: ✅ All pass — no violations to justify.

---

## Project Structure

### Documentation (this feature)

```text
specs/005-github-workflows/
├── plan.md              # This file
├── spec.md              # Feature specification
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
└── tasks.md             # Phase 2 output (speckit.tasks)
```

### Source Code

```text
src/daemon/
├── mod.rs                    # Add github_event module export
├── config.rs                 # Add GithubConfig struct
├── task.rs                   # Add GithubTrigger struct + GithubEventType enum
├── executor.rs               # Skip post-agent git ops for webhook-triggered tasks
├── github_event.rs           # NEW: event types, parsing, mention detection
├── webhook.rs                # NEW: axum webhook route + signature verification
├── api/
│   ├── mod.rs                # Add /github/webhook route
│   └── routes.rs             # No change (webhook in separate file)
```

### Division of Responsibility

| Concern | Owner |
|---------|-------|
| Receive + verify webhook | Daemon (`webhook.rs`) |
| Detect @mention, deduplicate | Daemon (`github_event.rs`) |
| Clone repository | Daemon (`executor.rs`) |
| Branch strategy (create/checkout) | Agent (reads raw event JSON) |
| All git operations post-clone | Agent (bash + git + `gh` CLI) |
| PR creation, review replies | Agent (`gh` CLI) |

---

## Complexity Tracking

No constitution violations — no entries required.

