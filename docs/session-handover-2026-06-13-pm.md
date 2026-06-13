# Session handover — 2026-06-13 (afternoon)

Continuation of `session-handover-2026-06-13.md`. That doc remains
the historical record for the morning session. This one is the
*current* state: what shipped since, what's now off the list, and
what's still open.

## Where the codebase is

Branch: `main`, clean working tree.
HEAD: `d437fce` (header style fix from the parallel cosmetics thread).
Tests: 859 passing. Clippy clean. Fmt clean.

New commits since the morning handover:

- `3b09b88` — `${env:VAR}` interpolation + `.env` loading. Secrets
  now live in `~/.config/hoosh/.env` (and optionally `.hoosh/.env`),
  not in `config.toml`. Project configs can override `model` without
  duplicating `api_key`. Real process env wins over `.env` files;
  project `.env` wins over global.
- `d437fce` — header style fix (parallel thread, cosmetic only).

Memory Phase B was live-verified on this machine. `memory_storage =
"local"` is set in global config and a `.hoosh/memory/` directory
exists in this repo (gitignored along with all of `.hoosh/` and
`.env*`).

## What's now done (crossed off the priority list)

From `docs/additional-gaps-from-cc-source.md` "Revised priority list":

- ✅ **P0 — Unify subagent prompts** (`712088f`, morning session)
- ✅ **P0 — Parallel-tool-calls nudge (C)** — landed in core
  instructions
- ✅ **P1 — Report-outcomes-faithfully rule (D)** — landed in core
  instructions
- ✅ **P1 — Diagnose-before-retry (E)** — landed in core instructions
- ✅ **P1 — Extended thinking / reasoning budgets (J)** — backend
  level (`b6c22c6`, `c05b126`). Per-agent defaults still TODO.
- ✅ **P2 — Actions/blast-radius section (A)** — landed in core
  instructions

Memory work:

- ✅ **Phase A** — tri-mode paths + `MEMORY.md` injection (`337b4f5`)
- ✅ **Phase B** — `save_memory` tool + 4-type taxonomy (`a7c0eb6`)
- ✅ **Phase B live verification** — confirmed write/read cycle works
  end-to-end with `memory_storage = "local"` in this repo

Config / secrets:

- ✅ **`${env:VAR}` interpolation + `.env` loading** (`3b09b88`) — not
  on the original parity list, came up this afternoon. Fixes the
  api-key-leak risk from project-config replace semantics.

## Clean to-do list (what's actually left)

Ordered roughly by leverage-per-effort, not by original priority tag.

### Small (prompt or one-file changes)

1. **Tool-preference hierarchy in core (B)** — hoist the `Read > cat`,
   `Edit > sed`, `Grep > grep` table from `hoosh_planner.txt` into
   `hoosh_core_instructions.txt` so all agents see it. Cuts `bash(cat
   …)` reach-for. One block of text.
2. **When-to-speak block (F)** — three lines in core instructions
   covering pre-tool-call narration, mid-work updates, end-of-turn
   summary. Stops both silent-until-done and running-commentary
   failure modes.
3. **Split `when_to_use` from system prompt (I)** — `AgentType` gains
   a separate `when_to_use: &str` field used by the `task` tool
   description; the system prompt file stays subagent-facing. Drift
   currently impossible because they're the same string, but each is
   suboptimal for its audience.

### Medium (touches more than one module)

4. **Per-agent thinking budgets** — wire `thinking_budget` through
   `AgentType` (Plan ~5000, Review ~3000, Explore None, Coder
   configurable). Plumbing through `TaskManager`. The backend side
   is already done — this is the per-subagent-default layer.
5. **Brevity mode v1** — `display.verbosity = "compact" | "full"`
   config knob. Compact renders every tool call as a single
   braille-prefixed summary line (no checkmarks — hoosh uses brailles,
   see `feedback_no_emojis_in_hoosh_ui`). Memory tools collapse to
   `"Saved memory: <slug>"` in *both* modes. Entry points:
   `src/tui/message_renderer.rs`, components under
   `src/tui/components/`, new `DisplayConfig` near `AppConfig.verbosity`
   (`mod.rs:155`), and a per-tool trait method like
   `fn display_mode(&self) -> ToolDisplay`. No expand-on-keypress in
   v1 — transcript on disk has everything.
