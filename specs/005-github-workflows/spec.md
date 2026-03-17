# Feature Spec: GitHub Workflow Triggers

**Feature**: `005-github-workflows`
**Branch**: `005-github-workflows`
**Date**: 2026-03-15
**Status**: Draft

---

## Overview

Extend the daemon with a GitHub webhook receiver that allows Hoosh to respond to GitHub events autonomously. When Hoosh is @mentioned in an issue or PR review, the daemon receives the webhook, clones the repository, then hands the raw event context to the agent. The agent uses the pre-installed `gh` CLI for all subsequent GitHub operations — including branch strategy, git commits, pushes, and PR creation.

This keeps the integration surface minimal: Hoosh does not reimplement GitHub's API. The `gh` CLI is the agent's primary interface for all GitHub interactions after sandbox setup.

---

## Goals

- Receive and verify GitHub webhook events
- Detect @hoosh mentions in issue comments and PR review comments
- Clone the repository before agent runs (clone only — agent handles branch strategy)
- Pass full event context to the agent as its initial instructions
- Let the agent drive all GitHub operations via `gh` CLI
- Deprecate the direct GitHub API usage in `PrProvider` (replaced by `gh` CLI)

## Non-Goals

- Webhooks for events not involving @hoosh mentions (CI status, labels, etc.)
- Implementing a GitHub App (Personal Access Token is sufficient)
- Re-implementing any GitHub API calls the `gh` CLI already handles
- Multi-repo fanout or webhook aggregation

---

## Prerequisites on the Target Machine

The machine running the daemon must have:

- `gh` CLI installed and authenticated (`gh auth status` succeeds)
- `git` installed and configured (user.name, user.email)
- Network access to GitHub

---

## Webhook Configuration

### Daemon Config Extension

```toml
[github]
webhook_secret = "..."    # Required for webhook verification. If unset, daemon logs a startup warning
                          # and the /github/webhook endpoint returns 500 until configured.
mention_handle = "@hoosh" # The @handle that triggers the agent. Defaults to "@hoosh" if omitted.
bot_login = "hoosh-bot"   # Optional: GitHub login of the bot account; events from this sender are ignored.
                          # If unset, daemon logs a startup warning and self-trigger protection is disabled.
```

The GitHub repo webhook must be configured to send events to `http://<daemon-host>/github/webhook`.

### Supported Events (GitHub webhook settings)

| GitHub Event Type | Enabled |
|-------------------|---------|
| `issue_comment`   | ✅ |
| `pull_request_review` | ✅ |
| `pull_request_review_comment` | ✅ |

---

## Triggering Rules

A webhook event triggers a task when **all** of the following are true:

1. Action is `created` (for comments) or `submitted` (for reviews)
2. Comment/review body contains the configured mention handle (e.g. `@hoosh`)
3. `sender.login` does not match `bot_login` in config (prevents self-trigger loops)
4. No existing task is already running for the same issue/PR (deduplication)
5. Webhook signature is valid

If a duplicate is detected, the daemon logs it and returns `200 OK` (do not retry).

---

## Sandbox Setup (Before Agent Starts)

The daemon clones the repository onto the default branch. That is all — the agent decides what branch to create or check out based on the event context it receives.

1. **Clone repository** using the repo URL from the webhook payload (default branch)

If the clone fails, the task is marked `Failed` immediately and no agent is started.

---

## Agent Context (Initial Instructions)

The agent receives a single user message containing:

1. A one-line framing sentence describing the event type
2. The raw webhook JSON payload (so the agent has full context without the daemon pre-processing it)
3. A closing operational hint pointing to `gh` CLI and git

### Example

```
You have been mentioned in a GitHub issue_comment event. The repository is already cloned at your working directory.

<event>
{ ...raw webhook JSON payload... }
</event>

Use `gh` CLI and git for all GitHub operations. Determine the appropriate branch strategy from the event context, make your changes, and push. Do not wait for further input.
```

The agent is responsible for reading the event, determining whether this is an issue (create a new branch) or a PR (check out the existing PR branch), and performing all subsequent git and GitHub operations.

---

## Use Cases

### UC-1: Issue Mention → Implement + Create PR

1. User files issue: "Add rate limiting"
2. User comments on issue: "@hoosh can you implement this?"
3. GitHub sends `issue_comment` webhook
4. Daemon verifies signature, detects @hoosh mention, clones repo to default branch
5. Agent receives event type + raw JSON payload
6. Agent creates a new branch (e.g. `hoosh/issue-47`), implements the feature, commits
7. Agent runs `gh pr create --title "..." --body "Closes #47" --base main`

### UC-2: PR Review → Address Feedback

1. Hoosh previously created a PR for some work
2. Human reviews the PR and @mentions hoosh with feedback
3. GitHub sends `pull_request_review` webhook
4. Daemon verifies, detects mention, clones repo to default branch
5. Agent receives event type + raw JSON payload
6. Agent reads the event, checks out the PR's existing head branch (`gh pr checkout 82`), addresses the feedback, commits, pushes
7. Agent replies to the review via `gh`

### UC-3: Any Other @mention Workflow

Any time `@hoosh` appears in a supported event, the agent is launched with the raw event context and full `gh` CLI / git access. The agent determines the appropriate action.

---

## HTTP API

### New Endpoint

```
POST /github/webhook
```

