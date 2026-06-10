# P0 — Subagents Ignore hoosh's Real Prompt Files

> Deep-dive on the highest-priority gap from
> `docs/prompting-strategy-improvement-plan.md`. Grounded in the actual code in
> `src/task_management/mod.rs`.

## Summary

**The `task` subagents (`plan`, `explore`, `review`) never load hoosh's real prompt
files. They use throwaway hardcoded string literals instead.**

This is a *correctness/maintenance* defect, not a polish item: there are two parallel,
disconnected prompt systems, and the well-crafted one is silently ignored by subagents.

## Where it lives

`AgentType::system_message` in `src/task_management/mod.rs` (lines ~36–60). Each
subagent's entire persona is a string literal baked into a `match`:

```rust
pub fn system_message(&self, task_prompt: &str, budget: Option<&ExecutionBudget>) -> String {
    let base = match self {
        AgentType::Plan => {
            "Analyze the codebase and create an implementation plan. \
        Use available tools to understand existing code patterns. \
        Break the task into specific, ordered steps that can be executed independently. \
        Focus on what needs to change and why."
        }
        AgentType::Explore => {
            "Search the codebase to understand its structure and answer questions. \
        Use file searches to locate relevant code, then examine specific files. \
        Look for patterns, dependencies, and how components interact. \
        Provide concrete findings with file paths and line references.\
        strive to be brief. Providing one result or one brief document should be prefered.
        "
        }
        AgentType::Review => {
            "Conduct thorough code review and quality analysis. \
        Use available tools to examine code for issues and improvements. \
        Focus on: bugs and logic errors, security vulnerabilities, performance issues, \
        code smells and anti-patterns, best practices violations, documentation gaps. \
        Provide specific findings with file paths, line numbers, and actionable recommendations. \
        Prioritize critical issues first."
        }
    };

    let mut message = format!("{}\n\nTask: {}", base, task_prompt);
    // ... budget text appended ...
}
```

That literal is *all* a spawned subagent gets, plus the appended `Task:` and budget text.

## Why it's a defect, not just "thin prompts"

There are **two parallel, disconnected prompt systems** in the repo:

1. **The rich one** — `src/prompts/hoosh_planner.txt`, `hoosh_reviewer.txt`, etc.
   Carefully written personas, registered in `config::DEFAULT_AGENTS`, loaded by
   `AgentDefinitionManager`. These drive the **main** agent.
2. **The throwaway one** — the string literals above. These drive the **subagents**.

So `hoosh_planner.txt` exists and was invested in, but spawning a `plan` subagent does
**not** use that file — it uses the 4-line literal. The good prompt and the
actually-executed prompt have drifted apart, and anyone editing `hoosh_planner.txt`
would not realize the subagent ignores their changes.

### Concrete symptoms

- **`explore` has no prompt file at all** — only the literal exists.
- **Formatting bug**: the Explore literal contains `references.\` immediately followed by
  a real newline + indentation (`\n            `), so raw whitespace leaks into the
  model's prompt. There's also a typo: `prefered` → `preferred`.
- **Maintenance trap**: edits to the `.txt` files silently do not affect subagents.

## The fix (Phase 1 of the improvement plan)

Make `AgentType::system_message` resolve its base text from the **same** prompt-file
mechanism the main agent uses:

- `AgentType::Plan`   → `hoosh_planner.txt`
- `AgentType::Review` → `hoosh_reviewer.txt`
- `AgentType::Explore` → **new** `hoosh_explore.txt`

with the current literal kept only as a fallback if the file is unavailable. Continue
appending `Task:` + budget text exactly as today.

**Result:** one source of truth. Editing a prompt file changes both the main agent and
the corresponding subagent, the explore prompt gets a real home, and the whitespace bug
disappears.

## Caveat before implementing

There are tests asserting substrings against `system_message` (e.g. `"code review"`,
`"Review auth code"`). The new file contents must preserve those substrings, or the
tests must be updated deliberately as part of the change.
