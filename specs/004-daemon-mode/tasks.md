# Tasks: Daemon Mode

**Input**: Design documents from `/specs/004-daemon-mode/`
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/api.md ✅, quickstart.md ✅

**Organization**: Tasks grouped by user story for independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: User story this task belongs to (US1–US4)

## Path Conventions

Single project: `src/` and `tests/` at repository root.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Add new dependencies and create the daemon module skeleton.

- [X] T001 Add daemon dependencies to `Cargo.toml`: `axum = "0.7"`, `tower = "0.4"`, `uuid = { version = "1.0", features = ["v4"] }`, `git2 = "0.19"`, `auth-git2 = "0.5"`, `nix = { version = "0.29", features = ["signal"] }`; also move `tempfile` from `[dev-dependencies]` to `[dependencies]` (needed for atomic writes in production `TaskStore` code)
- [X] T002 Create empty module files for `src/daemon/` directory: `mod.rs`, `config.rs`, `task.rs`, `store.rs`, `sandbox.rs`, `executor.rs`, `permissions.rs`, `pr_provider/mod.rs`, `pr_provider/github.rs`, `api/mod.rs`, `api/routes.rs`, `api/types.rs`
- [X] T003 [P] Create empty `src/cli/daemon.rs` and register `pub mod daemon` in `src/cli/mod.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core data structures and persistence layer that every user story depends on.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [X] T004 Implement `DaemonConfig` in `src/daemon/config.rs`: fields `bind_address: SocketAddr` (default `127.0.0.1:7979`), `default_token_budget: usize` (default `100_000`), `github_pat: Option<String>`, `ssh_key_path: Option<PathBuf>`, `sandbox_base_dir: PathBuf` (default system temp), `retain_sandboxes: bool` (default `false`); derive `Debug`, `Deserialize`, `Serialize`, `Clone`; also add `pub daemon: Option<DaemonConfig>` with `#[serde(default)]` to `AppConfig` in `src/config/mod.rs` — this is the only required touch to that file
- [X] T005 [P] Implement `Task` struct and `TaskStatus` enum in `src/daemon/task.rs`: all fields from data-model.md (`id`, `repo_url`, `base_branch`, `instructions`, `pr_title`, `pr_labels`, `token_budget`, `status`, `created_at`, `started_at`, `completed_at`, `pr_url`, `branch`, `tokens_consumed`, `error_message`, `sandbox_path`, `log_path`); derive `Serialize`, `Deserialize`, `Clone`, `Debug`; implement `Task::new(repo_url, base_branch, instructions, token_budget, global_default)` that generates `hoosh-<uuid>` ID
- [X] T006 Implement `TaskStore` in `src/daemon/store.rs`: persist each task as `~/.hoosh/daemon/tasks/<task-id>.json`; methods `create(&Task) -> Result<()>`, `update(&Task) -> Result<()>` (atomic write via tempfile + rename using `tempfile` crate already in dev-deps — add as regular dep), `get(id: &str) -> Result<Option<Task>>`, `load_all() -> Result<Vec<Task>>`; wrap in `Arc<tokio::sync::RwLock<HashMap<String,Task>>>` for in-memory cache
- [X] T007 Add unit tests for `TaskStore` at the bottom of `src/daemon/store.rs`: test `create_and_load_persists_to_disk`, `update_changes_status`, `load_all_returns_all_tasks`, `crash_recovery_marks_running_as_failed` (write a task with `Running` status, drop store, reload, verify status is `Failed` with `[incomplete]` marker)
- [X] T008 Wire `src/daemon/mod.rs`: `pub use` re-exports for `DaemonConfig`, `Task`, `TaskStatus`, `TaskStore`, and (to be added later) `DaemonServer`; add `pub mod daemon;` to `src/lib.rs`

**Checkpoint**: `cargo test src/daemon/store.rs` passes — foundation ready.

---

## Phase 3: User Story 1 — Submit a Coding Task and Receive a PR (Priority: P1) 🎯 MVP

**Goal**: A developer submits a task; the daemon clones the repo, runs the agent, commits changes, and opens a GitHub PR — all autonomously.

