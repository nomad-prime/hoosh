# Session handover — 2026-06-13

A long working session that covered Claude Code parity research,
extended-thinking wiring, and the first two phases of a persistent
memory feature. This doc captures *current state* and *the next move*
so a fresh-context session can pick up cleanly.

## Where the codebase is

All work landed on `main`. Commits in this session, newest first:

- `a7c0eb6` — Memory Phase B: `save_memory` tool + 4-type taxonomy in
  the prompt
- `337b4f5` — Memory Phase A: tri-mode memory paths + `MEMORY.md`
  prompt injection
- `c05b126` — OpenAI-compatible reasoning budget (OpenRouter `reasoning:
  {max_tokens}`)
- `b6c22c6` — Anthropic native extended thinking
- `0d2cb9c` — `BackendConfig.thinking_budget` field + a small TUI
  styling change
- `712088f` — Subagent prompt unification + core-instruction additions

Working tree is clean. `cargo test` is 849/849. `cargo clippy
--all-targets` clean. `cargo fmt --check` clean.

## What shipped, in one paragraph each

### Subagent prompt unification (712088f)
`AgentType::Plan/Explore/Review` now resolve their system prompts via
`include_str!` from `src/prompts/hoosh_planner.txt`,
`hoosh_explore.txt` (new), and `hoosh_reviewer.txt`. Previously they
used throwaway hardcoded literals while the rich `.txt` files only
fed the main agent — silent drift. Prompts were hedged to be
tool-agnostic about `bash` so they work for both the main agent
(has bash) and the read-only subagent registry (no bash). Core
instructions gained four CC-derived sections: Parallel Tool Calls,
Diagnose Before Retry, Report Outcomes Faithfully, Think Before You
Destroy.

### Extended thinking on Anthropic + OpenRouter (b6c22c6, c05b126)
`BackendConfig.thinking_budget: Option<u32>`. When set:
- **Anthropic native** sends `thinking: {type: "enabled", budget_tokens:
  n}`, forces temperature to 1.0, grows `max_tokens` to `max(base,
  budget + 4096)`.
- **OpenAI-compatible / OpenRouter** sends OpenRouter's unified
  `reasoning: {max_tokens: n}`, also forces temperature to 1.0 (no-op
  for o-series), grows `max_completion_tokens`.

Verified end-to-end against the user's actual OpenRouter setup with
`anthropic/claude-sonnet-4.6` — same prompt produced +436 output
tokens with `thinking_budget=5000` vs unset, response with
`message.reasoning`/`reasoning_details` parsed cleanly. Per-agent
budgets (Plan ~5000, Review ~3000, Explore None) are a deferred
follow-up.

### Memory Phase A (337b4f5)
General-purpose persistent memory at `<storage_root>/memory/`. New
`memory_storage: Option<ConversationStorageMode>` config field on
`AppConfig`/`ProjectConfig` — same `off|local|central` vocabulary as
`conversation_storage`. Falls back to `conversation_storage` if unset,
defaults to `Off`. Independent per-cwd scoping (matches existing
storage choice; not git-root-canonicalized like CC). `MEMORY.md` is
the entrypoint index, capped at 200 lines / 25 KB with a truncation
warning. Prompt block injected as a system message for new
conversations only; resumed conversations keep their stored history.

### Memory Phase B (a7c0eb6)
`SaveMemoryTool` — dedicated scoped-path tool, mirrors
`UpdateSessionFileTool`'s `.into_write_safe()` pattern for
auto-approval. Inputs: `name` (slugged), `type` (closed set:
`user|feedback|project|reference`), `description`, `body`. Writes
YAML frontmatter + body, updates `MEMORY.md` by replacing-or-
appending the slug-matched line. Registered when memory mode is not
`Off`. Prompt got the full CC-derived taxonomy + "what NOT to save" +
"when to access" + "before recommending from memory" drift caveat.
Reads use existing `read_file`/`grep`/`glob` — no dedicated tool.

## Critical user-preference orthogonality

`src/memory_mode/` and the new `src/memory/` are **orthogonal**.

- `MemoryMode::{Conversation, Summary}` is a **context-management**
  feature: it controls what the agent sees as its turn input.
  `Conversation` keeps full transcript; `Summary` drops the
  transcript and the agent maintains a single `summary.txt` per
  conversation as a substitute.
- `memory_storage: {Off, Local, Central}` is **persistent cross-session
  memory**. Independent of `MemoryMode`.

The user has corrected this conflation once. Saved as
`feedback`/memory-features-orthogonality.

## Feedback memories saved this session

In `~/.claude/projects/-Users-dev-Projects-hoosh/memory/`:

- `feedback_no_descriptive_comments.md` — Recurring failure mode.
  Default to ZERO comments on new fields/structs/helpers. The user
  has corrected this twice in this session. Skip rustdoc unless the
  WHY is genuinely hidden.
