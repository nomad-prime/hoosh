# Tasks: GitHub Workflow Triggers

**Input**: Design documents from `/specs/005-github-workflows/`
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅

**Organization**: Tasks grouped by user story. US1 = webhook pipeline (receive → verify → detect → deduplicate → queue). US2 = executor adaptation (agent context, skip post-agent ops, gh auth check). US3 = API surface & deprecation. The daemon's job is: receive event → verify → detect mention → clone repo → pass raw event to agent. The agent handles all branching, git ops, and GitHub API calls.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: [US1], [US2], [US3] — maps to user story phase
- Exact file paths included in all descriptions

---

## Phase 1: Setup

- [X] T001 Add `hmac = "0.12"`, `sha2 = "0.10"`, `hex = "0.4"` to `[dependencies]` in `Cargo.toml`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Config extension, task model extension, and store query — required by all subsequent work.

- [X] T002 Add `GithubConfig` struct (`webhook_secret: Option<String>`, `mention_handle: String` with `#[serde(default = "default_mention_handle")]` where `fn default_mention_handle() -> String { "@hoosh".to_string() }`, `bot_login: Option<String>`) and embed as `pub github: GithubConfig` in `DaemonConfig` in `src/daemon/config.rs`
- [X] T003 [P] Add `GithubEventType` enum (`IssueComment`, `PullRequestReview`, `PullRequestReviewComment`) with `#[derive(Debug, Clone, Serialize, Deserialize)]` and `#[serde(rename_all = "snake_case")]` in `src/daemon/task.rs`
- [X] T004 [P] Add `GithubTrigger` struct with `#[derive(Debug, Clone, Serialize, Deserialize)]` in `src/daemon/task.rs`; fields: `event_type: GithubEventType`, `delivery_id: String`, `trigger_ref: String` (`"issue:47"` or `"pr:82"` for deduplication), `repo_full_name: String`, `repo_url: String` (from `repository.clone_url`), `default_branch: String` (from `repository.default_branch` — used as the `base_branch` arg to `Sandbox::clone()`), `actor_login: String`, `issue_or_pr_number: u64`, `comment_url: Option<String>`, `raw_payload: serde_json::Value` (full webhook JSON, passed verbatim to agent)
- [X] T005 Add `pub trigger: Option<GithubTrigger>` field to `Task` struct and update all `Task::new` / deserialization call sites in `src/daemon/task.rs` (depends T003, T004)
- [X] T006 Add `query_active_by_trigger_ref(&self, trigger_ref: &str) -> Option<TaskId>` to `TaskStore` that searches in-memory cache for `Queued` or `Running` tasks with matching `trigger_ref` in `src/daemon/store.rs` (depends T005)
- [X] T007a [P] Unit tests for startup warning behavior: assert a warning is emitted (via captured log output or a test-observable hook) when `GithubConfig.webhook_secret` is `None`; assert a warning is emitted when `GithubConfig.bot_login` is `None`; assert no warning when both are set (depends T002)
- [X] T007 Log startup warnings if `config.github.webhook_secret.is_none()` or `config.github.bot_login.is_none()` in the daemon startup/init path (where `DaemonConfig` is loaded, likely `main.rs` or daemon init — NOT in `src/daemon/api/mod.rs`) (depends T002, T007a)
- [X] T008 [P] Declare `pub mod github_event` and `mod webhook` in `src/daemon/mod.rs`

**Checkpoint**: Config, task model, and store query ready.

---

## Phase 3: US1 — Webhook Reception & Mention Detection

**Goal**: The daemon receives GitHub webhooks, verifies the signature, detects @hoosh mentions, deduplicates, and queues the task.

**Independent Test**: POST a valid signed `issue_comment` payload with `@hoosh` in the body → `202 Accepted` + task in store with `trigger.is_some()` and `trigger_ref == "issue:N"`. POST with invalid signature → `401`. POST without mention → `200 no_action`. POST with duplicate active task → `200 duplicate`.

### Tests

