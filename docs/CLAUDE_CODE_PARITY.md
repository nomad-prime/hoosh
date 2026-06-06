# Claude Code parity roadmap

Plan for closing the gap between hoosh and Claude Code's CLI/session model so the [peyk](https://github.com/nomad-prime/peyk) bridge can swap `claude -p` → `hoosh --mode tagged` with minimal changes when the Claude Max subscription expires in September 2026.

Authored 2026-06-06 from a comparative investigation. Not a release plan — a task list ordered by impact.

---

## Context

[peyk](https://github.com/nomad-prime/peyk) is a courier service running on hooshi that accepts prompts from a phone, dispatches them to a coding agent, and pushes responses back as notifications. Today it uses Claude Code via:

```bash
output=$(claude --resume "$session_id" -p "$text" --output-format json)
session_id=$(echo "$output" | jq -r '.session_id')
result=$(echo "$output" | jq -r '.result')
```

The September goal is to swap to hoosh as the agent backend. After investigating both agents' CLI surfaces, the architectural gap is **smaller than initially feared** — hoosh's `tagged mode` + `.hoosh/conversations/` already maps cleanly onto Claude's `claude -p` + `~/.claude/projects/` model. The remaining gaps are documented below.

---

## What hoosh already has (with file pointers)

| Capability | Where in code | Notes |
|---|---|---|
| Tagged mode (analog of `claude -p`) | `src/tagged_mode.rs` | `hoosh --mode tagged "text"` — one-shot, prints markdown to stdout, no TUI |
| `--continue` resume most recent | `src/cli/mod.rs`, agent.rs:106-118 | Loads latest conversation from CWD's `.hoosh/conversations/` |
| Per-CWD conversation storage | `.hoosh/conversations/conv_YYYYMMDD_HHMMSS/` | Colocated, one folder per conversation, JSONL messages + meta.json |
| Privacy-first default | `src/session.rs:181` (`conversation_storage` config) | **Defaults to `false`** — conversations are ephemeral unless explicitly enabled. Confirmed behaviour. |
| Conversation listing | `hoosh conversations list` | Already exists |
| Daemon mode | `POST /jobs` | Stateless transactional, separate concern; not the target for bridge swap |
| `@hoosh` tagged-mode alias | `src/cli/shell_setup.rs` | Per-shell-PID session persistence via `HOOSH_TERMINAL_PID` — orthogonal to CWD-conversation model |

Critical to remember: **conversation storage is off by default in hoosh.** Users opt in via `conversation_storage = true` in `.hoosh/config.toml` (project) or `~/.config/hoosh/config.toml` (global). All the work below assumes storage is enabled when relevant; the off-by-default behaviour should be preserved.

---

## Gap analysis vs Claude Code

### Tier 1 — blocks the peyk bridge swap

Without these, the bridge needs ugly workarounds when swapping `claude` for `hoosh`.

#### 1.1 `--output-format json` for tagged mode

**What:** When `--output-format json` is set, tagged mode should print a single JSON object to stdout instead of streaming markdown to the terminal.

**Why:** Bridge reliably extracts `{result, session_id}`. Without it, the bridge must `tail -c 4000` the markdown output and has no way to get the conversation/session id for explicit resume.

**Acceptance:**
- `hoosh --mode tagged --output-format json "hello"` prints **only** valid JSON on stdout
- JSON has at minimum: `{"result": "<markdown response>", "session_id": "<id-or-null>", "model": "<which-backend>", "input_tokens": N, "output_tokens": M}`
- Spinners and progress UI go to stderr (or are suppressed) when JSON mode is on
- `session_id` is `null` when `conversation_storage = false` (privacy mode) — the bridge can detect this and switch to non-resume flow
- Works with `--continue` and (forthcoming) `--resume <id>` flags

**Implementation hints:**
- `src/tagged_mode.rs` currently calls `console().markdown(&response_content)` and emits events to stdout. Gate these behind an `OutputFormat` enum read from CLI.
- The `Conversation` type already carries an id (the `conv_YYYYMMDD_HHMMSS` string). Surface it for serialization.
- Token usage is already tracked by the agent — pass through.

**Effort:** ~2 hrs

---

#### 1.2 `--resume <id>` flag for tagged mode

**What:** `hoosh --mode tagged --resume <conv_id> "text"` resumes a specific conversation by id (not just "most recent").

**Why:** Bridge stores a session id and pins the conversation thread across invocations. `--continue` (most recent) is fragile if any other process touches the conversation store.

**Acceptance:**
- `hoosh --mode tagged --resume conv_20260606_193112 "text"` loads that specific conversation and appends the new turn
- If the id doesn't exist in the current CWD's store: exit non-zero with a clear error message (mirror Claude's `No conversation found with session ID: <id>` style)
- Compatible with `--output-format json` (returns the same conv_id in the output)
- When `conversation_storage = false`: hard error ("storage is disabled, cannot resume")

**Implementation hints:**
- Mirror the existing `--continue` flow in `src/agent.rs:106-118` (`ConversationStorage::load_latest`) — add a `load_by_id` variant or wire to an existing one.
- Update CLI parser in `src/cli/mod.rs` to add `#[arg(long, value_name = "ID")] pub resume: Option<String>`.

**Effort:** ~2-3 hrs

**Combined effort for Tier 1: ~half a day.** After this, peyk's `claude-bridge.sh` swap is genuinely 5 lines different.

---

### Tier 2 — interactive parity (when humans SSH in)

Won't affect the headless bridge but matters when you attach to hoosh on hooshi to debug or take over a conversation.

#### 2.1 Named sessions

**What:**
- `hoosh -n <name>` (or `--name`) at startup gives the new conversation a human-readable name
- `/rename <name>` mid-session changes the name
- `hoosh --resume <name>` resolves by name (in addition to id)

**Why:**
- The "per-repo named conversation" pattern (`hoosh -n peyk-thread` invoked from `/var/lib/claude/projects/peyk`) becomes natural
- Conversations list shows readable names instead of timestamps
- Picker (2.2 below) is useless without names

**Acceptance:**
- `name: Option<String>` field on `Conversation` meta.json; nullable for backward compat
- Name uniqueness: per-CWD scope. Resolution: exact match → resume, ambiguous → error
- `hoosh conversations list` shows `[name]` when set, falls back to id
- `/rename` slash command in TUI and tagged interactive flows

**Implementation hints:**
- Persistent slot lives in `meta.json` (under `.hoosh/conversations/<id>/`). Existing structure already has room for extension.
- Resolution logic: search current CWD's conversation dirs by name; for centralized storage (2.3) include the centralized location.

**Effort:** ~3-4 hrs

---

#### 2.2 Session picker

**What:** `hoosh --resume` (with no argument) opens an interactive ratatui picker showing conversations sorted by recency, with search filter, preview pane, and selection. Inspired by Claude's `/resume` picker.

**Why:** Without it, "show me my conversations" is `hoosh conversations list` + manual `--continue` or `--resume <id>` typing. Picker UI is a real productivity feature in Claude.

**Acceptance:**
- Arrow keys to navigate
- `/` enters search mode, filters by name or first prompt
- `Space` previews highlighted session content
- `Enter` resumes
- `Esc` exits
- Defaults to current CWD's conversations; `Ctrl+A` widens to all CWDs on this machine (requires the `conversations list` view to know about all stores)
- Empty state when no conversations: helpful message instead of blank screen

**Implementation hints:**
- ratatui is already a dependency (TUI mode uses it). Reuse layout patterns from existing TUI.
- Read all `.hoosh/conversations/*/meta.json` in the current CWD; merge with the centralized store if configured.

**Effort:** ~1-2 days

---

#### 2.3 Storage tri-mode (replaces `conversation_storage` boolean)

**What:** Extend the existing `conversation_storage` config knob from `bool` to a three-mode string:

```toml
# .hoosh/config.toml or ~/.config/hoosh/config.toml
conversation_storage = "off" | "local" | "central"
```

- `"off"` — equivalent to current `false`: ephemeral, no writes, no resume
- `"local"` — equivalent to current `true`: write to `.hoosh/conversations/` in CWD (current behavior)
- `"central"` — write to `~/.local/share/hoosh/projects/<encoded-cwd>/` (new). Matches Claude Code's `~/.claude/projects/` model. Encoded path = absolute CWD with `/` → `-`, similar to Claude's scheme.

**Why both local and central matter:**
- Local: some users (and use cases) genuinely want conversation history committed to the repo as an artifact. AI-pair-programming workflow, audit trails, team knowledge. Tools like aider do this deliberately.
- Central: most users don't want repo pollution and risk of leaking conversation content in a public commit. Centralized makes that the default-safe path.
- Off: existing privacy-first default; preserve this.

**Backwards compat:**
- Accept `true` (parses as `"local"`) and `false` (parses as `"off"`) for at least one minor release
- New default for fresh installs: `"off"` (unchanged)
- Document the migration in CHANGELOG

**Auto-gitignore for `"local"` mode in git repos:**
- On first conversation save in `"local"` mode, if `.git/` exists and `.gitignore` doesn't already cover `.hoosh/conversations/`:
  - Append two lines:
    ```
    # hoosh conversations (added automatically). Remove this line if you want to commit conversation history.
    .hoosh/conversations/
    .hoosh/memory/
    ```
- Idempotent: don't append if a matching line already present
- Don't `git add` the change — that's the user's call
- Skip silently in non-git directories

**Acceptance:**
- Config parses all three string values + the legacy booleans
- Selecting `central` mode writes to `~/.local/share/hoosh/projects/<encoded-cwd>/conv_*/`
- `--continue` / `--resume <id|name>` look in the right place based on configured mode
- Conversations list shows entries from whichever mode is active
- gitignore lines appear after first save in local mode, only once

**Implementation hints:**
- Config: change `conversation_storage: Option<bool>` → `Option<ConversationStorageMode>` with serde untagged enum or custom deserializer for backwards compat (`src/config/mod.rs`).
- Storage path resolution: extract a `resolve_storage_root(config, cwd) -> Option<PathBuf>` helper; `None` means off.
- gitignore writer: small utility, idempotent. Trigger on first `ConversationStorage::save_new`.

**Effort:** ~half a day (~4 hrs including the gitignore utility and tests)

---

#### 2.4 Session branching

**What:** `/branch <name>` (inside an active session) or `hoosh --continue --fork-session` (from CLI) creates a copy of the current conversation and switches to it, leaving the original intact.

**Why:** Real productivity feature in Claude. Lets you say "wait, what if we tried it differently" without losing the original thread. Maps cleanly onto how developers iterate on ideas.

**Acceptance:**
- New conversation gets a `parent_conv_id` field in meta.json
- Messages up to the branch point are copied (not moved)
- Original conversation continues to exist
- Picker (2.2) groups branches under the parent (collapsible)

**Implementation hints:**
- `ConversationStorage` already does deep file copy via JSONL append — branch is a `cp -r conv_old conv_new` plus meta tweak.

**Effort:** ~4-6 hrs

---

### Tier 3 — nice-to-have, low urgency

Not blocking anything; reference for the future.

| Feature | Notes |
|---|---|
| `--from-pr <number>` | Resume the session linked to a PR. Requires storing PR association in meta.json on PR-creating jobs (daemon mode mostly). |
| `--no-session-persistence` flag | Per-invocation override of `conversation_storage` to off. Useful when scripting one-shots that shouldn't pollute the store. |
| Picker `Ctrl+W` worktree widening | Show sessions from all worktrees of the current repo. Lower priority; assumes worktree support. |
| Picker `Ctrl+A` cross-project widening | Show sessions from all CWDs. Requires central-mode index OR scanning known paths. |
| Auto-name on plan accept | When accepting a plan in plan mode, derive a session name from the plan title. Plan mode is a separate Claude feature; only relevant if hoosh adds plan mode. |
| `/export` to clipboard or file | Polish. Existing JSONL is already greppable; bigger win would be a markdown render. |
| Retention sweep (`cleanupPeriodDays`) | Claude prunes sessions older than 30 days by default. Worth replicating once volume grows. |

---

## Recommended order of attack

1. **Tier 1 first (~half day)** — `--output-format json` + `--resume <id>`. Unblocks the peyk bridge swap. Without these, the September migration needs script gymnastics.
2. **Named sessions (Tier 2.1, ~3-4 hrs)** — pairs with `--resume` so the bridge can use `hoosh --resume peyk-thread` instead of opaque UUIDs.
3. **Storage tri-mode + auto-gitignore (Tier 2.3, ~4 hrs)** — should be done before encouraging users to enable storage, to avoid the leak footgun.
4. **Session picker (Tier 2.2, ~1-2 days)** — biggest interactive UX win. After Tier 1+1.1 unblocks bridge, focus shifts here.
5. **Branching (Tier 2.4, ~4-6 hrs)** — quality-of-life, not urgent.
6. **Tier 3 items** — opportunistically when each becomes annoying enough.

---

## Verification: peyk bridge swap test

When Tier 1 lands, the bridge swap should work like this. Add to `peyk/scripts/claude-bridge.sh` a `BRIDGE_AGENT` env var:

```bash
case "$BRIDGE_AGENT" in
  claude)
    bin="${CLAUDE_BIN:-/var/lib/claude/.local/bin/claude}"
    output=$("$bin" --resume "$session_id" -p "$text" --output-format json)
    ;;
  hoosh)
    bin="${HOOSH_BIN:-/usr/local/bin/hoosh}"
    output=$("$bin" --mode tagged --resume "$session_id" --output-format json "$text")
    ;;
esac

result=$(echo "$output" | jq -r '.result')
new_sid=$(echo "$output" | jq -r '.session_id')
```

The two branches should produce indistinguishable behavior for the bridge — same input, same output shape, same persistence semantics. That's the parity test.

When this passes, the September migration is a one-line env-var change in peyk's systemd unit:

```ini
Environment=BRIDGE_AGENT=hoosh
```

Anything that doesn't fit through this contract is parity work that hasn't landed yet.

---

## Out of scope (deliberately)

- **Hoosh daemon mode improvements** — daemon is the analog of "agentic PR runner" not "interactive CLI." Lives in a different design space, served by different needs. The bridge swap doesn't touch it.
- **MCP server integration** — orthogonal feature, useful but not on the parity path.
- **Multi-model routing** — hoosh already has it (`config.toml` backend selection); parity isn't the gap.
- **Plan mode** — Claude-specific UX, not core to bridge parity. Add later if it matches hoosh's roadmap.

---

## See also

- [peyk's claude-bridge](https://github.com/nomad-prime/peyk/blob/main/scripts/claude-bridge.sh) — the script that this work unblocks
- [bana's claude user setup](https://github.com/nomad-prime/bana/blob/main/hosts/hooshi/setup-claude-user.sh) — provisions the `claude` user; an analogous `hoosh` user already exists from hoosh's own server-setup
- Claude Code session docs: https://code.claude.com/docs/en/sessions
