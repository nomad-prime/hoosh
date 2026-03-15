# API Contract: Daemon REST API

**Base URL**: `http://127.0.0.1:7979` (default; configurable in `[daemon]` config section)

All request and response bodies are `application/json`.

---

## Endpoints

### POST /tasks — Submit a task

**Request body**:
```json
{
  "repo_url": "git@github.com:org/repo.git",   // required
  "base_branch": "main",                        // required
  "instructions": "Add unit tests for...",      // required
  "pr_title": "Add unit tests",                 // optional
  "pr_labels": ["automated", "testing"],        // optional, default []
  "token_budget": 50000                         // optional, overrides global default
}
```

**Response `202 Accepted`**:
```json
{
  "task_id": "hoosh-550e8400-e29b-41d4-a716-446655440000"
}
```

**Error responses**:
- `400 Bad Request` — missing required fields or invalid values
- `503 Service Unavailable` — daemon is shutting down (not accepting new tasks)

---

### GET /tasks — List all tasks

**Response `200 OK`**:
```json
[
  {
    "id": "hoosh-550e8400-...",
    "repo_url": "git@github.com:org/repo.git",
    "base_branch": "main",
    "status": "running",
    "created_at": "2026-03-14T10:00:00Z",
    "started_at": "2026-03-14T10:00:01Z",
    "completed_at": null,
    "tokens_consumed": 12450,
    "pr_url": null,
    "branch": "hoosh/550e8400-e29b-41d4-a716-446655440000",
    "error_message": null
  }
]
```

Status values: `"queued"`, `"running"`, `"completed"`, `"failed"`, `"cancelled"`

---

### GET /tasks/{id} — Get task by ID

**Response `200 OK`**: Same shape as individual task object in list above, plus full fields:
```json
{
  "id": "hoosh-550e8400-...",
  "repo_url": "...",
  "base_branch": "main",
  "instructions": "Add unit tests for...",
  "pr_title": null,
  "pr_labels": [],
  "token_budget": null,
  "status": "completed",
  "created_at": "2026-03-14T10:00:00Z",
  "started_at": "2026-03-14T10:00:01Z",
  "completed_at": "2026-03-14T10:03:45Z",
  "tokens_consumed": 34200,
  "pr_url": "https://github.com/org/repo/pull/42",
  "branch": "hoosh/550e8400-...",
  "error_message": null,
  "sandbox_path": "/tmp/hoosh-550e8400-..."
}
```

**Error responses**:
- `404 Not Found` — unknown task ID

---

### DELETE /tasks/{id} — Cancel a task

Cancels a task in `queued` or `running` state. Running tasks receive a cancellation signal; any partial changes are committed with an `[incomplete]` marker.

**Response `204 No Content`** — cancellation accepted (task may still be transitioning)

**Error responses**:
- `404 Not Found` — unknown task ID
- `409 Conflict` — task is already in a terminal state (`completed`, `failed`, `cancelled`)

---

### GET /tasks/{id}/logs — Get execution log

Returns the raw execution log for the task (stdout/stderr of agent run, git operations, errors).

**Response `200 OK`**:
```
Content-Type: text/plain; charset=utf-8

[2026-03-14T10:00:01Z] Cloning git@github.com:org/repo.git into /tmp/hoosh-550e8400-...
[2026-03-14T10:00:03Z] Branch created: hoosh/550e8400-e29b-41d4-a716-446655440000
[2026-03-14T10:00:03Z] Agent started
...
```

**Error responses**:
- `404 Not Found` — unknown task ID or log file not yet created

---

### GET /health — Health check

**Response `200 OK`**:
```json
{
  "status": "ok",
  "version": "0.4.6",
  "uptime_seconds": 3661,
  "active_tasks": 2,
  "shutting_down": false
}
```

---

## CLI Contract

```
hoosh daemon start [--port <PORT>] [--foreground]
hoosh daemon stop [--force]
hoosh daemon status

hoosh daemon submit
  --repo <URL>
  --branch <BRANCH>
  --instructions <TEXT>
  [--pr-title <TITLE>]
  [--label <LABEL>]...
  [--token-budget <N>]
```

`hoosh daemon submit` prints the task ID to stdout on success, making it scriptable:
```
TASK_ID=$(hoosh daemon submit --repo git@... --branch main --instructions "...")
```

---

## Error Response Format

All error responses use a consistent body:
```json
{
  "error": "description of what went wrong"
}
```
