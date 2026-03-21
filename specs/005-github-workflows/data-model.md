# Data Model: GitHub Workflow Triggers

**Feature**: `005-github-workflows` | **Date**: 2026-03-15

---

## New Types

### `GithubConfig` (extends `DaemonConfig`)

```rust
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct GithubConfig {
    /// HMAC-SHA256 webhook signing secret configured in GitHub repo settings
    pub webhook_secret: Option<String>,
    /// Handle to watch for in comments. Defaults to "@hoosh" if omitted in config.
    #[serde(default = "default_mention_handle")]
    pub mention_handle: String,
    /// GitHub login of the bot account; events from this sender are ignored.
    /// If unset, self-trigger protection is disabled and daemon logs a startup warning.
    pub bot_login: Option<String>,
}

fn default_mention_handle() -> String {
    "@hoosh".to_string()
}
```

**Location**: `src/daemon/config.rs`
**Validation**: If `webhook_secret` is missing and webhook route receives a request, return `500` with a configuration error (not `401`). If `bot_login` is missing, daemon logs a startup warning and self-trigger protection is disabled.

---

### `GithubTrigger` (embedded in `Task`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubTrigger {
    pub event_type: GithubEventType,
    /// X-GitHub-Delivery header value (idempotency key)
    pub delivery_id: String,
    /// Deduplication key, e.g. "issue:47" or "pr:82"
    pub trigger_ref: String,
    /// "owner/repo"
    pub repo_full_name: String,
    /// Clone URL from repository.clone_url — used by executor to clone the repo
    pub repo_url: String,
    /// repository.default_branch — passed as base_branch to Sandbox::clone()
    pub default_branch: String,
    /// GitHub login of who triggered the mention
    pub actor_login: String,
    pub issue_or_pr_number: u64,
    pub comment_url: Option<String>,
    /// Full webhook JSON payload — passed verbatim to the agent as initial context
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GithubEventType {
    IssueComment,
    PullRequestReview,
    PullRequestReviewComment,
}
```

**Location**: `src/daemon/task.rs`

---

### `Task` extension

The existing `Task` struct gains one optional field:

```rust
pub struct Task {
    // ... all existing fields unchanged ...
    pub trigger: Option<GithubTrigger>,  // None for manually submitted API tasks
}
```

---

## Webhook Payload Structs (Internal — for deserialization only)

These are used inside `github_event.rs` for parsing. They are `pub(crate)` only — not part of the public API.

### `IssueCommentPayload`

```rust
#[derive(Deserialize)]
pub(crate) struct IssueCommentPayload {
    pub action: String,           // "created" | "edited" | "deleted"
    pub comment: CommentBody,
    pub issue: IssueRef,
    pub repository: RepoRef,
    pub sender: ActorRef,
}

#[derive(Deserialize)]
pub(crate) struct CommentBody {
    pub body: String,
    pub html_url: String,
}

#[derive(Deserialize)]
pub(crate) struct IssueRef {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub html_url: String,
    pub pull_request: Option<serde_json::Value>, // present if this is a PR comment
}

#[derive(Deserialize)]
pub(crate) struct RepoRef {
    pub full_name: String,     // "owner/repo"
    pub clone_url: String,
    pub default_branch: String,
}

#[derive(Deserialize)]
pub(crate) struct ActorRef {
    pub login: String,
}
```

### `PullRequestReviewPayload`

```rust
#[derive(Deserialize)]
pub(crate) struct PullRequestReviewPayload {
    pub action: String,         // "submitted" | "edited" | "dismissed"
    pub review: ReviewBody,
    pub pull_request: PrRef,
    pub repository: RepoRef,
    pub sender: ActorRef,
}

#[derive(Deserialize)]
pub(crate) struct ReviewBody {
    pub body: Option<String>,
    pub state: String,          // "APPROVED" | "CHANGES_REQUESTED" | "COMMENTED"
    pub html_url: String,
}

#[derive(Deserialize)]
pub(crate) struct PrRef {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub html_url: String,
    pub head: BranchRef,
    pub base: BranchRef,
}

#[derive(Deserialize)]
pub(crate) struct BranchRef {
    #[serde(rename = "ref")]
    pub branch: String,         // branch name
}
```

### `PullRequestReviewCommentPayload`

```rust
#[derive(Deserialize)]
pub(crate) struct PullRequestReviewCommentPayload {
    pub action: String,
    pub comment: ReviewCommentBody,
    pub pull_request: PrRef,
    pub repository: RepoRef,
    pub sender: ActorRef,
}

#[derive(Deserialize)]
pub(crate) struct ReviewCommentBody {
    pub body: String,
    pub html_url: String,
    pub path: Option<String>,   // file path the comment is on
}
```

---

## State Transitions

Webhook-triggered tasks follow the same state machine as manually submitted tasks:

```
Webhook received
      │
      ▼
[Signature verification]
      │ fail → 401, no task
      │ pass
      ▼
[Mention detection]
      │ no mention → 200 OK, no task
      │ mention found
      ▼
[Deduplication check]
      │ duplicate → 200 OK, log
      │ new
      ▼
  Task: Queued
      │
      ▼
  Task: Running
  (sandbox setup → gh auth check → agent execution)
      │
      ├──▶ Task: Completed
      └──▶ Task: Failed
```

---

## Validation Rules

| Field | Rule |
|-------|------|
| `webhook_secret` | Must be present in config when webhook endpoint is used |
| `mention_handle` | Default `"@hoosh"` if not configured; case-sensitive match |
| `action` | Only `"created"` (comments) or `"submitted"` (reviews) trigger tasks |
| `trigger_ref` | Used as deduplication key; must be unique per active task window |
| Branch name | Must match `^hoosh/[a-z]+-[0-9]+(-[a-z0-9]+)?$` |
