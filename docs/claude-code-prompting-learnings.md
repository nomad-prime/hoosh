# Claude Code — Agent Prompting Strategy

> Learnings extracted from the Claude Code source at `/Users/dev/Projects/claude-code`.
> Key files: `src/constants/prompts.ts`, `src/constants/system.ts`, `src/tools/AgentTool/built-in/`, `src/tools/AgentTool/forkSubagent.ts`, `src/coordinator/coordinatorMode.ts`, `src/services/compact/prompt.ts`

---

## 1. Layered System Prompt Architecture

The system prompt is built from **composable section functions**, not one monolithic string:

```
getSimpleIntroSection()        → Role definition
getSimpleSystemSection()       → Tool use rules, markdown, hooks
getSimpleDoingTasksSection()   → Code style, task behavior, security
getActionsSection()            → Reversibility / blast radius thinking
getUsingYourToolsSection()     → When to use which tool
getSimpleToneAndStyleSection() → No emojis, concise, file:line format
getOutputEfficiencySection()   → "Go straight to the point"
// === __SYSTEM_PROMPT_DYNAMIC_BOUNDARY__ ===
[Dynamic sections]             → Memory, env info, MCP instructions, etc.
```

The `__SYSTEM_PROMPT_DYNAMIC_BOUNDARY__` marker splits the prompt into:
- **Static (globally cacheable)** — before the marker, shared across sessions as prefix cache
- **Dynamic (session-specific)** — after the marker, changes per user/session

Source: `src/constants/systemPromptSections.ts`, `src/constants/prompts.ts`

---

## 2. The Persona Formula

Every agent (main or sub) gets a crisp one-liner identity:

```
"You are Claude Code, Anthropic's official CLI for Claude."
"You are a verification specialist. Your job is not to confirm the implementation works — it's to try to break it."
"You are a file search specialist for Claude Code..."
"You are an agent for Claude Code... Complete the task fully—don't gold-plate, but don't leave it half-done."
```

Pattern: **who you are + one sharp behavioral constraint**

Source: `src/constants/system.ts`, `src/tools/AgentTool/built-in/verificationAgent.ts`, `src/tools/AgentTool/built-in/exploreAgent.ts`

---

## 3. Anti-Pattern Inoculation

They explicitly **name failure modes inside the prompt** so the model can resist them.

From the verification agent (`src/tools/AgentTool/built-in/verificationAgent.ts`):

```
"You have two documented failure patterns. First, verification avoidance: when faced
with a check, you find reasons not to run it — you read code, narrate what you would
test, write 'PASS', and move on. Second, being seduced by the first 80%: you see a
polished UI or a passing test suite and feel inclined to pass it, not noticing half
the buttons do nothing..."

"RECOGNIZE YOUR OWN RATIONALIZATIONS:
- 'The code looks correct based on my reading' — reading is not verification. Run it.
- 'I don't have a browser' — did you actually check for mcp__playwright__*?"
```

This is **preemptive self-awareness injection** — teach the model the trap before it falls in.

---

## 4. STRICTLY PROHIBITED / CRITICAL Caps Lock Pattern

Hard constraints use all-caps and `===` banners:

```
=== CRITICAL: READ-ONLY MODE - NO FILE MODIFICATIONS ===
You are STRICTLY PROHIBITED from:
- Creating new files
- Modifying existing files
- Deleting files
```

Then immediately follows with what the agent *can* do. Structure: **ban first, permit second**.

Source: `src/tools/AgentTool/built-in/exploreAgent.ts`, `src/tools/AgentTool/built-in/planAgent.ts`

---

## 5. Structured Output Contracts with Bad/Good Examples

For the verification agent, they define an exact output format with enforcement language and inline examples:

```
Every check MUST follow this structure. A check without a Command run block is not a PASS — it's a skip.

### Check: [what you're verifying]
**Command run:** [exact command]
**Output observed:** [actual terminal output]
**Result: PASS** (or FAIL)
```

They show a **Bad example** vs **Good example** inline so the model learns by contrast.

End verdict is a parseable token: `VERDICT: PASS` / `VERDICT: FAIL` / `VERDICT: PARTIAL`

Source: `src/tools/AgentTool/built-in/verificationAgent.ts`