- [X] T009 [P] [US1] Unit tests for `verify_signature()` in `src/daemon/webhook.rs`: valid HMAC accepted, tampered body rejected, missing/malformed header returns error
- [X] T010 [P] [US1] Unit tests for mention detection and bot filter in `src/daemon/github_event.rs`: handle present, handle absent, case-sensitive non-match; bot_login matches sender (returns None), bot_login does not match (passes through), bot_login is None (passes through)
- [X] T011 [P] [US1] Unit tests for `parse_github_event()` in `src/daemon/github_event.rs`: all three payload types return correct `GithubTrigger` (verify `trigger_ref`, `actor_login`, `default_branch`, `repo_url`, `raw_payload` round-trips); `issue_comment` on a PR thread (`issue.pull_request` present) returns `trigger_ref = "pr:N"` not `"issue:N"`; unsupported action returns `None`; bot sender returns `None`
- [X] T012 [P] [US1] Unit tests for `query_active_by_trigger_ref` in `src/daemon/store.rs`: `Queued` task with matching `trigger_ref` returns its ID; `Running` task with matching `trigger_ref` returns its ID; `Completed` and `Failed` tasks with matching `trigger_ref` return `None`; empty store returns `None`

### Implementation

- [X] T013 [P] [US1] Create minimal payload deserialization structs (`IssueCommentPayload`, `PullRequestReviewPayload`, `PullRequestReviewCommentPayload`, plus supporting structs `CommentBody`, `IssueRef`, `RepoRef`, `ActorRef`, `ReviewBody`, `PrRef`, `BranchRef`, `ReviewCommentBody`) as `pub(crate)` in `src/daemon/github_event.rs`; extract only: `action`, `sender.login`, `repository.clone_url`, `repository.full_name`, `repository.default_branch`, `issue.number` (with `issue.pull_request: Option<serde_json::Value>` to detect PR-thread comments), `pull_request.number`, `comment.body` / `review.body`, `comment.html_url`; `BranchRef` uses `#[serde(rename = "ref")]`
- [X] T014 [US1] Implement `mentions_handle(body: &str, handle: &str) -> bool` and `is_bot_sender(login: &str, bot_login: Option<&str>) -> bool` in `src/daemon/github_event.rs` (depends T013)
- [X] T015 [US1] Implement `parse_github_event(event_type: &str, payload: &[u8], mention_handle: &str, bot_login: Option<&str>) -> Result<Option<GithubTrigger>>` in `src/daemon/github_event.rs` (depends T014): routes on event type string; deserializes payload; filters action (`created` / `submitted`); checks mention and bot sender; for `issue_comment` sets `trigger_ref = "pr:N"` if `issue.pull_request` is present else `"issue:N"`; for `pull_request_review` and `pull_request_review_comment` always `"pr:N"`; stores full raw JSON as `raw_payload`; returns `None` for all no-op cases; returns `Err` only on deserialization failure
- [X] T016 [P] [US1] Implement `verify_signature(secret: &str, body: &[u8], signature_header: &str) -> bool` using `hmac::Hmac<sha2::Sha256>` with `Mac::verify_slice()` for constant-time comparison in `src/daemon/webhook.rs`
- [X] T017 [US1] Implement `handle_github_webhook()` axum handler in `src/daemon/webhook.rs` (depends T015, T016): extract `X-GitHub-Event`, `X-Hub-Signature-256`, `X-GitHub-Delivery` headers; return `500 {"error":"not_configured","detail":"github.webhook_secret is not set in daemon config"}` if `webhook_secret` is `None`; verify signature, return `401 {"error":"invalid_signature"}` on failure; call `parse_github_event`, return `200 {"status":"no_action","reason":"..."}` on `None`; return `422 {"error":"invalid_payload","detail":"<serde error message>"}` on `Err`; call `store.query_active_by_trigger_ref`, return `200 {"status":"no_action","reason":"duplicate","existing_task_id":"..."}` if found; construct agent message as `format!("You have been mentioned in a GitHub {event_type} event. The repository is already cloned at your working directory.\n\n<event>\n{pretty_json}\n</event>\n\nUse \`gh\` CLI and git for all GitHub operations. Determine the appropriate branch strategy from the event context, make your changes, and push. Do not wait for further input.")` where `pretty_json = serde_json::to_string_pretty(&trigger.raw_payload)?`; create `Task` with `repo_url = trigger.repo_url`, `base_branch = trigger.default_branch`, `instructions = agent_message`, `pr_title = None`, `token_budget` from `DaemonConfig`; set `task.trigger = Some(trigger)`; persist and dispatch with `tokio::spawn`; return `202 {"status":"accepted","task_id":"..."}`
- [X] T018 [US1] Register `POST /github/webhook` route with shared `AppState` in `src/daemon/api/mod.rs` (depends T017)

