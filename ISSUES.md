
### Subagents

Subagents should only show visually the tool calls, not agents repsonses and thinking

proper summary result under explore and plan

### Compression

broken

### Permission.json

if hoosh is running, a change on permissions file is overwritten

### ctrl+c on setup and init_permission 

just enters instead of exiting

### Pipe to file should trigger permissions

echo "Hello, this is a test file created at $(date)" > test_output.txt && c..

did not

### tool calling fixing in case of crashes

currently adding a tool message with some answer -> better just remove the ones that dont have proper answer

### bash permission
heredoc keeps asking

### Auto Scroll 
auto scrolling when dialogs open up in custom terminal has a limitation, lets see if we can remove that height limit


### LLM Keeps cd-ing in working directory


### Permission Dialog when exploring
currently does not pause the timer -> we have the methods in execution budget we should pause the timer, when user is in control

### Doubled blank line between consecutive tool calls

`add_tool_completion_header` emits a leading blank, and `complete_*_tool_calls` now emits a trailing blank after the `⎿` summary. When two tool calls land back-to-back, that's two blank lines between them. Either drop the leading blank (rely on the previous tool's trailing) or only emit the trailing one when the tool block is the last thing before the assistant's reply.

### Collapse parallel tool calls into a summary line

When the agent fires several reads/greps in parallel, hoosh currently prints one full header per call (each with its own glyph + name + summary). Claude Code instead collapses them into a single line like `Searching for 3 patterns, reading 1 file… (ctrl+o to expand)` and tucks the per-call detail behind an expand toggle. We should do the same when N>1 same-class tool calls land in the same batch — group them by tool family (Read/Grep/Glob → "Searching"), show counts, and keep the individual entries collapsible.

### Subagent budget % gets stuck

`budget_pct` shown in the active subagent line (`12% done`, etc.) only refreshes when a step event fires (`AssistantThought` / `ToolExecutionStarted/Completed` / `ToolResult`). Between events the cached `tool_call.budget_pct` is rendered as-is, so during long thinking pauses or long-running tools the percentage looks frozen while the timer keeps climbing — `percentage_used = max(steps_pct, time_pct)` and `time_pct` is real-time-based. Fix: either tick a time-only update from the event loop, or compute the time component in the TUI from the budget's start time + max duration.

### Subagent cost tracking

Two related gaps in `task_manager.rs` + `app_state.rs`:

1. The subagent completion line `Done (N tool uses · T tokens · Xs)` does not show cost. `SubagentTaskComplete` (in `agent_events.rs`) only carries token counts — extend it with a `cost` field and render it.
2. Subagent token usage is **not** summed into the parent's `total_cost`. `should_emit_to_parent` in `task_manager.rs` explicitly excludes `TokenUsage`, so the parent's accumulator runs short of reality on every subagent run. Either forward `TokenUsage` upward, or include cost in `SubagentTaskComplete` and add it to `total_cost` when handling that event.

### Agent output noisiness

hoosh tends to over-narrate: redundant pre/post summaries, tables when a sentence would do, repeating the diff in prose right after the diff was shown. Likely a system-prompt issue — tighten the default tone (terse updates, no recap of what the diff already shows, no decorative tables unless asked).