**Independent Test**: Start daemon, `POST /tasks` with a real (or mock) repo URL and instructions, poll `GET /tasks/{id}` until terminal, verify PR URL is set or status is `completed` with no PR (no changes case).

### Implementation for User Story 1

- [X] T009 Implement `Sandbox` in `src/daemon/sandbox.rs`: `Sandbox::create(task_id: &str, base_dir: &PathBuf) -> Result<Sandbox>` creates temp dir at `<base_dir>/hoosh-<task-id>/repo/` and opens `execution.log` for append; `clone(repo_url, base_branch, ssh_key_path) -> Result<()>` uses `git2::build::RepoBuilder` with `auth_git2::GitAuthenticator` SSH callbacks inside `tokio::time::timeout(Duration::from_secs(300), tokio::task::spawn_blocking(...))` — returns `Err` with a clear message on timeout; `create_branch(branch_name) -> Result<()>` creates and checks out `hoosh/<task-id>` branch via `git2`; `has_changes() -> Result<bool>` uses `repo.statuses(None)?` and checks if any entry has status flags containing `INDEX_NEW | INDEX_MODIFIED | INDEX_DELETED | WT_NEW | WT_MODIFIED | WT_DELETED`; `commit_all(message: &str) -> Result<()>` stages all via `repo.index().add_all`, writes tree, creates commit with repo signature; `push(branch_name, ssh_key_path) -> Result<()>` wraps the `spawn_blocking` push in `tokio::time::timeout(Duration::from_secs(120), ...)` — returns `Err` on timeout; `cleanup() -> Result<()>` removes the temp dir (skipped if `retain_sandboxes`)
- [X] T010 [P] Add unit tests for `Sandbox` at the bottom of `src/daemon/sandbox.rs`: use a `tempfile::TempDir` bare repo (`git2::Repository::init_bare(&tmp)`) as the "remote" and clone from it via `file://` URL — no network required; tests: `create_makes_sandbox_directory`, `clone_creates_repo_at_sandbox_path`, `create_branch_checks_out_new_branch`, `has_changes_returns_false_on_clean_repo`, `has_changes_returns_true_after_writing_file`, `commit_all_creates_commit_with_expected_message`, `push_sends_commit_to_bare_remote` (verify by opening bare repo and checking HEAD commit), `cleanup_removes_directory`
- [X] T011 [P] Implement `PrProvider` trait in `src/daemon/pr_provider/mod.rs`: `CreatePrParams { repo_url, head_branch, base_branch, title, body, labels }`, `PrResult { pr_url, pr_number }`; trait `PrProvider: Send + Sync { async fn create_pull_request(&self, params: CreatePrParams) -> Result<PrResult>; fn provider_name(&self) -> &'static str; }`
- [X] T012 Implement `GitHubPrProvider` in `src/daemon/pr_provider/github.rs`: implement `fn parse_github_remote(url: &str) -> Result<(String, String)>` that handles SSH format `git@github.com:owner/repo.git` (split on `:`, strip `.git`) and HTTPS format `https://github.com/owner/repo[.git]` (split on `/`, take last two segments); return a clear `anyhow::bail!` if neither pattern matches; use parsed `(owner, repo)` in `POST https://api.github.com/repos/{owner}/{repo}/pulls` with `Authorization: Bearer <pat>`, `Accept: application/vnd.github+json`, `X-GitHub-Api-Version: 2022-11-28`; body: `{ title, body, head: head_branch, base: base_branch }`; parse response `html_url` and `number` into `PrResult`; return structured `anyhow::Error` on non-2xx
- [X] T013 [P] Add unit tests for `GitHubPrProvider` at the bottom of `src/daemon/pr_provider/github.rs` using `httpmock`: `creates_pr_returns_url`, `handles_api_error_with_message`, `parses_ssh_url_extracts_owner_and_repo`, `parses_https_url_extracts_owner_and_repo`, `parses_https_url_without_git_suffix`, `invalid_url_returns_clear_error`
- [X] T014 Implement `TaskExecutor` in `src/daemon/executor.rs`: `TaskExecutor { store: Arc<TaskStore>, config: Arc<DaemonConfig>, pr_provider: Arc<dyn PrProvider> }`; method `run(task_id: String, cancel: Arc<AtomicBool>) -> Result<()>` that: (1) updates status to `Running`, (2) creates `Sandbox`, (3) clones repo, (4) resolves permissions (via `PermissionResolver` — stub returning global-only in US1, full impl in US3), (5) builds `Agent::new(...).with_cancellation_token(Arc::clone(&cancel))` using the new builder added to `src/agent/core.rs`; (6) subscribes to `AgentEvent::TokenUsage` via mpsc channel — maintain a running `total_tokens: usize` counter, incrementing by `input_tokens + output_tokens` on each event; when `total_tokens >= effective_budget` set `cancel.store(true, Ordering::Relaxed)` to signal cooperative cancellation; (7) on agent completion: if `has_changes()` — commit with `"hoosh: <instructions[:72]>"`, push, create PR, update task with `pr_url`; if no changes — update status to `Completed`; (8) on any error or when cancel was set: attempt `commit_all("[incomplete] <reason>")` + push, update status to `Failed` with `error_message`; (9) sandbox cleanup
- [X] T015 [P] Add unit tests for `TaskExecutor` at the bottom of `src/daemon/executor.rs` using mock `LlmBackend` and a mock `PrProvider`: `happy_path_creates_pr`, `no_changes_completes_without_pr`, `token_exhaustion_sets_cancel_flag_and_marks_failed`, `external_cancel_marks_failed_with_incomplete`
- [X] T016 [P] Implement HTTP request/response types in `src/daemon/api/types.rs`: `SubmitTaskRequest { repo_url, base_branch, instructions, pr_title, pr_labels, token_budget }`, `SubmitTaskResponse { task_id }`, `TaskResponse` (full task fields for GET), `HealthResponse { status, version, uptime_seconds, active_tasks, shutting_down }`, `ErrorResponse { error }`; all derive `Serialize`, `Deserialize`
- [X] T017 Implement `POST /tasks` route handler in `src/daemon/api/routes.rs`: validate `repo_url`, `base_branch`, `instructions` non-empty (return `400` with `ErrorResponse` if invalid); return `503` if `shutting_down`; create `Task` via `TaskStore`; create a fresh `Arc<AtomicBool>` cancel flag; spawn `TaskExecutor::run(task_id, Arc::clone(&cancel))` in `tokio::spawn`; store `(JoinHandle<()>, Arc<AtomicBool>)` in shared state map keyed by task ID; return `202` with `SubmitTaskResponse`
- [X] T018 Implement `DaemonServer` in `src/daemon/api/mod.rs`: struct holding `Arc<TaskStore>`, `Arc<TaskExecutor>`, `Arc<DaemonConfig>`, shared state map `Arc<RwLock<HashMap<String, (JoinHandle<()>, Arc<AtomicBool>)>>>`, `uptime_start: Instant`, `shutting_down: Arc<AtomicBool>`; method `new(config, store, executor) -> Self`; method `router(&self) -> Router` builds axum router with `POST /tasks` only (others added in US2)
- [X] T019 Implement `DaemonServer::start()` in `src/daemon/api/mod.rs`: scan `TaskStore::load_all()` for `Running` tasks and mark them `Failed` with `"[incomplete] daemon restarted unexpectedly"` (FR-023); bind `TcpListener` to `config.bind_address`; serve axum router; expose `DaemonServer` from `src/daemon/mod.rs`

