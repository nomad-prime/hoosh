# Additional gaps from a deeper read of `../claude-code`

Supplements `docs/claude-code-prompting-learnings.md` and
`docs/prompting-strategy-improvement-plan.md`. Each item below is grounded in a
specific file in `../claude-code/src` and judged against what hoosh actually
ships today. Items the existing docs already cover are not repeated.

Filtered for *intelligence* (output quality, fewer wrong turns, better
self-correction), not feature-parity. Cargo-cult items are listed under §
"Considered and skipped" with reasoning.

---

## A. The "ask before you destroy" section is missing from hoosh

CC's `getActionsSection()` (`src/constants/prompts.ts:255-266`) is a single
long paragraph teaching the model **reversibility-and-blast-radius thinking**.
It enumerates concrete risky-action categories (destructive ops, hard-to-
reverse ops, shared-state ops, third-party uploads) and ends with:

> "When you encounter an obstacle, do not use destructive actions as a
> shortcut to simply make it go away."

hoosh's prompts contain none of this. The closest is a one-line note in
`hoosh_planner.txt` not to `cd`. The agent has no framework for thinking
about whether an action is reversible before taking it.

**Why it matters more for hoosh than CC**: per `CLAUDE.md` §"Threat model",
hoosh is explicitly *not sandboxed*. The agent has the full user's
permissions. CC has the same exposure but ships the prompt-level guard
anyway; hoosh has neither sandbox nor prompt.

**Proposed**: lift a hoosh-tailored version of `getActionsSection` into
`hoosh_core_instructions.txt` (so it re-injects periodically via
`PeriodicCoreReminderStrategy`). Keep the four risky-action bullets verbatim;
swap CC-specific examples (`gh pr create`) for ones that match hoosh's tool
surface.

**Cost**: ~30 lines of prompt. Zero code.

---

## B. Tool-preference hierarchy is implicit, not explicit

CC's `getUsingYourToolsSection()` (`prompts.ts:269-314`) hardcodes:

```
Do NOT use the Bash tool to run commands when a relevant dedicated tool is provided.
- To read files use Read instead of cat, head, tail, or sed
- To edit files use Edit instead of sed or awk
- To create files use Write instead of cat heredoc or echo redirection
- To search for files use Glob instead of find or ls
- To search the content of files, use Grep instead of grep or rg
- Reserve Bash exclusively for system commands…
```

hoosh's `hoosh_planner.txt` has a tool-selection table, but `hoosh_coder.txt`
and the subagent literals don't. The result: in practice, hoosh agents reach
for `bash("cat …")` and `bash("grep …")` more often than they should. That
both bloats permission prompts and produces less reviewable tool output.

**Proposed**: hoist the table into `hoosh_core_instructions.txt` once, drop
the duplicate from `hoosh_planner.txt`. Apply to all agents.

---

## C. Parallel-tool-calls guidance is missing

CC ends `getUsingYourToolsSection` with:

> "If you intend to call multiple tools and there are no dependencies between
> them, make all independent tool calls in parallel."

hoosh already *supports* parallel tool calls (shipped in COMPETITOR_GAPS 3.2:
`ToolExecutor::execute_tool_calls` runs concurrently with a semaphore cap of
8). But no prompt tells the model to *use* the capability. Models default to
sequential calls unless explicitly told otherwise.

**Proposed**: one sentence in core instructions:

> "You can call multiple tools in a single response. If they have no
> dependencies between them, call them in parallel."

This is the single highest-ROI change in this list: the runtime is already
paid for, and the model just needs the nudge to use it.

---

## D. No explicit "report outcomes faithfully" anti-confabulation rule

CC ships (`prompts.ts:240`, ant-only):

> "Report outcomes faithfully: if tests fail, say so with the relevant
> output; if you did not run a verification step, say that rather than
> implying it succeeded. Never claim 'all tests pass' when output shows
> failures, never suppress or simplify failing checks…"

CC keeps it ant-only because it's a counterweight for a known model regression
(Capybara v8 had 29-30% false-claims rate). hoosh runs on many backends
(Anthropic, OpenAI, OpenRouter, local Ollama) — the small-Ollama-model
confabulation rate is almost certainly *worse* than CC's worst case.