- `project_memory_features_are_orthogonal.md` — `MemoryMode` is not a
  memory feature. Don't conflate it with general-purpose memory.

Plus pre-existing: `feedback_no_emojis_in_hoosh_ui.md`,
`feedback_no_doc_comments.md`.

## Next moves

### Immediate: live verification of memory Phase B (small, ~15 min)
Set `memory_storage = "local"` in `~/.config/hoosh/config.toml`.
Run hoosh in tmux, ask it to remember something
("remember that I prefer X"). Check that `.hoosh/memory/`
contains the expected file with frontmatter and that `MEMORY.md`
has the index entry. Restart hoosh and ask "what do you remember
about my preferences?" — confirm the model surfaces the memory.

### Phase C: background extractor fork (deferred — medium effort)
CC's safety net: a forked subagent runs at turn end, restricted to
read-anywhere + write-only-inside-memory-dir via `canUseTool`. Skips
when the main agent already wrote to memory this turn. Hoosh
doesn't have a `runForkedAgent` equivalent, and the existing
`TaskTool` subagent pattern is the closest precedent. Worth a design
pass before coding. Do NOT start without explicit go-ahead.

### Phase D: LLM relevance recall (deferred — medium effort)
Sonnet-style sidecall picks ≤5 relevant memories by description match
against the user query. Only worth doing once #Phase C exists and
memories accumulate enough that "dump everything" becomes a real
cost.

### Other deferred items
From `docs/additional-gaps-from-cc-source.md`:
- **Per-agent thinking budgets** — wire `thinking_budget` through
  `AgentType` so Plan defaults to ~5000, Review ~3000, Explore None.
  Requires plumbing through `TaskManager`.
- **Item H — `omitClaudeMd` for Plan/Explore subagents** — save
  tokens by not injecting AGENTS.md content into fast read-only
  subagents.
- **Item I — split `when_to_use` from system prompt** — the `task`
  tool description (caller-facing) and subagent system prompt
  (subagent-facing) are currently the same string.
- **Item A — actions/blast-radius section** in core instructions —
  DONE this session (in `hoosh_core_instructions.txt`).
- **Real Phase 2 from the parity plan** — give Plan/Review subagents a
  read-only bash so their prompts' `cargo check` references actually
  work. Permission-surface change.

## Key files to know about

- `src/memory/` — Phase A + B. `mod.rs`, `entrypoint.rs` (MEMORY.md
  truncation), `prompt.rs` (system prompt builder), `tool.rs`
  (SaveMemoryTool), `tests.rs`, plus `src/prompts/memory_instructions.txt`.
- `src/memory_mode/` — the orthogonal context-management feature. Do
  not confuse.
- `src/storage/mode.rs` — `resolve_storage_root` (conversations) and
  `resolve_memory_root` (memory) live here.
- `src/config/mod.rs` — `memory_storage_mode()` and
  `memory_storage_root()` resolvers on `AppConfig`.
- `src/cli/agent.rs:86-89` — `SaveMemoryTool` registration gate.
- `src/session.rs:188-198` — memory directory mkdir + prompt
  injection.
- `src/backends/anthropic.rs` — `thinking_request_overrides` helper.
- `src/backends/openai_compatible.rs` — `reasoning_request_overrides`
  helper.

## Pickup docs already written

These are the breadcrumbs you should read first if continuing:

1. `docs/cc-memory-architecture-vs-hoosh.md` — the memory comparison
   that motivated Phases A–D.
2. `docs/memory-phase-b-pickup.md` — written *before* Phase B was
   coded. Now historical, but useful for the design rationale.
3. `docs/additional-gaps-from-cc-source.md` — broader CC findings
   beyond memory.
4. `docs/CLAUDE_CODE_PARITY.md` — the original parity roadmap.

## Things that will save you time

- **Tests have one flaky test**: `daemon::sandbox::tests::
  clone_creates_repo_at_sandbox_path` occasionally fails on a network
  hiccup. If `cargo test --lib` shows exactly one failure and it's
  that test, re-run with `--no-fail-fast` to confirm everything else
  is green.
- **Existing precedent for scoped-path tools**:
  `src/memory_mode/tool.rs` `UpdateSessionFileTool`. Same pattern was
  used for `SaveMemoryTool`. If you add another scoped tool, copy
  this shape.
- **The user's OpenRouter API key is in
  `~/.config/hoosh/config.toml`** under `[backends.openai]`. Don't
  print it; just use it.

## Restart prompt

```
Read docs/session-handover-2026-06-13.md first. The handover names
what's done, what's next, and the user's preferences. Then check
git log -5 to confirm the head matches a7c0eb6 (or later). After
that, ask me what to work on next — don't assume Phase C is the
default.
```
