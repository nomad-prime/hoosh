# hoosh — open work

Single source of truth for what's left. When something ships, delete
its entry. When something new comes up, add it here.

Last reset: 2026-06-13. HEAD at reset: `f22ff2f`.

## Small (prompt edits or one-file changes)

### Show model thinking in TUI — collapse follow-up
v1 shipped: Anthropic `thinking` blocks and OpenAI-compatible
`reasoning` field are extracted into `LlmResponse.thinking`, emitted
as `AgentEvent::AssistantThinking`, and rendered dimmed-italic
inline under a `⎿ thinking` header (`add_thinking` in
`src/tui/app_state.rs`). Remaining: ctrl+o to collapse/expand the
most recent thinking block. Blocked because `insert_before` writes
straight into terminal scrollback — collapse needs the fullview
re-render path (`app_loop_fullview.rs`) and a new
`MessageLine::Thinking { content, collapsed }` variant. Also a
`display.show_thinking` config knob to suppress entirely.

## Medium

### Brevity mode v1 — remaining
v1 shipped: `display_compact` bool on `AppConfig`, runtime
`Ctrl+B` toggle via `text_input_handler.rs`, completion paths in
`app_state.rs` skip the `⎿` continuation line when compact.
Subagent task summaries are preserved (status info, not result
body). Status message surfaces the new mode on toggle.
Remaining: memory tools (`save_memory`) should collapse to
`Saved memory: <slug>` in *both* modes by default — needs raw
tool name on `ActiveToolCall` (currently only `display_name`
flows through `ToolCalls` event). Also: per-tool override via a
trait method like `fn display_mode(&self) -> ToolDisplay` if a
second tool ever needs the same treatment.

### Orchestration mode for Task subagents
Top-level agent runs in orchestration mode: minimal direct tool
use, delegates to Plan/Explore/Review/Coder subagents and stitches
results. Goal is keeping the main context lean and letting
specialists do focused work with their own budgets. Open
questions: how to express "orchestrator-only" vs hybrid; whether
it's a config knob (`agent.mode = "orchestrate" | "direct"`) or
a separate AgentType; how to handle short tasks where delegation
overhead isn't worth it.

### Read-only bash for Plan/Review subagents
Their prompts reference `cargo check` but the subagents can't
actually run shell commands. Permission-surface change — needs a
constrained bash with a whitelist (cargo, git read ops, etc.).
Original Phase 2 of the parity plan.

## Larger (gated — do not start without explicit go-ahead)

### Memory Phase C — background extractor fork
CC's safety net: a forked subagent runs at turn end, restricted to
read-anywhere + write-only-inside-memory-dir via `canUseTool`.
Skips when the main agent already wrote to memory this turn. Hoosh
has no `runForkedAgent` equivalent; closest precedent is the
existing `TaskTool` subagent pattern. **Needs a design pass before
coding.** See `docs/cc-memory-architecture-vs-hoosh.md` for the
rationale.

### Memory Phase D — LLM relevance recall
Sonnet-style sidecall picks ≤5 relevant memories by description
match against the user query. Only worth doing after Phase C
exists and memories accumulate enough that "dump everything"
becomes a real cost.

### XML `<env>` block
`generate_environment_context` currently emits markdown. CC uses
XML tags. Defer until something higher-value motivates touching
it.

## Non-negotiable constraints

- **No emojis in hoosh UI** — including `✓`, `✗`, etc. Brailles only
  (`⎿`, `◐`, `●`, `○`). See memory `feedback_no_emojis_in_hoosh_ui`.
- **No descriptive comments** on new code. Default to zero comments
  on new fields/structs/helpers. Skip rustdoc unless the WHY is
  genuinely hidden. See memory `feedback_no_descriptive_comments`.
- **`MemoryMode` (`src/memory_mode/`) is orthogonal to general
  memory (`src/memory/`)**. The first is context-management when
  the transcript is dropped; the second is persistent cross-session
  memory. Do not conflate. See memory
  `project_memory_features_are_orthogonal`.
- **A parallel thread does TUI cosmetics**. `src/tui/header.rs` was
  recently touched there. If `git status` shows uncommitted TUI
  changes you didn't make, leave them alone unless the user asks.
- **The OpenRouter API key lives in `~/.config/hoosh/.env`** as
  `OPENROUTER_API_KEY=…`. Config references it via
  `${env:OPENROUTER_API_KEY}`. Don't print or copy it.
- **One flaky test**:
  `daemon::sandbox::tests::clone_creates_repo_at_sandbox_path`
  (network hiccup). If you see exactly one failure and it's that
  test, re-run.

## Pickup prompt

```
Read docs/TODO.md first — it has the current open list and the
constraints. Then run `git log -5 --oneline` to confirm HEAD is
where TODO.md says it should be.

Before suggesting work, check `git status` — a parallel cosmetics
thread sometimes has uncommitted changes in src/tui/. Leave those
alone.

Then ask me which item to take. Don't assume a default.
- Small items 1-3 are the cheapest wins.
- Per-agent thinking budgets is the highest output-quality lift.
- Brevity mode is what we discussed most recently.
- Memory Phase C/D require my explicit go-ahead.

Honour all the constraints listed in TODO.md without exception.
```

## Reference material (keep)

- `docs/CLAUDE_CODE_PARITY.md` — peyk bridge swap roadmap; mostly
  done, separate concern from the prompt/feature work above.
- `docs/cc-memory-architecture-vs-hoosh.md` — the comparison that
  motivated memory Phases A-D. Read before Phase C.
- `docs/claude-code-prompting-learnings.md` — research notes from
  reading CC's prompts.
- `docs/prompting-strategy-improvement-plan.md` — research notes,
  superseded in priorities by the items above but still useful for
  rationale.
- `docs/COMPETITOR_GAPS.md` — competitor analysis.
