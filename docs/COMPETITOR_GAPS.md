# Competitor gaps — must-ship list

The shortlist of things hoosh needs to be a credible alternative to Claude Code, ordered easy → hard. Not parity for parity's sake; just the items whose absence makes a user bounce in the first session, plus the wedges that make them prefer hoosh.

Explicitly **out of scope**: MCP, LSP, hooks, IDE extensions, notebook edit, session picker, compression (handoff command already solves this better).

---

## Tier 1 — Easy / quick wins

### 1.1 Ctrl+C semantics
First Ctrl+C cancels the in-flight turn; second exits the program. Today first Ctrl+C exits, which is a footgun users hit in minute one. Small state machine in the TUI input loop.

### 1.2 Logging to file
Wire `tracing` (or equivalent) to `~/.config/hoosh/logs/hoosh.log` with rotation. Debug events are already emitted via `AgentEvent` but discarded. Without this, users can't send us anything when things go wrong.

### 1.3 Runtime backend + model switch
A `/backend` and `/model` slash command that swaps the live backend without editing TOML and restarting. Hoosh's multi-backend is the wedge — currently buried in config. Should also show current cost-per-1k and context size on switch.

### ~~1.4 WebSearch / WebFetch~~ — folded into 2.1
Decided not to ship as a separate tool. Bash already gives the agent `curl`; adding a dedicated tool would just add schema bloat to every turn for zero new capability. The actual blocker is permission friction on every `curl` invocation — addressed under 2.1 by classifying read-only network commands (`curl`/`wget`/`gh api` GETs) as a single pre-approvable class. Playwright stays deferred until real use cases prove curl is insufficient.

---

## Tier 2 — Medium

### 2.1 Permission leak fixes + read-only-network class
- Pipe redirects (`cmd > file`, `cmd >> file`, `tee`) must trigger write permission.
- Heredocs stop re-prompting after first approval in the session.
- Disallow / warn on `cd` outside working dir (agent keeps doing it).
- **New**: classify read-only network reads as their own permission class — `curl https://...` (no `-X POST`/`--data`/`--upload-file`), `wget`, `gh api` GETs. Single one-time approval covers all of them so web fetch doesn't get permission-prompt friction.
Audit the bash parser in `src/tools/bash/parser.rs`; add cases + tests.

### 2.2 Tool-call recovery on crash
On reload, drop assistant tool calls that have no corresponding tool result instead of injecting fake responses. Today we patch with a fake message and the next turn confuses the model.

### 2.3 Cancel-midflight UX
Cancel currently leaves the screen in a weird state — partial spinner, ambiguous cursor, unclear whether the next prompt is a new turn or a continuation. Define and implement clean cancel: stop streaming, close spinner, append a `[cancelled]` marker to the message, reset input.

### 2.4 Warm-up / pre-approve at session start
At session start, ask the agent to list the commands it expects to need for the upcoming task; show the user a single approval dialog for the whole batch. Turns the permission system from per-call friction into a one-time gate. Pairs naturally with 3.1 (plan mode).

### 2.5 Queue messages mid-flight
Let the user type while the agent works; queued messages are delivered at the next turn boundary (or interrupt and append, configurable). Requires decoupling the input task from the agent task in the TUI loop.

---

## Tier 3 — Hard

### 3.1 Plan → autopilot toggle
"Approve this plan, then run unattended until done." Shift+Tab style. Requires:
- A plan-mode prompt + structured plan output
- An acceptance UI
- Mode switch into autopilot with the warm-up allowlist (2.4) covering the plan's commands
- A stop condition / budget so autopilot terminates cleanly

