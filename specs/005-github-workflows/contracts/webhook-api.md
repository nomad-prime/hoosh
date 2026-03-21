# Contract: GitHub Webhook API

**Feature**: `005-github-workflows` | **Date**: 2026-03-15

---

## POST /github/webhook

Receives GitHub webhook events. Verifies signature, detects @hoosh mentions, and dispatches tasks.

### Request

**Headers**:

| Header | Required | Description |
|--------|----------|-------------|
| `X-GitHub-Event` | ✅ | Event type: `issue_comment`, `pull_request_review`, `pull_request_review_comment` |
| `X-Hub-Signature-256` | ✅ | `sha256=<hmac-hex>` — HMAC-SHA256 of raw body using webhook secret |
| `X-GitHub-Delivery` | ✅ | GitHub-generated unique delivery UUID |
| `Content-Type` | ✅ | `application/json` |

**Body**: Raw GitHub webhook JSON payload (see GitHub docs for schema).

---

### Responses

#### 202 Accepted — Task queued

```json
{
  "status": "accepted",
  "task_id": "hoosh-550e8400-e29b-41d4-a716-446655440000"
}
```

#### 200 OK — No action taken

```json
{
  "status": "no_action",
  "reason": "no_mention"
}
```

Possible `reason` values:
- `"no_mention"` — @hoosh not found in comment/review body
- `"unsupported_event"` — event type not handled
- `"unsupported_action"` — action is not `created` or `submitted`
- `"duplicate"` — a task is already running/queued for this issue/PR

For duplicates, additionally:
```json
{
  "status": "no_action",
  "reason": "duplicate",
  "existing_task_id": "hoosh-550e8400-e29b-41d4-a716-446655440000"
}
```

#### 401 Unauthorized — Signature invalid or missing

```json
{
  "error": "invalid_signature"
}
```

#### 422 Unprocessable Entity — Payload parse error

```json
{
  "error": "invalid_payload",
  "detail": "missing field `repository`"
}
```

#### 500 Internal Server Error — Misconfiguration

```json
{
  "error": "not_configured",
  "detail": "github.webhook_secret is not set in daemon config"
}
```

---

## Existing Endpoints (Unchanged)

All existing daemon endpoints remain unchanged:

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/tasks` | Submit task manually |
| `GET` | `/tasks` | List all tasks |
| `GET` | `/tasks/:id` | Get task details (now includes `trigger` field) |
| `DELETE` | `/tasks/:id` | Cancel task |
| `GET` | `/tasks/:id/logs` | Stream logs |
| `GET` | `/health` | Health check |

### `GET /tasks/:id` — Updated Response Shape

The `trigger` field is added (null for manually submitted tasks):

```json
{
  "id": "hoosh-...",
  "status": "completed",
  "repo_url": "https://github.com/acme/backend.git",
  "base_branch": "main",
  "instructions": "...",
  "trigger": {
    "event_type": "issue_comment",
    "delivery_id": "abc-123",
    "trigger_ref": "issue:47",
    "repo_full_name": "acme/backend",
    "actor_login": "alice",
    "issue_or_pr_number": 47,
    "comment_url": "https://github.com/acme/backend/issues/47#issuecomment-99999"
  },
  "branch_name": "hoosh/issue-47",
  "pr_url": "https://github.com/acme/backend/pull/53",
  "created_at": "2026-03-15T10:00:00Z",
  "started_at": "2026-03-15T10:00:01Z",
  "completed_at": "2026-03-15T10:05:30Z"
}
```
