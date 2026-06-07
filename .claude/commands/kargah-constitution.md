---
description: Write short, board-specific rules of engagement when this work needs guardrails the project's AGENTS.md doesn't already cover — usually skipped
tags: [kargah, kickoff]
---

This command writes a short rules file that every worker on this task board respects. **It is optional and usually skipped.** Most work inherits the project's existing `AGENTS.md` / `CLAUDE.md` as its rules of engagement and does not need a separate file.

## Decide if it's needed

Ask the user: "Does this work need rules the project's AGENTS.md doesn't already cover? Examples: a research spike with extra guardrails, a customer-specific build, a refactor where the usual conventions are deliberately relaxed."

If the answer is no, or they're unsure, **don't create the file**. Tell them: "Skipping — agents will inherit project AGENTS.md. Re-run if you find you need board-specific rules." Stop.

## If yes, author it

Read `kickoff/brief.md` for context — rules should relate to the brief, not invent unrelated policy.

The file is **a handful of bullets, no prose**. Three sections, all optional:

```markdown
## Rules
- <one rule per bullet, present tense, testable>

## Forbidden
- <what agents must never do on this board>

## Defer to humans
- <decisions that get posted back to humans, not agent-decided>
```

Hard cap: **20 lines total.** If you're writing more, you're writing a treatise — that's the speckit failure mode we're rejecting. No rationales, no version numbers, no amendment process, no "principles", no Roman numerals. Just rules.

## Write and confirm

Write to `kickoff/constitution.md` with `write_file`. Print: `Rules written. Next: /kargah-facts (optional) or /kargah-kickoff to generate seed tasks.`
