# Quickstart: GitHub Workflow Triggers

**Feature**: `005-github-workflows` | **Date**: 2026-03-15

---

## Prerequisites

1. **`gh` CLI installed and authenticated** on the daemon machine:
   ```bash
   gh auth status   # must succeed
   ```

2. **Daemon running** with a publicly accessible URL (or ngrok for local dev):
   ```bash
   hoosh daemon start
   ```

3. **GitHub repo webhook configured**:
   - URL: `https://<your-daemon-host>:7979/github/webhook`
   - Content type: `application/json`
   - Secret: a random string you'll also put in daemon config
   - Events: `Issue comments`, `Pull request reviews`, `Pull request review comments`

---

## Daemon Configuration

Add to your `~/.hoosh/config.toml`:

```toml
[daemon]
bind_address = "0.0.0.0:7979"
sandbox_base = "/tmp"
token_budget = 100000

[github]
webhook_secret = "your-webhook-secret-here"
mention_handle = "@hoosh"   # optional, this is the default
```

---

## Testing Locally with ngrok

```bash
# Start daemon
hoosh daemon start

# In another terminal, expose it
ngrok http 7979

# Use the ngrok URL as your GitHub webhook URL
# e.g. https://abc123.ngrok.io/github/webhook
```

---

## Triggering a Task

### Issue mention

1. Open any issue in your configured repo
2. Comment: `@hoosh please implement this`
3. Daemon receives the webhook and queues a task

### PR review mention

1. Open a pull request
2. Submit a review with: `@hoosh can you fix the timeout issue?`
3. Daemon receives the webhook and queues a task

---

## Monitoring

```bash
# Watch task list
hoosh daemon list

# Follow logs for a specific task
hoosh daemon logs hoosh-<task-id>

# Check daemon health
curl http://localhost:7979/health
```

---

## What the Agent Does

The agent is given full context from the webhook event and has access to:

- `bash` tool (to run `gh`, `git`, and any other commands)
- File read/write/edit tools (for code changes)
- All tools available in normal daemon task execution

The agent will typically:
1. Read the issue/PR context it was given
2. Explore the repo with `ls`, `grep`, file reads
3. Make changes
4. Run tests with `cargo test` or appropriate test command
5. Commit and push
6. Create a PR with `gh pr create` (for issue tasks) or push amendments (for review tasks)
7. Reply to the original comment/review to close the loop

---

## Troubleshooting

| Problem | Fix |
|---------|-----|
| Webhook returns 401 | Check `webhook_secret` matches GitHub repo setting |
| Webhook returns 500 with "not_configured" | Add `[github]` section to config.toml |
| Task created but immediately fails | Run `gh auth status` on the daemon machine |
| Agent can't push to branch | Ensure `gh` CLI has write access to the repo |
| No task created for mention | Check that `mention_handle` matches exactly (case-sensitive) |
