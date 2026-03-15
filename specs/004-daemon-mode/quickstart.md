# Quickstart: Daemon Mode

**Branch**: `004-daemon-mode` | **Date**: 2026-03-14

---

## Prerequisites

1. **SSH key configured** for git operations (clone/push to target repos)
2. **GitHub PAT** with `repo` scope (for PR creation)
3. `hoosh` built from this branch (`cargo build --release`)

---

## Configuration

Add a `[daemon]` section to `~/.hoosh/config.toml`:

```toml
[daemon]
github_pat = "ghp_xxxxxxxxxxxxxxxxxxxx"
default_token_budget = 100000
# bind_address = "127.0.0.1:7979"   # default
# retain_sandboxes = false           # default
```

---

## Start the Daemon

`hoosh daemon start` always runs in the foreground. Use your OS service manager for lifecycle management.

### systemd (Linux)

```ini
# /etc/systemd/system/hoosh-daemon.service
[Unit]
Description=Hoosh Autonomous Coding Daemon
After=network.target

[Service]
Type=simple
User=hoosh
ExecStart=/usr/local/bin/hoosh daemon start --port 7979
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

```bash
systemctl enable hoosh-daemon
systemctl start hoosh-daemon
systemctl status hoosh-daemon
journalctl -u hoosh-daemon -f
```

### launchd (macOS)

```bash
# Run directly (e.g. in a tmux session or via launchd plist)
hoosh daemon start --port 7979
```

### Manual / debugging

```bash
hoosh daemon start
# Verify it's accepting requests
hoosh daemon status
# → daemon is running (pid 12345, listening on 127.0.0.1:7979)
```

> The daemon binds to localhost by default. For external access put a reverse proxy (nginx, Caddy) with auth in front — do not expose the task API to the internet bare.

---

## Submit a Task

```bash
# Via CLI
hoosh daemon submit \
  --repo "git@github.com:myorg/myrepo.git" \
  --branch main \
  --instructions "Add input validation to the login form and write tests for it"

# Output: hoosh-550e8400-e29b-41d4-a716-446655440000

# Or directly via HTTP
curl -s -X POST http://127.0.0.1:7979/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "repo_url": "git@github.com:myorg/myrepo.git",
    "base_branch": "main",
    "instructions": "Add input validation to the login form and write tests for it"
  }' | jq .
```

---

## Monitor Tasks

```bash
# List all tasks
curl -s http://127.0.0.1:7979/tasks | jq .

# Poll a specific task
TASK_ID="hoosh-550e8400-e29b-41d4-a716-446655440000"
curl -s http://127.0.0.1:7979/tasks/$TASK_ID | jq .status

# Watch logs in real time (log file is plain text)
# Note: log path is returned in the task detail response
curl -s http://127.0.0.1:7979/tasks/$TASK_ID | jq .sandbox_path
# then: tail -f /tmp/hoosh-550e8400-.../execution.log

# Or via the log endpoint
curl -s http://127.0.0.1:7979/tasks/$TASK_ID/logs
```

---

## Cancel a Task

```bash
TASK_ID="hoosh-550e8400-e29b-41d4-a716-446655440000"
curl -s -X DELETE http://127.0.0.1:7979/tasks/$TASK_ID
# → 204 No Content (partial changes will be committed with [incomplete] marker)
```

---

## Stop the Daemon

```bash
# Graceful: stop accepting new tasks, wait for running tasks to finish
hoosh daemon stop

# Immediate: cancel all running tasks and exit
hoosh daemon stop --force
```

---

## Permission Configuration

Global baseline (applies to all tasks):
```json
// ~/.hoosh/permissions.json
{
  "version": 1,
  "allow": [
    { "operation": "read_file", "pattern": "*" },
    { "operation": "list_directory", "pattern": "*" },
    { "operation": "write_file", "pattern": "src/**" },
    { "operation": "bash", "pattern": "cargo:*" }
  ],
  "deny": [
    { "operation": "bash", "pattern": "rm -rf:*", "reason": "No destructive commands" }
  ]
}
```

Repo-level overrides (checked into `<repo>/.hoosh/permissions.json`):
```json
{
  "version": 1,
  "allow": [
    { "operation": "write_file", "pattern": "tests/**" }
  ],
  "deny": []
}
```

---

## Crash Recovery

If the daemon process is killed unexpectedly (OOM, SIGKILL), on the next start it will:
1. Scan `~/.hoosh/daemon/tasks/` for any task with status `running`
2. Mark each such task as `failed` with message `[incomplete] daemon restarted unexpectedly`
3. Begin accepting new tasks normally

No partial git state is recovered — the operator should manually inspect or discard the sandbox directory.
