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

### 1.4 WebSearch / WebFetch (curl-first approach)
Don't build a search tool. Pre-approve `curl` for HTTPS GETs to a configurable allowlist (or all of HTTPS by default in autopilot) and let the agent drive. For richer fetching add a thin `web_fetch` tool that wraps `curl` + readability extraction. Defer Playwright until a real use case asks for it.

---

## Tier 2 — Medium

### 2.1 Permission leak fixes
- Pipe redirects (`cmd > file`, `cmd >> file`, `tee`) must trigger write permission.
- Heredocs stop re-prompting after first approval in the session.
- Disallow / warn on `cd` outside working dir (agent keeps doing it).
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