**Checkpoint**: `cargo test` passes. Start daemon manually (`hoosh daemon start` skeleton), `curl -X POST .../tasks` returns a task ID.

---

## Phase 4: User Story 2 — Monitor and Cancel Tasks (Priority: P2)

**Goal**: Developers and CI systems can list tasks, poll status, view logs, cancel running tasks, and check daemon health.

**Independent Test**: Submit several tasks, poll `GET /tasks` and `GET /tasks/{id}`, cancel one with `DELETE /tasks/{id}`, verify status transitions; `GET /health` returns `{"status":"ok"}`.

### Implementation for User Story 2

- [X] T020 Add log writing to `TaskExecutor` in `src/daemon/executor.rs`: write timestamped lines to `sandbox.log_file` for each significant event (clone started/completed, branch created, agent started, each token usage update, commit, push, PR created, failure reason); update `task.log_path` in store after sandbox creation
- [X] T021 [P] Add `GET /tasks` route handler in `src/daemon/api/routes.rs`: load all tasks from `TaskStore`, serialize as JSON array of `TaskResponse`, return `200`
- [X] T022 [P] Add `GET /tasks/{id}` route handler in `src/daemon/api/routes.rs`: load task by ID from `TaskStore`; return `200` with `TaskResponse` or `404` with `ErrorResponse`
- [X] T023 [P] Add `DELETE /tasks/{id}` route handler in `src/daemon/api/routes.rs`: look up task; return `404` if not found; return `409` if status is terminal (`Completed`, `Failed`, `Cancelled`); retrieve `Arc<AtomicBool>` cancel flag from shared state map and call `cancel.store(true, Ordering::Relaxed)` — the agent's step loop will exit cooperatively at its next step boundary; update task status to `Cancelled`; return `204`
- [X] T024 [P] Add `GET /tasks/{id}/logs` route handler in `src/daemon/api/routes.rs`: load task by ID; read `log_path` file as UTF-8 string; return `200` with `Content-Type: text/plain; charset=utf-8`; return `404` if task unknown or log file doesn't exist yet
- [X] T025 [P] Add `GET /health` route handler in `src/daemon/api/routes.rs`: count active tasks (non-terminal) from in-memory map; return `200` with `HealthResponse { status: "ok", version, uptime_seconds, active_tasks, shutting_down }`
- [X] T026 Register all new routes in `src/daemon/api/mod.rs`: update `router()` to add `GET /tasks`, `GET /tasks/:id`, `DELETE /tasks/:id`, `GET /tasks/:id/logs`, `GET /health`
- [X] T027 Implement graceful shutdown in `src/daemon/api/mod.rs`: `DaemonServer::shutdown(force: bool)` — if `!force`: set `shutting_down = true`, await all `JoinHandle`s (tasks drain naturally); if `force`: set `shutting_down = true`, iterate shared state map and set each `Arc<AtomicBool>` cancel flag to `true`, then await all `JoinHandle`s; install `tokio::signal::ctrl_c()` + `tokio::signal::unix::signal(SIGTERM)` handlers in `start()` to trigger graceful shutdown

