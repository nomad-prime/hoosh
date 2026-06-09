# Claude Code parity roadmap

Plan for closing the gap between hoosh and Claude Code's CLI/session model so the [peyk](https://github.com/nomad-prime/peyk) bridge can swap `claude -p` → `hoosh --mode tagged` with minimal changes when the Claude Max subscription expires in September 2026.

Authored 2026-06-06. Updated 2026-06-07 after Tier 1 + 2.1 + 2.3 landed.

---

## Status snapshot

| Tier | Item | Status | Where |
|---|---|---|---|
| 1.1 | `--output-format json` for tagged mode | ✅ done | `717ae7d` — `src/output_format.rs`, `src/tagged_mode.rs` |
| 1.2 | `--resume <id\|name>` flag | ✅ done | `717ae7d` — `--resume` accepts id or name, `conflicts_with` `--continue` |
| 2.1 | Named sessions (`-n`/`--name`, `/rename`) | ✅ done | `b5ed673` + `9e05129` — `src/commands/rename_command.rs`, name persisted on Conversation |
| 2.3 | Storage tri-mode + auto-gitignore | ✅ done | `b5ed673` — `src/storage/mode.rs` (260 lines, with tests) |
| 3.1 | `--no-session-persistence` per-invocation override | ✅ done | `src/cli/agent.rs` — flips `conversation_storage` → `Off` for the invocation; mutually exclusive with `--resume`/`--continue`/`--name`; JSON output yields `session_id: null` |
| 2.2 | Session picker (interactive TUI) | ❌ explicitly deferred | named sessions cover most discovery; `hoosh conversations list \| fzf` is one pipe away |
| 2.4 | Session branching (`/branch`, `--fork-session`) | ❌ explicitly deferred | power-user feature; revisit if hoosh ships plan mode |
| 3.x | `--from-pr <n>`, picker key-binds, plan-mode auto-name, `/export`, retention sweep | future | not on the parity-for-September path |

**Result:** the bridge swap from `claude -p` to `hoosh --mode tagged` is now mechanically possible. See "Verification" at the bottom for the test script.

---

## Context

[peyk](https://github.com/nomad-prime/peyk) is a courier service running on hooshi that accepts prompts from a phone, dispatches them to a coding agent, and pushes responses back as notifications. Today it uses Claude Code via:

```bash
output=$(claude --resume "$session_id" -p "$text" --output-format json)
session_id=$(echo "$output" | jq -r '.session_id')
result=$(echo "$output" | jq -r '.result')
```

The September goal is to swap to hoosh as the agent backend. After investigating both agents' CLI surfaces, the architectural gap turned out to be smaller than initially feared — hoosh's `tagged mode` + `.hoosh/conversations/` already mapped cleanly onto Claude's `claude -p` + `~/.claude/projects/` model. The remaining gaps were:

1. Structured output (so the bridge can parse `{result, session_id}`)
2. Explicit resume by id/name (so the bridge can pin a thread)
3. Named sessions + storage choice (so the per-repo workflow has a clean home)

All three landed (above). What's left is polish.

---

## What hoosh has (relevant to parity)

| Capability | Where in code | Notes |
|---|---|---|
| Tagged mode (analog of `claude -p`) | `src/tagged_mode.rs` | `hoosh --mode tagged "text"` — one-shot, prints markdown to stdout |
| `--output-format json` | `src/output_format.rs`, `--output-format text\|json` | JSON form returns `{result, session_id, model, input_tokens, output_tokens}` with stderr-routed progress |
| `--continue` | `src/cli/mod.rs`, agent.rs | Loads latest conversation from CWD's store |
| `--resume <id\|name>` | `src/cli/mod.rs` | Resolves by id or human-readable name in current CWD's store |
| `-n / --name` and `/rename` | `src/commands/rename_command.rs` | Per-CWD-scoped unique names |
| Storage tri-mode | `src/storage/mode.rs` (`ConversationStorageMode::{Off,Local,Central}`) | Configured via `conversation_storage` in `.hoosh/config.toml` or `~/.config/hoosh/config.toml`. Accepts legacy `true`/`false` (→ `local`/`off`) for backwards compat. |
| Auto-gitignore for `local` mode in git repos | `src/storage/mode.rs::ensure_local_storage_gitignored` | Appends `.hoosh/conversations/` + `.hoosh/memory/` with a comment line explaining how to opt back in |
| Privacy-first default | `src/session.rs:181` (`conversation_storage` default) | Defaults to `Off` — conversations are ephemeral unless explicitly enabled. **Preserved through tri-mode migration.** |
| Daemon mode | `POST /jobs` | Stateless transactional, separate concern; not the bridge swap target |
| `@hoosh` tagged-mode alias | `src/cli/shell_setup.rs` | Per-shell-PID session persistence, orthogonal to CWD-conversation model |

---

## Tier 3.1 — done

`--no-session-persistence` lands in `src/cli/agent.rs`: when set, it overrides the resolved config's `conversation_storage` to `Off` for the duration of that invocation. Mutually exclusive with `--resume`, `--continue`, and `--name` (each errors with a clear message rather than silently no-op'ing). JSON output already routes `session_id: null` when storage is disabled, so the new flag composes with `--output-format json` for free. Default behavior is unchanged when the flag is absent.