6. **`omitClaudeMd` for Plan/Explore subagents (H)** — gate AGENTS.md
   injection on agent type. Plan/Explore skip it; Review keeps it.
   Saves ~1-2k tokens per subagent turn. Sequencing: do this *after*
   Item I lands so the agent-type plumbing is already in place.
7. **Real Phase 2 parity — read-only bash for Plan/Review subagents**
   — their prompts reference `cargo check` but they can't actually
   run it. Permission-surface change; needs a constrained bash that
   only allows a whitelist.

### Larger

8. **Memory Phase C — background extractor fork** — CC's safety net:
   a forked subagent runs at turn end, restricted to read-anywhere +
   write-only-inside-memory-dir via `canUseTool`. Skips when the main
   agent already wrote to memory this turn. Hoosh has no
   `runForkedAgent` equivalent; closest precedent is the existing
   `TaskTool` subagent pattern. **Design pass before coding.** Do not
   start without explicit go-ahead.
9. **Memory Phase D — LLM relevance recall** — Sonnet-style sidecall
   picks ≤5 relevant memories by description match against the user
   query. Only worth doing after Phase C exists and memories
   accumulate enough that "dump everything" becomes a real cost.
10. **XML `<env>` block** — `generate_environment_context` currently
    emits markdown. CC uses XML tags. Defer until something
    higher-value motivates touching it.

## Critical context for the next agent

- **`MemoryMode` (`src/memory_mode/`) is orthogonal to general memory
  (`src/memory/`)**. The morning handover and a saved feedback memory
  both flag this. Don't conflate them.
- **No emojis in hoosh UI** — including no `✓`, `✗`, etc. Use braille
  glyphs (`⎿`, `◐`, `●`, `○`). See `feedback_no_emojis_in_hoosh_ui`.
- **No descriptive comments** — the user has corrected this multiple
  times. Default to zero comments on new fields/structs/helpers.
  Skip rustdoc unless the WHY is genuinely hidden. See
  `feedback_no_descriptive_comments`.
- **A parallel thread is doing TUI cosmetics**. `src/tui/header.rs`
  was touched there. Don't be surprised if other `src/tui/` files
  have uncommitted changes you didn't make — leave them alone unless
  the user asks.
- **The OpenRouter API key is in `~/.config/hoosh/.env`** as
  `OPENROUTER_API_KEY=…`. Config references it via
  `${env:OPENROUTER_API_KEY}`. Don't print or copy it elsewhere.
- **Tests have one flaky test**:
  `daemon::sandbox::tests::clone_creates_repo_at_sandbox_path` —
  network hiccup. If you see exactly one failure and it's that test,
  re-run.

## Pickup prompt for the next agent

```
Read docs/session-handover-2026-06-13-pm.md first — it supersedes
the morning handover and has the current clean to-do list with
crossed-off items removed. Then run `git log -5 --oneline` to
confirm HEAD matches `d437fce` (or later).

Before suggesting work, check whether the parallel cosmetics thread
has touched src/tui/ — `git status` will show it. Leave those
changes alone.

Then ask me which of the open items to take. Do not assume any
default — items 1-3 are the cheapest wins, item 4 (per-agent
thinking budgets) is the highest output-quality lift, item 5
(brevity mode) is what we discussed most recently. Phase C and D
of memory are explicitly gated on my go-ahead.

Constraints (do not violate without asking):
- No emojis in hoosh UI. Brailles only.
- No descriptive comments on new code.
- `MemoryMode` and the new `memory/` module are orthogonal — do not
  conflate.
- The TUI cosmetics thread owns src/tui/header.rs etc; coordinate
  before touching tui files.
```