**Checkpoint**: `cargo test` passes. All six HTTP endpoints respond correctly; cancel transitions task to `Cancelled`; logs endpoint streams execution output.

---

## Phase 5: User Story 3 — Permission Control (Priority: P3)

**Goal**: An administrator sets a global permission baseline; repo maintainers can extend (but not exceed) it per repo. Tasks are halted on denial with partial work committed.

**Independent Test**: Configure a global deny on a specific operation, submit a task that requires it, verify task ends as `Failed` with an `[incomplete]` marker; verify repo-level allow that conflicts with global deny is silently dropped.

### Implementation for User Story 3

- [X] T028 Implement `PermissionResolver` in `src/daemon/permissions.rs`: `resolve(global: PermissionsFile, repo_level: Option<PermissionsFile>) -> PermissionsFile` — if no repo file, return global unchanged; otherwise: `combined_deny = union(global.deny, repo.deny)`; `filtered_repo_allow = repo.allow.filter(|r| !global.deny.iter().any(|d| d.conflicts_with(r)))`; `combined_allow = union(global.allow, filtered_repo_allow)`; return merged `PermissionsFile`; add `load_global() -> Result<PermissionsFile>` that loads `~/.hoosh/permissions.json` via existing `PermissionsFile::load_permissions_safe`; add `load_repo(repo_path: &Path) -> Option<PermissionsFile>` that loads `<repo>/.hoosh/permissions.json`
- [X] T029 [P] Add unit tests for `PermissionResolver` at the bottom of `src/daemon/permissions.rs`: `global_only_returns_global`, `repo_extends_global_allows`, `repo_deny_adds_to_global_deny`, `repo_allow_conflicting_with_global_deny_is_dropped`, `no_repo_file_returns_global_unchanged`; use `PermissionsFile` and `PermissionRule` from existing `src/permissions/storage.rs`
- [X] T030 Integrate `PermissionResolver` into `TaskExecutor::run()` in `src/daemon/executor.rs`: after clone, call `PermissionResolver::load_global()` and `PermissionResolver::load_repo(&sandbox.repo_dir)`; call `PermissionResolver::resolve(global, repo_level)`; construct `PermissionManager::non_interactive(merged_permissions_file)` (the new headless constructor added in `src/permissions/mod.rs` — pre-loads the merged file, denies unknown tools without prompting) and pass to `ToolExecutor`; replace the US1 stub