### 3.2 Parallel tool calls
Execute independent tool calls concurrently. Hard parts:
- Permission prompts must serialize (can't ask two questions at once in the TUI)
- Output ordering in the conversation
- Cancellation semantics for a half-completed batch
- Resource caps (don't spawn 20 bashes)

### 3.3 Image input / screenshot support
Two-track:
- **Multimodal path**: backends that support images (Anthropic, OpenAI, some Together models) accept inline images. Add image content type to `ConversationMessage`, wire clipboard paste + drag-drop + file ref (`@screenshot.png`) in the TUI.
- **Parallel-call path**: for non-multimodal primary backends, run the image through a side multimodal model (e.g. local llava via Ollama, or a cheap Haiku call) to produce a text description, then inject as a system reminder. Lets users on Together/Ollama still benefit.

---

## Execution order

Suggest shipping in tier order. Tier 1 is 1-2 days total; gets immediate user-visible improvement. Tier 2 is the trust + UX foundation needed before plan-mode is worth building. Tier 3 is where hoosh starts pulling ahead.

---

## Progress (newest first)

- **3.2 Parallel tool calls** — shipped this session. `ToolExecutor::execute_tool_calls` now runs concurrently via `futures::join_all` gated by a `tokio::Semaphore` (cap 8, configurable via `with_max_parallel_tool_calls`). Result order preserved. Permission/approval prompts serialize end-to-end by locking the response receiver **before** emitting the request event (`permissions::ask_user_tool_permission`, `tool_executor::request_approval`) — fixes a race where the TUI dialog (`Option<...>`) would otherwise be overwritten by a second concurrent request. Cancellation of in-flight batches still deferred (token only checked at agent-loop boundaries). Also: in-flight tool indicator now sweeps left-to-right with a full-height braille bar (`⡇⣿⢸` through 3 cells, 8 frames), and Completed/Error use `●` green/red instead of `✓`/`✗` to match the dot family
- **2.5 Queue messages mid-flight** — shipped `fce6ae8`; queue indicator above input shipped `2db95e6`; emoji purge `54363d3`
- **2.2 Tool-call recovery on crash** — shipped `fce6ae8`. `Conversation::sanitize_orphan_tool_calls` drops orphans instead of synthesising fake results; storage rewrite via `ConversationStorage::rewrite_messages` makes drops survive reload
- **2.1 Permission leaks + read-only-network class** — shipped `f550eda`. `NetworkReadPattern` (`net:read`), `cd:outside` detection in `BashTool::describe_permission`, `SubshellPattern` no longer offers project-wide trust (`allow_project_wide_trust` flag propagated through `CommandPatternResult` → `ToolPermissionDescriptor` → dialog)
- **1.4** Folded into 2.1 — no separate WebFetch tool
- **1.3 Runtime `/backend` + `/model` switch** + slash-only-at-start — shipped `d563c71`
- **1.2 File-based logging** with rotation — shipped `bf1682f`. `~/Library/Application Support/hoosh/logs/` on macOS, `~/.config/hoosh/logs/` on Linux. Level via `HOOSH_LOG`/`RUST_LOG`
- **1.1 Ctrl+C / Esc cancel** + prompt restore — shipped `bf1682f`

## Remaining must-ships

- **2.3 Cancel-midflight UX polish** — partial spinner/cursor cleanup. Small (~1-2h). Probably the next pick.
- **2.4 Warm-up / pre-approve at session start** — pairs naturally with 3.1 plan-mode; defer until plan-mode design is firmer.
- **3.1 Plan → autopilot toggle** — the wedge. Bundle with 2.4.
- **3.3 Image input / screenshot support** — highest "wait it can't do that?" moment for new users.

---

## Resume prompt — read this if you're picking up cold

You're continuing work on `docs/COMPETITOR_GAPS.md`. The user explicitly chose this roadmap and has been driving picks one item at a time. The pattern that's been working:

1. **Understand existing code before touching it.** The user has corrected me twice for almost-redeveloping things that already exist. Before any new pattern/handler/component, grep for what's already there. Tier 2.1 was mostly tightening existing patterns, not new ones.
2. **Reproducer tests first** when the user reports "X is broken." Several reported bugs turned out to already be fixed (e.g. pipe-to-file leak from ISSUES.md). Don't trust the bug report over the code.
3. **No emojis in hoosh UI, ever.** Memory file: `feedback_no_emojis_in_hoosh_ui.md`. Use Unicode geometric shapes (○ ◐ ● ⎿) or plain text. Look at `TodoListComponent` for the right vocabulary.
4. **Verify in tmux before commit.** The user explicitly asked for live verification and caught a hallucinated capability ("did you actually ask hoosh what model it sees?"). Don't claim things work without observing them.
5. **Commit after user confirms.** Pipeline: `cargo fmt && cargo clippy --all-targets && cargo test`. All must be green. Push only after the user says "go ahead" or equivalent.
6. **Slash commands**: only fire when input is empty (start of prompt). Implemented via `Completer::should_trigger`.
7. **The three event loops** (`app_loop.rs`, `app_loop_inline.rs`, `app_loop_fullview.rs`) duplicate a lot — every change to one needs the same in the other two. Helpers go in `app_loop.rs` as `pub(crate)`, called from siblings.

**Where the user is likely to want to start**: Tier 2.3 (cancel UX polish) — small and the cancel state still feels rough. Or 3.3 (image input) for impact. Ask before assuming.

**Key files for the most likely next item (2.3)**: `src/tui/handlers/quit_handler.rs`, `src/tui/app_loop.rs` (the `ShouldCancelTask` branch — there are three copies across the loops), `src/agent/core.rs` (where streaming lives — find what doesn't shut down cleanly on cancel).

**3.2 follow-ups (deferred, worth picking up later)**: in-flight tool cancellation (pass the cancellation token through `ToolExecutionContext` so bash and other long-running tools can abort mid-execution), and exposing `max_parallel_tool_calls` as an `AppConfig` knob.

**Threat model reminder** from `CLAUDE.md`: hoosh isn't sandboxed at the OS level. Permission system is for "confused agent," not malicious input. Don't add defenses that pretend otherwise.

