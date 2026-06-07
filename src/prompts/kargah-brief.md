---
description: Write a one-page brief describing the desired outcome — the single immutable artifact that seeds the task list, replacing spec/plan/clarify pipelines
tags: [kargah, kickoff]
---

This command helps the user write a brief: the smallest description of intent that produces useful seed tasks. It replaces multi-stage spec/plan/clarify/tasks pipelines with one immutable file.

The brief is the only mandatory kickoff artifact. Once seeded, it does not change — new requirements become new tasks, not brief edits.

## Read the current state

1. Read `kickoff/brief.md`. If it doesn't exist, run `/kargah-init` first — tell the user and stop.
2. If the file exists but only has the skeleton headers, treat this as fresh authoring.

## Discipline (load-bearing)

The brief is **one screen, three sections**. Do not let it grow. If you find yourself writing user stories, priorities, acceptance criteria per requirement, or success metrics — stop. Those don't belong here. The brief is the seed; tasks carry the rest.

- **Outcome** — one paragraph. What the world looks like when done. Avoid HOW. No tech choices.
- **Constraints** — bullet list. Non-negotiables the result must respect. More than 5 bullets means you're mixing in design preferences — push those out.
- **Out of scope** — bullet list. What is deliberately not solved here. Cheap drift guard.

Explicitly reject these speckit-shaped patterns:
- No FR-### IDs (tasks have IDs; requirements become tasks)
- No P1/P2/P3 priorities (priority is claim-order, set on the board, not here)
- No "as a user, I want…" stories (the brief describes outcome, not journeys)
- No measurable success criteria (acceptance lives per-task, not in the brief)

## Conversation

Ask the user only the questions they haven't already answered in their initial prompt, in this order:

1. "In one paragraph, what does done look like?" — keep them outcome-focused; redirect if they answer with implementation
2. "What's non-negotiable about the result?" — push for 1-5 bullets
3. "What's deliberately out of scope?" — push for 1-3 bullets

If they gave you the whole brief in their first message, skip straight to writing.

## Write

Write the final brief to `kickoff/brief.md` using `write_file`. Use exactly this structure, nothing else:

```markdown
## Outcome
<one paragraph>

## Constraints
- <bullet>
- <bullet>

## Out of scope
- <bullet>
- <bullet>
```

No comments, no preamble, no metadata.

## Confirm

Print: `Brief written to kickoff/brief.md. Next: /kargah-constitution (optional) or /kargah-kickoff to generate seed tasks.`