**Checkpoint**: `cargo test` passes. Permission denial mid-task produces a `Failed` task with `[incomplete]` in commit message; repo-level override test confirms deny-wins behaviour.

---

## Phase 6: User Story 4 — Start, Stop, and Inspect the Daemon via CLI (Priority: P4)

**Goal**: Developers manage the daemon lifecycle from the terminal without constructing HTTP requests.

**Independent Test**: `hoosh daemon start`, verify process appears in `hoosh daemon status`, run `hoosh daemon stop`, verify process exits; `hoosh daemon submit --repo ... --branch ... --instructions ...` prints a task ID.

### Implementation for User Story 4

- [X] T031 Add `Daemon` variant to `Commands` enum in `src/cli/mod.rs` with `action: DaemonAction` subcommand; add `pub use daemon::handle_daemon` export
- [X] T032 Implement `DaemonAction` enum in `src/cli/mod.rs`: `Start { #[arg(long)] port: Option<u16> }`, `Stop { #[arg(long)] force: bool }`, `Status`, `Submit { #[arg(long)] repo: String, #[arg(long)] branch: String, #[arg(long)] instructions: String, #[arg(long)] pr_title: Option<String>, #[arg(long = "label")] labels: Vec<String>, #[arg(long)] token_budget: Option<usize> }`
- [X] T033 Implement `handle_daemon(action: DaemonAction, config: AppConfig) -> Result<()>` in `src/cli/daemon.rs`: dispatch to `start`, `stop`, `status`, `submit` handlers; read daemon port from config (or CLI override)
- [X] T034 Implement `daemon_start(port: Option<u16>, config: DaemonConfig) -> Result<()>` in `src/cli/daemon.rs`: build `DaemonServer` and call `.start().await` — always runs in the foreground; process lifecycle (backgrounding, restart-on-failure) is the operator's concern via systemd/launchd; remove `--foreground` flag from T032's `Start` variant
- [X] T035 [P] Implement `daemon_stop(force: bool) -> Result<()>` in `src/cli/daemon.rs`: read PID from `~/.hoosh/daemon.pid`; if `force` send `SIGKILL` via `libc` or `nix` crate; otherwise send `SIGTERM`; wait for process to exit (poll `kill -0 <pid>` with 100ms intervals, 10s timeout); remove PID file; print confirmation
- [X] T036 [P] Implement `daemon_status() -> Result<()>` in `src/cli/daemon.rs`: read PID file; if missing — print `daemon is not running`; check liveness with `kill -0 <pid>`; if alive — `GET http://127.0.0.1:{port}/health` and print status + uptime + active tasks; if PID exists but process dead — print `daemon is not running (stale PID file)` and delete PID file
- [X] T037 [P] Implement `daemon_submit(args, port) -> Result<()>` in `src/cli/daemon.rs`: build `SubmitTaskRequest` from CLI args; `POST http://127.0.0.1:{port}/tasks` using `reqwest` blocking client or `tokio` async; print `task_id` to stdout on success; print error to stderr on failure
- [X] T038 Wire `Commands::Daemon` in `src/main.rs`: add `Some(Commands::Daemon { action })` match arm that calls `AppConfig::load()` (same pattern as the `None` chat arm — required so `config.daemon` is available), then calls `handle_daemon(action, config).await?`

