---
description: Record known facts about the system or domain so future agents don't burn tokens rediscovering them — optional, skip if nothing comes to mind
tags: [kargah, kickoff]
---

This command writes a list of known facts that get loaded into the task board's findings store on init. The point is to avoid having agents rediscover things you could have just told them. **It is optional.**

## The bar for inclusion

Each fact must pass this test: **an agent would otherwise spend a turn discovering this.** If they'd find it instantly from a single file read, it doesn't belong here — let them read the file.

Good facts:
- Architectural choices not obvious from code (`auth uses JWT HS256, key in vault /prod/auth`)
- Historical context (`previous attempt failed because of circular deps in core::session`)
- Cross-cutting constraints (`the v2 API contract in proto/v2/ must stay backward-compatible`)
- Operational details (`releases go through staging, never direct to prod`)

Bad facts (don't include):
- Anything plainly visible in a single file
- Restatements of the brief or rules
- Speculation, "we should probably…", or unverified claims

## Format

Each bullet is one fact. Optional `[topic]` prefix lets a future scheduler inject relevant facts when an agent claims a related task. Topics are short, free-form (`auth`, `db`, `build`, `api`).

```markdown
- [auth] tokens are JWT HS256, 1h TTL, key in vault path /prod/auth/jwt
- [db] postgres 16, schemas in `db/migrations/`, sqitch-managed
- [build] release build is `cargo build --release --features prod`
- previous attempt at this refactor failed because of circular deps in `core::session`
- v2 API contract in `proto/v2/` must stay backward-compatible — customer X depends on it
```

No section headers. No commentary between bullets. Just facts, one per line.

## Conversation

Ask the user: "What does an agent need to know on day one that they couldn't trivially discover? Things like architectural choices, historical attempts, operational quirks. List as many as come to mind."

If they say "nothing comes to mind", skip the file — empty facts is fine. Tell them: "Skipping. Re-run when you notice agents rediscovering something." Stop.

Otherwise, take their list, format each item as a bullet (add a `[topic]` prefix where obvious), write to `kickoff/facts.md`.

## Confirm

Print: `Facts written: <N> entries. Next: /kargah-kickoff to generate seed tasks.`