**Proposed**: include unconditionally in `hoosh_core_instructions.txt`. The
phrasing is already battle-tested.

---

## E. "Diagnose before retry" guidance is missing

CC: "If an approach fails, diagnose why before switching tactics — read the
error, check your assumptions, try a focused fix. Don't retry the identical
action blindly, but don't abandon a viable approach after a single failure
either."

This single sentence prevents two opposite failure modes (loop-on-broken-cmd
vs flail-after-one-error). hoosh has neither.

**Proposed**: add to core instructions.

---

## F. Brief-update / communication discipline

CC's `getOutputEfficiencySection` distinguishes between:
- **Pre-tool-call narration** ("Before your first tool call, briefly state
  what you're about to do")
- **Mid-work updates** ("at key moments: when you find something load-
  bearing, when changing direction")
- **End-of-turn summary** (not relevant here)

hoosh prompts say "be concise" but don't tell the model *when* to speak. The
practical effect is one of two bad modes: silent until done (user thinks
hoosh hung), or running commentary on every tool call (noise).

**Proposed**: add a 3-line "when to speak" block to core instructions. Lower
priority than A–E but cheap.

---

## G. `<env>` block is markdown, not XML (already in the existing plan as P2)

Already covered as Phase 4 in `prompting-strategy-improvement-plan.md`. Keep
it deferred until something higher-value motivates touching
`generate_environment_context`.

---

## H. `omitClaudeMd` for read-only subagents (architectural)

In CC, both `EXPLORE_AGENT` and `PLAN_AGENT` set `omitClaudeMd: true`:

```ts
// Explore is a fast read-only search agent — it doesn't need commit/PR/lint
// rules from CLAUDE.md.
omitClaudeMd: true,
```

Rationale (from CC's own comment): a fast read-only agent doesn't need the
main agent's commit/PR/test-running conventions — those are dead context that
slows the tool. The *main* agent has full context and interprets results.

hoosh injects the full env-context block + main agent prompt for every
subagent. For a 30-second explore subagent, this is ~1-2k tokens of pure
overhead per turn.

**Proposed**: gate AGENTS.md inclusion on agent type. Plan/Explore subagents
skip it; Review subagent keeps it (it needs to check conventions). Phase this
*after* Phase 1 lands so the agent-type plumbing is already in place.

---

## I. Subagent description ≠ system prompt (architectural)

CC's `whenToUse` string is what the caller sees ("Software architect agent
for designing implementation plans…") and is *separate* from the system
prompt the subagent itself reads. The two have different audiences.

hoosh today: the same string in `AgentType::system_message` is both told to
the subagent ("you are a planner") and used in the `task` tool description
that the main agent reads ("plan: Analyzes codebases…"). Drift between the
two is currently impossible because they're literally the same string, but
also: each is suboptimal for its audience.

**Proposed**: when Phase 1 splits these into prompt files, also surface a
separate `when_to_use` description on `AgentType` that the `task` tool
description string consumes. The system prompt file is for the subagent;
`when_to_use` is for the caller.

This is the only item here that touches `AgentType`'s public surface.

---

## J. Extended thinking / reasoning budgets are completely unwired

hoosh's only "thinking" references are a UX state (`AgentEvent::Thinking`,
`CancelKind::Thinking` in `src/agent/conversation.rs`) used to mean "no tools
fired yet, take the turn back." Nothing in any backend sets Anthropic's
`thinking: { budget_tokens: ... }` or OpenAI's `reasoning_effort`. The
capability is entirely absent.

This is arguably the largest single intelligence lift available, and it
costs nothing in prompt-engineering effort — it's a backend/config change.

**Where it helps the most**:
- **Plan subagent**: large context, multi-step reasoning, no time pressure.
  Exactly the workload extended thinking was built for. Plan output quality
  should improve materially.
- **Review subagent**: same shape — adversarial reading of a diff benefits
  from more deliberation before committing to findings.
- **Coder (main agent)**: moderate benefit on hard tasks; not free, so worth
  gating on task complexity rather than enabling globally.
- **Explore**: should stay fast — keep off.

**Shape of the change**:
- Add `thinking_budget: Option<u32>` to `BackendConfig` (so users can override
  per-backend) and an optional default per `AgentType` (Plan: ~5000,
  Review: ~3000, Explore: None, Coder: configurable).
- Anthropic backend: thread budget into the request body, handle the
  `thinking` content block in responses (display or hide is a UX choice).
- OpenAI-compatible backend: map to `reasoning_effort` for o-series and
  Anthropic-compatible providers (OpenRouter passes `thinking` through).
- Backends that don't support either: ignore the budget silently rather
  than error.
- Cost surfacing: thinking tokens are billed separately on Anthropic;
  surface them in the token-usage counter so users see the trade-off.

**Cost**: ~half a day plus tests. Backend trait gets one optional field;
the request-shaping is local to each backend impl.

**Why higher priority than most prompt items**: a well-tuned thinking
budget on Plan/Review likely improves output quality more than any of
items A-I combined. Prompt edits push at the margins; extended thinking
changes the model's reasoning depth.

**Recommended sequencing**: do this *after* the prompt-improvement thread
closes (item A or whatever lands first). Don't bundle prompt edits with a
backend change — they affect different review surfaces.

---

## Considered and skipped (with reasoning)

### Static/dynamic prompt cache boundary marker (CC §1)
CC's `__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__` only pays off if there's a prompt-
prefix cache that can scope to `'global'`. hoosh re-sends the system prompt
every turn through its own conversation model — no scope to cache at.
Same conclusion as the existing plan; documenting here for completeness.

### Fork-subagent `<fork_boilerplate>` (CC §6)
hoosh subagents can't recursively spawn (the `task` tool is excluded from
subagent registries — confirmed at `src/tools/task_tool.rs:83`'s callsite
context). The fork-boilerplate solves a problem hoosh doesn't have.

### Verification-agent VERDICT contract (CC §5)
CC gates this behind both a build-time `feature('VERIFICATION_AGENT')` flag
AND a remote `tengu_hive_evidence` flag defaulting to `false`
(`prompts.ts:393`). Even at Anthropic it's not the default. Adding it to
hoosh now is shipping someone else's unfinished experiment.

### `<tick>` heartbeat / autonomous proactive mode (CC §10)
hoosh has no proactive/cron mode in the bridge-target path. Daemon mode is
not "proactive" in the CC sense (transactional jobs, not continuous wake-
ups).

### Compaction resumption prompt (CC §8)
hoosh uses sliding-window truncation, not summarization. There's no resume
point to inject a prompt at. Revisit only if summary-based context
management lands.

### Background memory-extraction agent (CC `services/extractMemories`)
Interesting but big surface — needs (a) a memdir, (b) a fork mechanism, (c) a
post-turn hook. ~1-2 weeks of work for a "nice to have." Not worth pursuing
until the cheaper prompt improvements (A-E) prove out.

---

## Revised priority list (rolled up from this doc + existing plan)

The existing plan's phases stand, but the prompt items below should ride
into Phase 1 since they're pure prompt edits:

| Priority | Item | Origin | Cost |
|---|---|---|---|
| **P0** | Unify subagent prompts with `.txt` files | existing plan Phase 1 | small |
| **P0** | Parallel-tool-calls nudge (C) | new | one sentence |
| **P1** | Tool-preference hierarchy in core (B) | new | one block |
| **P1** | Report-outcomes-faithfully rule (D) | new | one paragraph |
| **P1** | Diagnose-before-retry (E) | new | one sentence |
| **P1** | Real tool-gating for read-only subagents | existing plan Phase 2 | medium |
| **P2** | Actions/blast-radius section (A) | new | one section |
| **P2** | Anti-pattern inoculation for subagents | existing plan Phase 3 | small |
| **P2** | When-to-speak block (F) | new | one block |
| **P2** | `omitClaudeMd` for plan/explore subagents (H) | new | small |
| **P2** | Split `when_to_use` from system prompt (I) | new | small |
| **P1** | Extended thinking / reasoning budgets (J) | new | medium |
| **P3** | XML `<env>` block | existing plan Phase 4 | small |
| **P3** | Background memory extraction | new | large |

**Recommended first PR**: existing plan's Phase 1 (prompt unification) + items
B, C, D, E (~4 paragraphs of core-instructions text). All pure prompt edits,
none touches the permission system or tool registry. Phase 2 (tool gating)
and items A, H, I get their own PRs.