**Checkpoint**: Webhook endpoint fully functional — signature verified, mentions detected, deduplication enforced, task queued async.

---

## Phase 4: US2 — Agent Execution via Webhook

**Goal**: Agent receives full event context and runs without daemon post-agent git operations; `gh` auth failure is caught before the agent starts.

**Independent Test**: POST signed `issue_comment` with `@hoosh` → task completes without any `PrProvider` call (verified via mock with 0-call assertion). POST with unauthenticated `gh` → task immediately transitions to `Failed` with message containing `"gh auth login"`. Clone failure → task immediately transitions to `Failed`.

### Tests

- [X] T019 [P] [US2] Integration test in `src/daemon/executor_webhook_tests.rs` (referenced via `#[cfg(test)] #[path = "executor_webhook_tests.rs"] mod tests;` in `executor.rs`): build minimal `AppState` with local bare-repo `file://` remote and mock backend; record the time before POSTing a signed `issue_comment` payload (with `@hoosh` in body) to `handle_github_webhook()`; assert `202 Accepted` is returned within 100ms (use `std::time::Instant` — the response must arrive before sandbox clone completes, confirming async dispatch); assert task in store with `trigger.is_some()`, `trigger.trigger_ref == "issue:N"`, and `instructions` containing `<event>` block; assert executor completes without calling `PrProvider` (use `MockPrProvider` with call-count assertion of 0)
- [X] T020 [P] [US2] Test clone failure path in `src/daemon/executor_webhook_tests.rs`: mock a `Sandbox::clone()` that returns `Err`; assert task transitions to `Failed` with error logged; assert no agent is started

### Implementation

- [X] T021 [US2] Extend `TaskExecutor::execute()` in `src/daemon/executor.rs` (depends T005): for webhook-triggered tasks (`task.trigger.is_some()`), if `Sandbox::clone()` returns `Err`, mark task `Failed` with the clone error and return before starting the agent; before starting the agent run `gh auth status` as a subprocess; if it fails, mark task `Failed` with message `"gh CLI not authenticated — run 'gh auth login' on the daemon machine"` and return; after agent turn completes, skip the entire post-agent block (both the normal path: commit → push → `PrProvider::create_pull_request`, and the incomplete path: "[incomplete]" commit + push); mark task `Completed` or `Failed` and return

**Checkpoint**: Webhook-triggered tasks run to completion with agent owning all git/GitHub ops; daemon never calls `PrProvider` for webhook tasks; `gh` auth failure fails fast with actionable message.

---

## Phase 5: US3 — API Surface & Deprecation

**Goal**: `GET /tasks/:id` exposes the `trigger` field; `webhook_secret` is never leaked via API or logs; `PrProvider` is marked deprecated.

**Independent Test**: `GET /tasks/:id` for a webhook-triggered task returns JSON with a `trigger` object containing `event_type`, `delivery_id`, `trigger_ref`, etc. and no `webhook_secret` field anywhere. `cargo clippy` emits a deprecation warning for any new `PrProvider` usage.

### Tests

- [X] T022 [P] [US3] Unit/integration test in `src/daemon/api/types.rs`: serialize a `TaskResponse` that contains a `GithubTriggerResponse` with a non-empty `raw_payload` in the underlying `GithubTrigger`; assert the resulting JSON contains `trigger.event_type`, `trigger.trigger_ref`, `trigger.actor_login`; assert the JSON does NOT contain `raw_payload` or `webhook_secret`

### Implementation

