# Data Model: Daemon Mode

**Branch**: `004-daemon-mode` | **Date**: 2026-03-14

---

## Entities

### Task

The central entity. Represents one unit of autonomous work submitted to the daemon.

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `id` | `String` | yes | Format: `hoosh-<uuid-v4>` |
| `repo_url` | `String` | yes | Git remote URL (SSH or HTTPS) |
| `base_branch` | `String` | yes | Branch to open PR against |
| `instructions` | `String` | yes | Freeform prompt for the agent |
| `pr_title` | `Option<String>` | no | Overrides auto-generated PR title |
| `pr_labels` | `Vec<String>` | no | Labels to apply to the PR |
| `token_budget` | `Option<usize>` | no | Per-task override; falls back to `DaemonConfig.default_token_budget` |
| `status` | `TaskStatus` | yes | See status lifecycle below |
| `created_at` | `DateTime<Utc>` | yes | Set at submission |
| `started_at` | `Option<DateTime<Utc>>` | no | Set when agent execution begins |
| `completed_at` | `Option<DateTime<Utc>>` | no | Set when reaching any terminal state |
| `pr_url` | `Option<String>` | no | Populated after PR is created |
| `branch` | `Option<String>` | no | `hoosh/<task-id>`, set before push |
| `tokens_consumed` | `usize` | yes | Running total; updated after each LLM call |
| `error_message` | `Option<String>` | no | Set on failure/cancellation |
| `sandbox_path` | `Option<String>` | no | Absolute path to sandbox directory |
| `log_path` | `Option<String>` | no | Absolute path to execution log file |

**Persistence**: `~/.hoosh/daemon/tasks/<task-id>.json`

**Validation rules**:
- `repo_url` must be non-empty
- `base_branch` must be non-empty
- `instructions` must be non-empty
- `token_budget`, if provided, must be > 0

---

### TaskStatus

```rust
pub enum TaskStatus {
    Queued,     // Accepted, not yet started
    Running,    // Agent is executing
    Completed,  // Agent finished, changes committed (PR created if changes existed)
    Failed,     // Halted due to error, permission denial, token exhaustion, or crash recovery
    Cancelled,  // Explicitly cancelled via DELETE /tasks/{id}
}
```

**State transitions**:

```
Queued → Running → Completed
                 → Failed
       → Cancelled  (from Queued before agent starts)
Running → Cancelled (via cancel request — triggers graceful abort)
        → Failed    (permission denial, token exhaustion, disk full, crash recovery)
```

**Terminal states**: `Completed`, `Failed`, `Cancelled` — no further transitions.

---

### Sandbox

Ephemeral per-task workspace. Not persisted independently — lifecycle is managed by `TaskExecutor`.

| Field | Type | Notes |
|-------|------|-------|
| `path` | `PathBuf` | Temp directory, e.g. `/tmp/hoosh-<task-id>/` |
| `log_file` | `PathBuf` | `<sandbox>/execution.log` |
| `repo_dir` | `PathBuf` | `<sandbox>/repo/` — cloned repository |

**Lifecycle**:
1. Created before agent starts (`tokio::fs::create_dir_all`)
2. Log file opened for writing (append mode)
3. Repo cloned into `repo_dir`
4. Agent runs with `repo_dir` as working directory
5. On completion/failure: push + PR creation (if applicable)
6. Sandbox deleted after task reaches terminal state (unless `retain_sandboxes = true`)

---

### PermissionSet

Resolved per-task from two sources. Not stored independently — computed at task start.

| Field | Type | Notes |
|-------|------|-------|
| `allow` | `Vec<PermissionRule>` | Additive union of global + repo-level allows |
| `deny` | `Vec<PermissionRule>` | Union of global + repo-level denies; global deny wins over repo allow |

**Resolution algorithm** (applied after repo clone, before agent start):
1. Load `~/.hoosh/permissions.json` → `global`
2. If `<sandbox>/repo/.hoosh/permissions.json` exists → load `repo_level`
3. `result.allow = global.allow + (repo_level.allow - global.deny_patterns)`
4. `result.deny = global.deny + repo_level.deny`

---

### DaemonConfig

Stored in `~/.hoosh/config.toml` under a `[daemon]` section (extends existing `AppConfig`).

| Field | Type | Default | Notes |
|-------|------|---------|-------|
| `bind_address` | `SocketAddr` | `127.0.0.1:7979` | FR-017 localhost binding |
| `default_token_budget` | `usize` | `100_000` | FR-019 global token cap |
| `github_pat` | `Option<String>` | `None` | FR-022 PAT for PR API |
| `ssh_key_path` | `Option<PathBuf>` | `None` | FR-021; if None, relies on SSH agent |
| `sandbox_base_dir` | `PathBuf` | system temp | Base dir for task sandboxes |
| `retain_sandboxes` | `bool` | `false` | Keep sandbox after completion |

---

## Source Module Layout

```text
src/daemon/
├── mod.rs               # Public re-exports
├── config.rs            # DaemonConfig (extends AppConfig)
├── task.rs              # Task, TaskStatus structs + serde impls
├── store.rs             # TaskStore — JSON file persistence
├── sandbox.rs           # Sandbox lifecycle + git operations
├── executor.rs          # TaskExecutor — orchestrates clone/run/commit/pr
├── permissions.rs       # Two-level permission resolution
├── pr_provider/
│   ├── mod.rs           # PrProvider trait + CreatePrParams + PrResult
│   └── github.rs        # GitHubPrProvider (reqwest impl)
└── api/
    ├── mod.rs           # Router construction (axum)
    ├── routes.rs        # Handler functions
    └── types.rs         # HTTP request/response types
```

```text
src/cli/
└── daemon.rs            # handle_daemon() — start/stop/status/submit CLI subcommands
```

`src/cli/mod.rs` gains `Daemon` in the `Commands` enum.
