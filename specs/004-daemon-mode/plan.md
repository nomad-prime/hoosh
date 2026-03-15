# Implementation Plan: Daemon Mode

**Branch**: `004-daemon-mode` | **Date**: 2026-03-14 | **Spec**: `specs/004-daemon-mode/spec.md`
**Input**: Feature specification from `/specs/004-daemon-mode/spec.md`

## Summary

Add a long-running background daemon to hoosh that accepts task submissions via a REST API, autonomously clones repositories, runs the hoosh agent, commits changes to a dedicated branch, and opens a GitHub pull request. Tasks run in parallel with no concurrency cap, each in an isolated sandbox directory. The daemon exposes full task lifecycle management (submit, list, status, cancel, logs) and enforces a two-level permission system (global + per-repo overrides).

## Technical Context

**Language/Version**: Rust 2024 edition (matches `Cargo.toml:4`)
**Primary Dependencies**: tokio 1.0 (async runtime), axum 0.7 (HTTP server, new), reqwest 0.12 (GitHub PR API, existing), serde_json (task persistence, existing), uuid 1.0 (task IDs, new), git2 0.19 + auth-git2 0.5 (git operations, new), clap 4.0 (CLI, existing)
**Storage**: JSON files per task in `~/.hoosh/daemon/tasks/<task-id>.json`; execution logs as plain text files in sandboxes
**Testing**: `cargo test` with unit tests per module and integration tests using mock backends and mock `PrProvider`
**Target Platform**: macOS/Linux (wherever hoosh runs)
**Project Type**: Single Rust binary (adds `daemon/` module + CLI subcommand to existing binary)
**Performance Goals**: No concurrency cap; each task runs in its own `tokio::spawn`'d future. HTTP API must respond within 50ms for all non-blocking operations
**Constraints**: Localhost-only binding by default (FR-017); no auth on HTTP API in v1; GitHub only for PRs in v1
**Scale/Scope**: Designed for developer/team use (tens of concurrent tasks); not a high-throughput system

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### I. Test-First Development ✅
- Unit tests required for: `TaskStore` (CRUD + crash recovery scan), `PermissionResolver` (merge logic, deny-wins), `Sandbox` (lifecycle, git command construction), `TaskExecutor` (token budget enforcement, cancellation), API route handlers (request validation, response shapes)
- Integration tests required for: full task flow with mock agent + mock `PrProvider`; cancel mid-execution; token exhaustion halt; crash recovery on restart
- **No violations.**

### II. Trait-Based Design & Dependency Injection ✅
- `PrProvider` trait with `GitHubPrProvider` impl — satisfies FR-022 extensibility
- `TaskStore` behavior testable by injecting a store with known state
- Agent already uses trait (`LlmBackend`) — daemon reuses existing injection pattern
- **No violations.**

### III. Single Responsibility ✅
- `store.rs`: task persistence only
- `sandbox.rs`: git operations + directory lifecycle only
- `executor.rs`: orchestration (clone → agent → commit → PR) only
- `permissions.rs`: two-level resolution logic only
- `api/routes.rs`: HTTP handler functions only
- **No violations.**

### IV. Flat Module Structure ✅
- New top-level `src/daemon/` module; flat within it with one nested `pr_provider/` and `api/` for grouping (justified by distinct concerns)
- **No violations.**

### V. Clean Code Practices ✅
- Follow existing naming conventions (snake_case files, PascalCase structs)
- No obvious comments; error handling via `anyhow::Result`
- **No violations.**

**Gate result**: PASS. No violations to justify.

## Project Structure

### Documentation (this feature)

```text
specs/004-daemon-mode/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   └── api.md           # REST API contract
└── tasks.md             # Phase 2 output (/speckit.tasks — not yet created)
```

### Source Code (repository root)

```text
src/
├── daemon/
│   ├── mod.rs               # Public re-exports: DaemonServer, DaemonConfig, TaskStore
│   ├── config.rs            # DaemonConfig; extends AppConfig [daemon] section
│   ├── task.rs              # Task struct, TaskStatus enum, serde impls
│   ├── store.rs             # TaskStore — JSON file CRUD + crash-recovery scan
│   ├── sandbox.rs           # Sandbox — create/destroy temp dir, git clone/commit/push
│   ├── executor.rs          # TaskExecutor — clone→run→commit→pr orchestration
│   ├── permissions.rs       # Two-level permission resolution (global + repo overlay)
│   ├── pr_provider/
│   │   ├── mod.rs           # PrProvider trait, CreatePrParams, PrResult
│   │   └── github.rs        # GitHubPrProvider (reqwest + GitHub REST API)
│   └── api/
│       ├── mod.rs           # Router construction (axum)
│       ├── routes.rs        # Handler functions for all endpoints
│       └── types.rs         # HTTP request/response types (serde)
├── cli/
│   ├── mod.rs               # Add Daemon variant to Commands enum
│   └── daemon.rs            # handle_daemon() — start/stop/status/submit
└── [existing modules unchanged]

tests/
├── integration/
│   └── daemon_task_flow.rs  # End-to-end task flow with mock agent
└── [existing tests unchanged]
```