**Checkpoint**: Full lifecycle test: `hoosh daemon start` → `hoosh daemon status` shows running → `hoosh daemon submit ...` returns task ID → `hoosh daemon stop` exits cleanly.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [X] T039 Add integration test for full task flow in `tests/integration/daemon_task_flow.rs`: start `DaemonServer` in-process with mock `LlmBackend` and mock `PrProvider`; tests: `submit_task_completes_with_pr` (POST → poll until Completed → pr_url set), `submit_task_no_changes_completes_without_pr`, `cancel_running_task_transitions_to_cancelled` (POST → DELETE mid-run → poll until Cancelled), `submit_with_missing_field_returns_400` (empty `repo_url`), `get_unknown_task_returns_404`, `cancel_completed_task_returns_409`
- [X] T040 [P] Run `cargo clippy --all-targets -- -D warnings` and fix all reported issues
- [X] T041 [P] Run `cargo fmt --check` and format any unformatted files
- [ ] T042 [P] Validate quickstart.md scenarios manually: start daemon, submit task with mock credentials, verify all HTTP endpoints respond as documented in `contracts/api.md`

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 — **BLOCKS all user stories**
- **US1 (Phase 3)**: Depends on Phase 2 — first deliverable, MVP
- **US2 (Phase 4)**: Depends on Phase 2 — can start in parallel with US1 after Phase 2
- **US3 (Phase 5)**: Depends on Phase 3 (needs `executor.rs` stub to replace)
- **US4 (Phase 6)**: Depends on Phase 3 (needs `DaemonServer::start()` to call)
- **Polish (Phase 7)**: Depends on all desired stories complete

### User Story Dependencies

- **US1 (P1)**: After Foundational — no story dependencies
- **US2 (P2)**: After Foundational — independent of US1 (different routes, same store)
- **US3 (P3)**: After US1 — replaces the permission stub in `executor.rs`
- **US4 (P4)**: After US1 — wraps `DaemonServer` in CLI commands

### Within Each User Story

- Types/models → services/logic → HTTP handlers → wiring
- Tests written alongside implementation (bottom of same file per AGENTS.md)
- Core implementation before integration with other stories

### Parallel Opportunities

- T002 + T003 (Phase 1): parallel
- T004 + T005 (Phase 2): parallel (different files)
- T009 + T011 + T016 (Phase 3): parallel after T008
- T010 + T013 + T015 (Phase 3 tests): parallel (different files)
- T021 + T022 + T023 + T024 + T025 (Phase 4 routes): all parallel after T026 plan is known
- T029 (Phase 5 tests): parallel with T028 after T028 signature is defined
- T035 + T036 + T037 (Phase 6): parallel after T034 defines daemon port

---

## Parallel Example: User Story 1

```
# After Phase 2 completes, these can start simultaneously:
Task T009: Implement Sandbox in src/daemon/sandbox.rs
Task T011: Implement PrProvider trait in src/daemon/pr_provider/mod.rs
Task T016: Implement HTTP types in src/daemon/api/types.rs

# Once T009 is drafted (interface known), these can run in parallel:
Task T010: Unit tests for Sandbox
Task T012: Implement GitHubPrProvider (depends on T011 interface)

# Once T011 + T012 are done:
Task T013: Unit tests for GitHubPrProvider

# Once T009 + T011 + T012 are done:
Task T014: Implement TaskExecutor
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (**CRITICAL — blocks everything**)
3. Complete Phase 3: User Story 1 (submit task → agent runs → PR created)
4. **STOP and VALIDATE**: `POST /tasks`, poll status, verify PR appears
5. Ship MVP — core value proposition delivered

### Incremental Delivery

1. Setup + Foundational → task persistence working
2. US1 → end-to-end automated PR creation (demo!)
3. US2 → operational visibility (list, status, cancel, logs, health)
4. US3 → permission safety gates enabled
5. US4 → full CLI UX (no curl required)
6. Polish → production-ready

### Parallel Team Strategy

After Phase 2:
- Developer A: US1 (executor + sandbox + PR provider)
- Developer B: US2 (HTTP monitoring routes + shutdown)
- Both merge after their stories are independently tested

---

## Notes

- [P] tasks touch different files — safe to run in parallel
- [Story] label maps each task to its user story for traceability
- Tests are co-located at the bottom of each source file per AGENTS.md convention
- Avoid `unwrap()` in production code — use `?` with `anyhow::Context`
- Verify `cargo clippy` passes after each phase
- Commit after each task or logical group
- The `PermissionManager` already exists in `src/permissions/mod.rs` — `TaskExecutor` constructs one using merged rules, not the interactive TUI flow
