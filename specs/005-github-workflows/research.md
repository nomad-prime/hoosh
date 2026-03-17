# Research: GitHub Workflow Triggers

**Feature**: `005-github-workflows` | **Date**: 2026-03-15

---

## Decision Log

### 1. Webhook Signature Verification Library

**Decision**: Use `hmac 0.12` + `sha2 0.10` + `hex 0.4` crates
**Rationale**: These are the RustCrypto standard crates, widely used, well-maintained, and provide `Mac::verify_slice()` for constant-time comparison which prevents timing attacks. `secrecy` crate wraps the secret to prevent accidental logging.
**Alternatives considered**:
- `ring` — heavier dependency, FIPS-compliant but overkill here
- Manual HMAC — never acceptable for security-sensitive code

### 2. PrProvider Replacement Strategy

**Decision**: Keep `PrProvider` as-is for manually submitted API tasks; skip it entirely for webhook-triggered tasks (agent uses `gh` CLI instead). Mark for future removal but do not remove in this feature.
**Rationale**: Minimizes risk of breaking existing functionality. The `gh` CLI is more flexible and already authenticated on the target machine. Removing `PrProvider` immediately would require verifying all existing consumers.
**Alternatives considered**:
- Remove `PrProvider` now — rejected, scope creep + risk
- Replace with `gh` CLI wrapper in Rust — rejected, unnecessary (agent can call `gh` directly via bash tool)

### 3. GitHub Event Parsing Approach

**Decision**: Deserialize only the fields we need (partial deserialization via `serde_json`). Use `#[serde(default)]` and `Option<T>` liberally to handle GitHub's varied payloads.
**Rationale**: GitHub webhook payloads are large and vary by event type. We don't need the full schema. Partial deserialization keeps the code lean.
**Alternatives considered**:
- `octocrab` crate — heavy dependency, brings in a full GitHub client we don't need
- `github-webhook` crate — unmaintained as of research date
- Full schema structs — unnecessary complexity

### 4. Supported Event Types

**Decision**: Support `issue_comment` (created), `pull_request_review` (submitted), and `pull_request_review_comment` (created).
**Rationale**:
- `issue_comment` covers @mentions in both issues AND PR comment threads (GitHub routes PR comments through this event)
- `pull_request_review` covers full review submissions with @mentions in the review body
- `pull_request_review_comment` covers inline code comments with @mentions
- Other events (push, issues opened, etc.) are lower priority and don't fit the @mention trigger model
**Alternatives considered**: Only `issue_comment` — insufficient, misses review-body mentions

### 5. Sandbox Branch Strategy When Branch Already Exists

**Decision**: If branch `hoosh/issue-{n}` already exists in the remote, append a short task ID suffix: `hoosh/issue-{n}-{short-id}` (first 6 chars of task UUID).
**Rationale**: Prevents collision when multiple hoosh tasks are triggered for the same issue. Keeps branches traceable to their source event.
**Alternatives considered**:
- Force-push to existing branch — dangerous, could overwrite in-progress work
- Error and fail — bad UX, user has to manually clean up

### 6. Deduplication Strategy

**Decision**: Before queuing a new task, check if a `Queued` or `Running` task with the same `trigger_ref` exists. If so, return `200 OK` and do not create a duplicate.
**Rationale**: GitHub may retry webhooks. Multiple @mentions in quick succession should not create multiple tasks.
**Alternatives considered**:
- Allow duplicates — results in race conditions and double-work
- Queue with lock per `trigger_ref` — over-engineered for low-frequency events

### 7. Agent Context Format

**Decision**: Plain text prompt (not structured JSON) with all relevant context: event type, actor, repo, issue/PR title + body, comment text, branch info, and operational hint about `gh` CLI.
**Rationale**: The agent (LLM) performs best with natural language context. Structured JSON requires the agent to parse it before acting; plain text reads naturally.
**Alternatives considered**:
- JSON context in system message — less readable for LLM
- Minimal context + tool calls to fetch details — adds latency and token overhead for easily-available data

### 8. `gh` CLI Availability Check

**Decision**: Check `gh auth status` as part of sandbox setup before starting the agent. If it fails, mark the task `Failed` with a clear error message.
**Rationale**: Fail fast with actionable error rather than letting the agent discover the missing tool mid-execution.
**Alternatives considered**:
- Check at daemon startup — not reliable, auth state can change; also, `gh` might be available in PATH but not authenticated

---

## Open Questions (Resolved)

| Question | Resolution |
|----------|-----------|
| Should webhook trigger tasks re-use the existing `POST /tasks` code path? | Yes — `executor.rs` is the single execution engine; webhook handler constructs a `Task` and dispatches it |
| Do we need to handle `pull_request` events (not just comments)? | No — @mention trigger model requires a comment. PR open/close events don't contain @mentions meaningfully |
| What repo URL format does the webhook payload use? | `repository.clone_url` (HTTPS) and `repository.ssh_url` — use HTTPS for clone (consistent with `gh` CLI auth) |
| Can the agent push to `hoosh/issue-{n}` without a PAT? | Yes, if `gh` is authenticated — `gh` sets up git credential helpers automatically |