**Headers required by GitHub**:
- `X-GitHub-Event`: event type (`issue_comment`, `pull_request_review`, etc.)
- `X-Hub-Signature-256`: HMAC-SHA256 signature (`sha256=<hex>`)
- `X-GitHub-Delivery`: unique delivery GUID

**Response**:
- `202 Accepted`: event accepted and task queued
- `200 OK`: event received but no action taken (not a mention, duplicate, unsupported event)
- `401 Unauthorized`: invalid or missing signature
- `422 Unprocessable Entity`: payload parse error

### Deduplication

Before queuing, check if a `Running` or `Queued` task already exists with the same `trigger_ref` (e.g. `issue:47` or `pr:82`). `Completed` and `Failed` tasks do not block re-triggering. If a duplicate active task is found, return `200 OK` with `{"status": "duplicate", "existing_task_id": "..."}`.

---

## Task Model Extension

The existing `Task` struct gains:

```rust
pub struct Task {
    // ... existing fields ...
    pub trigger: Option<GithubTrigger>,  // present for webhook-triggered tasks
}

// Pseudocode — see tasks.md T003/T004 for full derive attributes
pub struct GithubTrigger {
    pub event_type: GithubEventType,     // IssueComment | PrReview | PrReviewComment
    pub delivery_id: String,             // X-GitHub-Delivery value
    pub trigger_ref: String,             // "issue:47" or "pr:82" for deduplication
    pub repo_full_name: String,          // "owner/repo"
    pub repo_url: String,                // clone URL from repository.clone_url
    pub default_branch: String,          // repository.default_branch — used for Sandbox::clone()
    pub actor_login: String,             // who mentioned @hoosh
    pub issue_or_pr_number: u64,
    pub comment_url: Option<String>,
    pub raw_payload: serde_json::Value,  // full webhook JSON, passed verbatim to agent
}

pub enum GithubEventType {
    IssueComment,
    PullRequestReview,
    PullRequestReviewComment,
}
```

---

## PrProvider Deprecation

The current `PrProvider` trait and `GitHubPrProvider` implementation made direct GitHub API calls to create PRs. With this feature, the agent uses `gh pr create` instead.

**Migration plan**:
- Keep `PrProvider` code in place initially (no immediate removal)
- For webhook-triggered tasks, skip `PrProvider` entirely (agent handles it via `gh` CLI)
- For manually-submitted API tasks (`POST /tasks`), keep `PrProvider` as fallback if configured
- Mark `PrProvider` as `#[deprecated]` after this feature ships
- Remove in a future cleanup feature

---

## Security

- Webhook signature MUST be verified using HMAC-SHA256 with constant-time comparison before any payload processing
- Raw request body must be used for signature verification (never re-serialized)
- `webhook_secret` is never logged
- The daemon must not expose `webhook_secret` via any API endpoint

---

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Invalid webhook signature | `401`, no task created |
| Unsupported event type | `200 OK`, no task |
| @hoosh not in body | `200 OK`, no task |
| Sender is bot account (`bot_login`) | `200 OK`, no task (silent) |
| `bot_login` not configured | Daemon logs startup warning; self-trigger protection disabled |
| Duplicate trigger (task already running) | `200 OK`, log + return |
| Sandbox clone failure | Task marked `Failed`, error logged |
| Agent errors | Task marked `Failed`, error logged |
| Missing `gh` CLI | Task marked `Failed` with actionable error message |

---

## Configuration Example

```toml
[daemon]
bind_address = "0.0.0.0:7979"
sandbox_base = "/tmp"
token_budget = 100000

[github]
webhook_secret = "super-secret"
mention_handle = "@hoosh"
bot_login = "hoosh-bot"
```

---

## Clarifications

### Session 2026-03-15

- Q: For PR review triggers, should the agent push to the existing PR head branch or a new hoosh/ branch? → A: The agent decides — it reads the raw event payload to determine the branch, then uses `gh pr checkout` or `git checkout` as appropriate.
- Q: Should the daemon filter events sent by the bot itself to prevent infinite loops? → A: Yes — add `bot_login` to config; daemon skips events where `sender.login == bot_login`. Confirmed: `sender.login` is a standard top-level field present in all three supported GitHub webhook payload types.
- Q: Should Completed/Failed tasks block re-triggering on the same issue/PR? → A: No — only Queued/Running tasks trigger deduplication; Completed and Failed allow a fresh task.
- Q: When `issue_comment` fires on a PR thread (`issue.pull_request` present), treat as issue or PR? → A: The daemon doesn't distinguish — it passes the raw payload to the agent. The agent reads `issue.pull_request` from the JSON and acts accordingly.
- Q: Should missing `webhook_secret` warn at daemon startup? → A: Yes — log a startup warning (consistent with `bot_login`); runtime 500 still applies when endpoint is hit.
- Q: Is `bot_login` required or optional? → A: Optional — daemon starts and logs a startup warning if unset; self-trigger protection is simply disabled.
- Q: Does the daemon perform any post-agent git operations (commit, push, PR creation)? → A: No — for webhook-triggered tasks the daemon skips all post-agent git operations. The agent is responsible for all git and GitHub work via bash/git/`gh` CLI.

---

## Out of Scope (Future Work)

- GitHub App authentication (vs PAT)
- Handling `push` events for CI-like triggers
- Rate limiting webhook delivery retries
- Multi-tenant (per-repo secrets)
