# Research: Daemon Mode

**Branch**: `004-daemon-mode` | **Date**: 2026-03-14

---

## HTTP Server Library

**Decision**: `axum 0.7`

**Rationale**: axum is built directly on hyper/tokio, which the project already uses. It provides a minimal, ergonomic router with extractor-based request parsing and JSON support via serde_json (already in deps). It is the most widely adopted Rust HTTP framework for async services as of 2026.

**Alternatives considered**:
- **warp**: Good tokio integration but filter-based composition is less ergonomic for straightforward REST; lower community momentum than axum.
- **actix-web**: Higher performance ceiling but pulls in a separate actor runtime and heavier dependency tree; overkill for a localhost-only daemon API.
- **tiny_http**: Synchronous/blocking — requires spawning OS threads for each connection, incompatible with the tokio executor already in use.

**New dependency**: `axum = "0.7"` + `tower = "0.4"` (lightweight middleware)

---

## Git Operations

**Decision**: `git2` crate (libgit2 bindings) + `auth-git2` helper, wrapped in `tokio::task::spawn_blocking`

**Rationale**: The daemon runs unattended with unbounded concurrent tasks. `git2` provides structured error types (distinguish auth failure from network failure from "repo not found"), no subprocess overhead, and precise programmatic control over branch creation and commit authorship. The `auth-git2` crate automatically queries the SSH agent and falls back to `~/.ssh/id_ed25519` / `~/.ssh/id_rsa`, covering the operator's configured SSH keys without manual `SSH_KEY_PATH` env-var wiring.

Since libgit2 is synchronous, all git operations are wrapped in `tokio::task::spawn_blocking` to avoid blocking the async executor — a pattern already used in the project for blocking I/O.

**Alternatives considered**:
- **`std::process::Command` calling system `git`**: Simpler, zero new deps, reuses operator's full git environment. Rejected because error handling is string-parsing of stderr, subprocess overhead accumulates with many concurrent tasks, and there's no structured way to detect "no changes to commit" vs. actual failure.
- **gitoxide (gix)**: Pure Rust, async-native, but push and SSH transport are not yet production-stable as of 2026.

**Pattern**:
```rust
// In tokio::task::spawn_blocking:
let mut auth = GitAuthenticator::new();
auth.set_ssh_key_from_paths("git", config.ssh_key_path.clone(), None);
let mut builder = RepoBuilder::new();
builder.remote_callbacks({ let mut cb = RemoteCallbacks::new(); cb.credentials(auth.credentials()); cb });
let repo = builder.clone(&repo_url, &sandbox_path)?;

// Create and checkout branch
let branch_name = format!("hoosh/{}", task_id);
let obj = repo.revparse_single(&format!("origin/{}", base_branch))?;
repo.branch(&branch_name, &obj.peel_to_commit()?, false)?;
repo.set_head(&format!("refs/heads/{}", branch_name))?;
repo.checkout_head(None)?;
```

**New dependencies**: `git2 = "0.19"`, `auth-git2 = "0.5"`

---

## GitHub PR Creation

**Decision**: `reqwest 0.12` (already in Cargo.toml) behind a `PrProvider` trait

**Rationale**: reqwest is already a dependency. The GitHub REST API `POST /repos/{owner}/{repo}/pulls` endpoint is simple and well-documented. Abstracting behind a `PrProvider` trait satisfies FR-022 (platform extensibility).

**API pattern**:
```
POST https://api.github.com/repos/{owner}/{repo}/pulls
Authorization: Bearer <PAT>
Content-Type: application/json

{
  "title": "...",
  "body": "...",
  "head": "hoosh/<task-id>",
  "base": "<base_branch>"
}
```

**Trait design**:
```rust
#[async_trait::async_trait]
pub trait PrProvider: Send + Sync {
    async fn create_pull_request(&self, params: CreatePrParams) -> Result<PrResult>;
    fn provider_name(&self) -> &'static str;
}
```

**No new dependencies needed** (reqwest already present).

---

## Task Persistence

**Decision**: One JSON file per task in `~/.hoosh/daemon/tasks/<task-id>.json` via serde_json

**Rationale**: The task store needs only three operations: write (on create/update), read-all (on startup for crash recovery), read-one (on status query). No relational queries, no joins, no aggregations. JSON files per task are crash-safe (atomic file write with rename), use no new dependencies, and support the startup crash-recovery scan trivially (iterate directory, find status=running, update to failed).

**Alternatives considered**:
- **SQLite via sqlx**: Async-native, good for queries, but adds ~2MB to binary and requires async migration management. Query complexity doesn't justify it.
- **SQLite via rusqlite**: Simpler than sqlx but synchronous — must be wrapped in `tokio::task::spawn_blocking`. More complexity than JSON files for this use case.
- **sled**: Embedded key-value store; good performance but overkill and adds another dependency for simple file-based state.

**Crash recovery pattern**:
```rust
// On daemon startup
for task in store.load_all().await? {
    if task.status == TaskStatus::Running {
        store.update_status(task.id, TaskStatus::Failed, Some("[incomplete] daemon restarted")).await?;
    }
}
```

**No new dependencies needed** (serde_json already present). Use `uuid` crate for task IDs.

**New dependency**: `uuid = { version = "1.0", features = ["v4"] }`

---

## Process Management (Daemon Lifecycle)

**Decision**: PID file at `~/.hoosh/daemon.pid`, check liveness via `kill -0 <pid>`

**Rationale**: Standard Unix daemon pattern. Simple, no extra dependencies. `hoosh daemon status` reads the PID file and probes liveness. `hoosh daemon stop` sends SIGTERM; `--force` sends SIGKILL.

**Alternatively**: Could communicate via the HTTP API (GET /health) for status checks — more robust since HTTP will respond even if PID file is stale. Both mechanisms are used: PID file for process management, /health for connectivity verification.

---

## Token Budget Enforcement

**Decision**: Wrap agent execution with a cumulative token counter; halt via cancellation token

**Rationale**: The existing `TokenAccountant` in `src/context_management/token_accountant.rs` already tracks cumulative token usage. The daemon wraps agent execution with a `tokio::sync::watch` budget channel: after each LLM response, the executor checks total tokens consumed against the budget and drops the cancellation token if exceeded.

**Integration point**: Agent already emits `AgentEvent::TokenUsage` events (or similar) that the executor subscribes to. If total exceeds budget, the executor calls cancel and then performs the incomplete commit.

---

## Summary of New Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `axum` | `0.7` | HTTP server for daemon REST API |
| `tower` | `0.4` | Middleware for axum |
| `uuid` | `1.0` (features: v4) | Task ID generation |
| `git2` | `0.19` | libgit2 bindings for clone/commit/push |
| `auth-git2` | `0.5` | SSH key + agent authentication for git2 |

All other needs (JSON, HTTP client, async) are covered by existing dependencies.
