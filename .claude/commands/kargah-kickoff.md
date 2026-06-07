---
description: Generate seed tasks from kickoff/ artifacts and write them as JSON — the single LLM call that replaces a spec→clarify→plan→tasks pipeline
tags: [kargah, kickoff]
---

This is the terminal step of kickoff. It reads the brief (and optional rules + facts) and produces a JSON array of seed tasks ready for a task board. **One LLM call, one JSON output, done.** No clarify pass. No plan synthesis. No tasks-from-spec conversion.

> Note: the `kargah` CLI doesn't exist yet. For now, this command writes `.kargah/seeds.json` — the same JSON shape `kargah init` will eventually ingest. Until then it's a checked artifact you can read, edit, and feed to whatever orchestrates the work.

## Read the kickoff bundle

1. Read `kickoff/brief.md`. **Required.** If missing, stop and tell the user to run `/kargah-init` then `/kargah-brief`.
2. Read `kickoff/constitution.md` if it exists. If not, use `(none — defer to project AGENTS.md)`.
3. Read `kickoff/facts.md` if it exists. If not, use `(none)`.

## Generate seed tasks

Apply this prompt to yourself — produce the output directly, don't ask another model:

```
BRIEF:
<brief.md contents>

RULES:
<constitution.md contents, or "(none — defer to project AGENTS.md)">

KNOWN FACTS:
<facts.md contents, or "(none)">

Produce 5-10 seed tasks as a JSON array. Each task:
{
  "id":           "<short-kebab-case-slug, unique within this array>",
  "goal":         "<one line, outcome-shaped>",
  "acceptance":   "<how the claimer knows it's done>",
  "scope":        "<paths, modules, or 'research only'>",
  "claimable_by": "agent" | "human" | "any",
  "blocked_by":   []
}

Rules:
- One task per outcome sentence in the brief, max.
- One verify task per constraint in the brief.
- Mark a task `human` only when a decision is required.
- Do not invent work outside the brief.
- If a constraint is already covered by a fact, skip the verify task.
- No user stories. No priorities. No phases. Just tasks.
- `blocked_by` uses the `id` field of another task in the same array.
- Return JSON only, no prose, no markdown fence.
```

## Write the output

Create `.kargah/` if it doesn't exist (`mkdir -p .kargah`). Write the JSON array to `.kargah/seeds.json` with `write_file`, 2-space indentation, valid JSON (no trailing commas, no comments).

## Confirm

Print one line: `Seeded <N> tasks → .kargah/seeds.json. Review and edit before running the board (once kargah ships).`

Do not paste the JSON back to the user. They can open the file.