- [X] T023 [P] [US3] Introduce a `GithubTriggerResponse` struct in `src/daemon/api/types.rs` that mirrors `GithubTrigger` but omits `raw_payload`; implement a `From<&GithubTrigger>` conversion; update `TaskResponse` in `src/daemon/api/types.rs` to include `trigger: Option<GithubTriggerResponse>` (do NOT modify serde attributes on `GithubTrigger` in `task.rs` — that struct must retain `raw_payload` for full disk persistence); verify `webhook_secret` is absent from all serialized response types; use `grep -rn "webhook_secret" src/daemon/webhook.rs src/daemon/api/mod.rs` to confirm no log macros (`tracing::debug!`, `tracing::info!`, `tracing::error!`, etc.) interpolate `webhook_secret` — if any are found, fix them before marking this task complete
- [X] T024 [P] [US3] Add `#[deprecated(note = "Use gh CLI via agent for PR creation")]` to `PrProvider` trait in `src/daemon/pr_provider/mod.rs` and to `GitHubPrProvider` impl in `src/daemon/pr_provider/github.rs`

**Checkpoint**: API response includes `trigger`; no secret leakage; `PrProvider` marked for future removal.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T025 [P] Run `cargo clippy` and fix all warnings in `src/daemon/`
- [X] T026 [P] Run `cargo fmt`
- [X] T027 Validate end-to-end flow per `specs/005-github-workflows/quickstart.md`; the automated timing assertion in T019 covers the 100ms NFR — confirm no regressions by re-running `cargo test` after T025/T026

---

## Dependencies & Execution Order

- **Phase 1**: No dependencies — start immediately
- **Phase 2**: Depends on Phase 1; T003, T004, T008 can run in parallel immediately; T002 must complete before T007a and T007; T007a must complete before T007; T005 depends on T003 + T004; T006 depends on T005
- **Phase 3**: Depends on Phase 2 complete; T009, T010, T011, T012 (tests) MUST be written first before T013–T017 (implementation); T013, T016 can start in parallel; T014 depends on T013; T015 depends on T014; T017 depends on T015 + T016; T018 depends on T017
- **Phase 4**: Depends on Phase 3 complete; T019, T020 (tests) written before T021; T021 depends on T005
- **Phase 5**: Depends on Phase 3 complete; T022 (test) before T023 (impl); T023, T024 in parallel
- **Phase 6**: Depends on all other phases complete; T025, T026 in parallel; T027 after both

---

## Parallel Execution Examples

### Phase 2 Parallel Group (after T002 completes)
```
Task T003:  Add GithubEventType enum in src/daemon/task.rs
Task T004:  Add GithubTrigger struct in src/daemon/task.rs
Task T007a: Unit tests for startup warning behavior
Task T008:  Declare modules in src/daemon/mod.rs
```
Note: T007 (implementation) runs after T007a (tests), per test-first requirement.
Note: T003 and T004 both modify src/daemon/task.rs — coordinate to avoid conflicts.

### Phase 3 Test Group (write before any implementation)
```
Task T009: Unit tests for verify_signature() in src/daemon/webhook.rs
Task T010: Unit tests for mention detection in src/daemon/github_event.rs
Task T011: Unit tests for parse_github_event() in src/daemon/github_event.rs
Task T012: Unit tests for query_active_by_trigger_ref in src/daemon/store.rs
```

### Phase 3 Implementation Parallel Group
```
Task T013: Payload deserialization structs in src/daemon/github_event.rs
Task T016: verify_signature() in src/daemon/webhook.rs
```

---

## Implementation Strategy

### MVP (US1 Only)

1. Complete Phase 1 + Phase 2
2. Complete Phase 3 (US1) — webhook receives, verifies, detects, deduplicates, queues
3. **Validate**: POST signed webhook → 202 + task in store
4. Continue to Phase 4 (US2) to make tasks actually execute correctly

### Incremental Delivery

1. Setup + Foundational → data types ready
2. US1 complete → webhook endpoint fully operational, tasks queued
3. US2 complete → agent runs correctly, no daemon post-agent interference
4. US3 complete → clean API surface, deprecation warnings in place
5. Polish → ship