---

## Explicitly deferred (not on the parity path)

### Session picker (was Tier 2.2)

**Why deferred:**

With named sessions live, the discovery story is now:
- "What conversations do I have?" → `hoosh conversations list`
- "Resume one I remember the name of" → `hoosh --resume <name>`
- "Fuzzy search" → `hoosh conversations list | fzf | xargs hoosh --resume`

These cover ~95% of what the picker would do, with zero new code. The picker's remaining value (in-flight preview, branch grouping, keyboard shortcuts) is genuinely "would be nice" but doesn't unblock anything. Roughly 1-2 days of ratatui work for marginal UX polish.

**Revisit if:** humans on hooshi or the laptop start submitting enough parallel conversations that named-list-and-resume feels noisy. Likely never at one-user scale.

### Session branching (was Tier 2.4)

**Why deferred:**

Branching is a power-user feature in Claude. Real-world use is rare; most "try a different approach" flows are served by `/clear` + retry, or just by re-rolling the prompt. ~4-6 hrs of storage-clone + meta-tweak work that benefits no automated workflow.

**Revisit if:** hoosh adds plan mode (checkpoint-style branching becomes more compelling), or if someone hits a concrete use case the workarounds don't serve.

### Other Tier 3 items

- `--from-pr <number>` — requires the daemon path to store PR association in meta.json; relevant if/when daemon and tagged-mode session worlds merge. Not parity work.
- Picker key-binds (`Ctrl+W`/`Ctrl+A`) — moot without the picker.
- Auto-name on plan accept — moot without plan mode.
- `/export` to clipboard or file — JSONL is already greppable; bigger win is markdown rendering, which is a separate feature.
- Retention sweep (`cleanupPeriodDays`) — worth replicating once volume grows. Trivial cron-style addition.

---

## Verification: peyk bridge swap test

The parity acceptance test is now runnable. In `peyk/scripts/claude-bridge.sh`, add a `BRIDGE_AGENT` env var that picks the agent and adjusts the invocation:

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
"$PEYK_BIN" job done "$id" --summary "$result"
```

Both branches should produce indistinguishable behavior at the contract level: same JSON shape, same persistence semantics, same session_id resumability. Run the same prompt sequence ("set X = banana", "what's X?") against each — both should answer "banana" on the second prompt.

When this passes, the September migration is a one-line env-var change in peyk's systemd unit:

```ini
Environment=BRIDGE_AGENT=hoosh
```

Anything that breaks under this swap is parity work that hasn't been done yet. Right now: nothing should break (Tier 1+2.1+2.3 covers the bridge contract). The `--no-session-persistence` addition above doesn't affect the bridge but smooths future scripting.

---

## Out of scope (deliberately)

- **Hoosh daemon mode improvements** — daemon is the analog of "agentic PR runner" not "interactive CLI." Different design space.
- **MCP server integration** — orthogonal feature; not on the parity path.
- **Multi-model routing** — hoosh already has it via `config.toml` backend selection.
- **Plan mode** — Claude-specific UX, not core to bridge parity.

---

## See also

- [peyk's claude-bridge](https://github.com/nomad-prime/peyk/blob/main/scripts/claude-bridge.sh) — the bridge this work unblocks
- [bana's claude user setup](https://github.com/nomad-prime/bana/blob/main/hosts/hooshi/setup-claude-user.sh) — provisions the `claude` system user used by the bridge
- Claude Code session docs: https://code.claude.com/docs/en/sessions