---

## 6. Fork/Subagent Identity Isolation

When a subagent is spawned, a `<fork_boilerplate>` block is injected into its first user message, overriding parent instructions:

```
STOP. READ THIS FIRST.
You are a forked worker process. You are NOT the main agent.

RULES (non-negotiable):
1. Your system prompt says "default to forking." IGNORE IT — that's for the parent. You ARE the fork.
2. Do NOT converse, ask questions, or suggest next steps
3. Do NOT editorialize or add meta-commentary
...
```

Prevents recursive agent spawning. Rigid output format enforced:

```
Scope: <echo directive>
Result: <findings>
Key files: <paths>
Files changed: <with commit hash>
Issues: <if any>
```

Source: `src/tools/AgentTool/forkSubagent.ts`

---

## 7. Environment Context Block via XML

Dynamic env info is injected as a structured XML block in the system prompt:

```xml
<env>
Working directory: /path/to/project
Is directory a git repo: Yes
Platform: darwin
Shell: zsh
OS Version: Darwin 24.0.0
</env>
```

Source: `src/constants/prompts.ts`

---

## 8. Context Compaction & Resumption Prompt

When context is compressed, a resumption message is injected as a user message:

```
This session is being continued from a previous conversation that ran out of context.
The summary below covers the earlier portion...

Continue the conversation from where it left off without asking the user any further
questions. Resume directly — do not acknowledge the summary, do not recap what was
happening, do not preface with "I'll continue" or similar.
```

In autonomous mode, it adds:

```
You are running in autonomous/proactive mode. This is NOT a first wake-up — you were
already working autonomously before compaction. Continue your work loop.
```

Source: `src/services/compact/prompt.ts`

---

## 9. Tool Preference Hierarchy as an Explicit Rule

From the system prompt:

```
Do NOT use the Bash tool to run commands when a relevant dedicated tool is provided.
- To read files use Read instead of cat, head, tail, or sed
- To edit files use Edit instead of sed or awk
- To search for files use Glob instead of find or ls
```

They teach the agent the same tool-selection discipline they'd want from a human engineer.

Source: `src/constants/prompts.ts`

---

## 10. Autonomous / Proactive Mode as a Separate Prompt Path

There's a distinct system prompt branch for autonomous (cron/proactive) mode using `<tick>` heartbeats:

```
You are in proactive mode. Take initiative — explore, act, and make progress without
waiting for instructions.

You will receive periodic <tick> prompts. These are check-ins. Do whatever seems most
useful, or call Sleep if there's nothing to do.

If you have nothing useful to do on a tick, you MUST call Sleep. Never respond with
only a status message like "still waiting" — that wastes a turn and burns tokens.
```

Source: `src/main.tsx` (line ~2203), `src/services/compact/prompt.ts`

---

## 11. Coordinator / Orchestrator Role

The coordinator agent has its own system prompt distinct from worker agents. Key patterns:

- Receives worker results as `<task-notification>` XML in user-role messages
- Never thanks/acknowledges workers — treats their output as internal signals
- Told explicitly: "Every message you send is to the user"
- Workers return structured XML: `<task-id>`, `<status>`, `<result>`, `<usage>`
- Uses `SendMessage` tool to continue existing workers (preserving their loaded context)

```
"Answer questions directly when possible — don't delegate work that you can handle
without tools."
```

Source: `src/coordinator/coordinatorMode.ts`

---

## Summary Table

| Principle | Technique |
|---|---|
| Role clarity | One-liner persona per agent type |
| Hard constraints | CAPS + `===` banners + "STRICTLY PROHIBITED" |
| Failure mode prevention | Named and pre-inoculated in the prompt |
| Output contracts | Exact format with Bad/Good inline examples + parseable verdict tokens |
| Agent isolation | Injected `<fork_boilerplate>` override on fork |
| Cache efficiency | Static/dynamic boundary marker splits prompt |
| Context resumption | Injected summary message on compaction |
| Tool hierarchy | Explicit ranked list in system prompt |
| Autonomous mode | Separate prompt path with `<tick>` heartbeat tags |
| Verification | Independent adversarial verifier agent with anti-rationalization rules |
| Coordinator/worker split | Separate system prompts; workers return structured XML |