**Structure Decision**: Single project (Option 1). The daemon is a new subsystem within the existing binary — `hoosh daemon start` launches it in-process. No separate binary needed.

## Complexity Tracking

> No Constitution Check violations to justify.

## Design Decisions

### Daemon Process Model

The daemon runs as a foreground or background OS process. `hoosh daemon start` forks (or uses `nohup`) to background the process and writes its PID to `~/.hoosh/daemon.pid`. `hoosh daemon stop` reads the PID file, sends SIGTERM (graceful) or SIGKILL (--force), then waits for exit.

The daemon binary is the same `hoosh` binary — no separate process needed. `hoosh daemon start --foreground` is the in-process variant for debugging and system service managers (systemd, launchd).

### Task Execution Flow

```
POST /tasks
  → TaskStore::create(task) [status=Queued]
  → tokio::spawn(TaskExecutor::run(task_id))
  → return {task_id}

TaskExecutor::run:
  1. TaskStore::update_status(Running)
  2. Sandbox::create()
  3. git clone --branch <base> <repo_url> <sandbox/repo/>
  4. PermissionResolver::resolve(global_perms, <sandbox/repo/.hoosh/permissions.json>)
  5. Agent::run(instructions, permission_set, token_budget, cancellation_token)
     ↑ monitors token events; cancels agent if budget exceeded
  6. if changes_detected:
       git add -A && git commit -m "hoosh: <instructions[:72]>"
       git push origin hoosh/<task-id>
       PrProvider::create_pull_request(...)
       TaskStore::update(status=Completed, pr_url=...)
     else:
       TaskStore::update(status=Completed)
  7. Sandbox::cleanup() [unless retain_sandboxes=true]

On any error (clone fail, permission halt, token exhaustion, disk full):
  → if repo has staged changes: git commit -m "[incomplete] <reason>"
  → TaskStore::update(status=Failed, error_message=...)
  → Sandbox::cleanup()
```

### Token Budget Enforcement

`TaskExecutor` subscribes to `AgentEvent::TokenUsage` on the event channel. After each event, it checks cumulative total against the task's effective budget. On breach:
1. Send cancellation signal via `tokio_util::sync::CancellationToken`
2. Agent loop detects cancellation between tool calls and returns early
3. Executor performs incomplete commit + updates task status to Failed

### Graceful Shutdown

`DaemonServer` holds a `Arc<Mutex<bool>> shutting_down` flag. On SIGTERM:
1. Set `shutting_down = true` — HTTP layer returns 503 for new POST /tasks
2. Await all running `tokio::JoinHandle`s to complete
3. Exit process

On SIGKILL / `--force` flag: cancel all running task `CancellationToken`s, then exit.

### Two-Level Permission Resolution

```rust
fn resolve(global: PermissionsFile, repo_level: Option<PermissionsFile>) -> PermissionsFile {
    let Some(repo) = repo_level else { return global; };
    let combined_deny = union(global.deny, repo.deny);
    // Drop repo allows that conflict with any global deny rule
    let filtered_repo_allow = repo.allow.into_iter()
        .filter(|rule| !global.deny.iter().any(|d| d.conflicts_with(rule)))
        .collect();
    let combined_allow = union(global.allow, filtered_repo_allow);
    PermissionsFile { allow: combined_allow, deny: combined_deny, ..global }
}
```

### PID File and Status

`~/.hoosh/daemon.pid` contains the PID as a plain integer. `hoosh daemon status`:
1. Read PID file
2. `kill -0 <pid>` to verify process is alive
3. GET `/health` to verify HTTP is responding
4. Print status + uptime + active task count

### Incomplete Commit Marker

When any task halts before completion (permission denial, token exhaustion, cancellation, disk full, crash recovery), the commit message is prefixed with `[incomplete]` and the PR body contains:

```
⚠️ This PR was created from an incomplete run.

Reason: <error_message>
Tokens consumed: <N> / <budget>
```

---

## Phase 0 Complete

All research decisions resolved. See `research.md` for full rationale.

## Phase 1 Complete

Design artifacts:
- `data-model.md` — entity definitions, field specs, module layout
- `contracts/api.md` — full REST API + CLI contract
- `quickstart.md` — operator setup and usage guide
