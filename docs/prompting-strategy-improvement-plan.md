# hoosh Prompting Strategy — Improvement Design

> Derived from a first-hand reading of the Claude Code source at `../claude-code`
> (not just the summary in `claude-code-prompting-learnings.md`) cross-referenced
> against hoosh's actual prompt machinery.

## 1. How hoosh assembles prompts today (ground truth)

| Concern | Where | Notes |
|---|---|---|
| Main agent personas | `src/prompts/hoosh_*.txt` | Registered in `config::DEFAULT_AGENTS`, loaded by `AgentDefinitionManager`, injected via `conv.add_system_message(agent.content)` in `session.rs::load_or_create_conversation`. |
| Core-instruction reminders | `src/prompts/hoosh_*_core_instructions.txt` | Re-injected on a token interval by `PeriodicCoreReminderStrategy`. Good design, ahead of CC's doc. |
| Environment context | `session.rs::generate_environment_context` | Markdown block, injected as a system message. |
| Subagent personas | `src/task_management/mod.rs::AgentType::system_message` | **Hardcoded 3-line strings**, injected as a *user* message in `task_manager.rs`. |
| Subagent tool gating | `src/tools/task_tool.rs::get_tool_registry_for_agent` | Stub: returns the same readonly registry for `plan`/`explore`/`review`, ignores `agent_type`. |
| Context management | `src/context_management/*` | Sliding-window + tool-output truncation. No summarization/compaction, so CC's "resumption prompt" pattern does **not** apply. |

## 2. What I learned from the real CC source (corrections to the summary)

1. **Read-only is enforced structurally first.** `planAgent.ts`, `exploreAgent.ts`, and
   `verificationAgent.ts` all set `disallowedTools: [FILE_EDIT, FILE_WRITE, ...]`. The
   CAPS banner only *mirrors* that ("attempting to edit files will fail"). A banner without
   real tool removal would be a hollow promise.
2. **Tool names are interpolated** (`${FILE_READ_TOOL_NAME}`) so the prompt can never drift
   from the tool schema. hoosh hardcodes tool names in `.txt` files.
3. **Output efficiency / tone / actions are separate composable sections** in CC. hoosh bundles
   everything into one big `.txt` per agent.

> Note: CC also ships an adversarial *verification agent* with a parseable `VERDICT:` token.
> It is gated behind a build-time flag **and** a remote flag that defaults to `false`, so it is
> effectively off in public builds and unproven. Deliberately **out of scope** for hoosh — see §5.

## 3. Gap analysis (prioritized)

### P0 — Subagents ignore the rich prompts (correctness bug, not just polish)
`AgentType::system_message` duplicates a weak version of what already exists in
`hoosh_planner.txt` / `hoosh_reviewer.txt`, and there is **no** explore prompt file.
Two disconnected prompt systems.

### P1 — Read-only claims aren't backed by tool gating
If we add "READ-ONLY" banners to subagent prompts, `get_tool_registry_for_agent` must
actually remove write/edit tools per agent type, or the banner lies.

### P2 — Persona one-liners + anti-pattern inoculation missing from subagents
The rich main-agent `.txt` files have good personas; subagents get bland descriptions.

### P2 — Env context is Markdown, not an XML `<env>` block
Minor: XML is more parse-stable and matches CC. Low risk.

## 4. Proposed changes (incremental, each independently validatable)

### Phase 1 — Unify subagent prompts with the rich prompt files (P0)
- Introduce a single source of truth: have `AgentType` resolve its base prompt from a
  prompt file (`hoosh_planner.txt`, `hoosh_reviewer.txt`, new `hoosh_explore.txt`) via the
  existing `DEFAULT_AGENTS`/`include_str!` mechanism, falling back to the current string if
  the file is unavailable.
- Add `hoosh_explore.txt` and register it.
- Keep `task_prompt` + budget appended exactly as today.
- Tests: existing `mod_tests` assert substrings like `"code review"` / `"Review auth code"`;
  preserve those or update tests deliberately.

### Phase 2 — Make read-only real, then state it (P1)
- Implement `get_tool_registry_for_agent` to build a per-type registry that excludes
  `write_file`/`edit_file` for `plan` and `explore` (and `review`, which may still run
  read-only bash checks). Verify against `ReadOnlyToolProvider`.
- Only after gating is real, add the `=== CRITICAL: READ-ONLY MODE ===` banner to those
  prompts (banner mirrors reality, per CC).

### Phase 3 — Anti-pattern inoculation + persona sharpening (P2)
- Add "RECOGNIZE YOUR OWN RATIONALIZATIONS" blocks to explore/plan/review prompts.
- Sharpen each subagent's first line to "who you are + one behavioral constraint".

### Phase 4 — XML `<env>` block (P2, low risk)
- Wrap `generate_environment_context` output in `<env>...</env>` with `key: value` lines.

## 5. Explicitly NOT doing (and why)
- **Static/dynamic cache boundary marker (#1)**: hoosh re-sends system messages per request
  via its own conversation model; there's no prefix-cache abstraction that a marker would
  feed. Adding a marker now is cargo-culting. Revisit only if/when prompt caching lands.
- **Fork boilerplate (#6) / coordinator split (#11) / autonomous `<tick>` (#10)**: hoosh's
  `task` subagents don't recursively spawn (the task tool is excluded from subagent
  registries) and there's no coordinator/proactive mode in this codebase path. No-op.
- **Compaction resumption prompt (#8)**: hoosh uses sliding-window truncation, not
  summarization. Not applicable.
- **Adversarial verification agent + `VERDICT:` contract (#5)**: in CC this is gated behind a
  build-time flag and a remote flag defaulting to `false` — effectively dark in public builds
  and unproven. It would also only pay off with (a) TUI surfacing like other `task` agents and
  (b) caller-side verdict parsing to gate completion. Too speculative for now; revisit if the
  pattern proves out upstream.

## 6. Risk & sequencing
Phases are ordered by dependency: Phase 2's banner depends on Phase 2's gating. Each phase
compiles and passes tests on its own. Recommended first PR: **Phase 1 + Phase 4** (pure prompt
plumbing, lowest risk, highest clarity win). Phase 2 is behavioral and warrants its own review.
