---
description: Write a session handoff so a future agent can pick up where this one left off
tags: [session, handoff]
---

You're handing this session off — to a future you, or to a fresh agent. Write a clean handoff that someone with no memory of this conversation could pick up cold.

Cover:
- **Goal** — what we were trying to accomplish, in one or two sentences
- **Decisions** — key choices made and *why*, especially anything non-obvious that would be re-litigated otherwise
- **Done** — concrete: file paths edited, functions added, commands run, commits made
- **Left** — what's outstanding, ordered by priority
- **Gotchas** — constraints, conventions, things a fresh agent would miss (project quirks, things that look wrong but are right, etc.)
- **Key paths** — file paths and line numbers worth jumping to immediately

Steps:

1. Get the current UTC timestamp by running `date -u +%Y%m%d_%H%M%S` via the `bash` tool. Don't guess or use a placeholder — you can't know the time without checking.
2. Create the handoffs directory if it doesn't exist: `mkdir -p .hoosh/handoffs`.
3. Write the handoff to `.hoosh/handoffs/handoff_<that timestamp>.md` using the `write_file` tool.

Once written, confirm only the path. Don't paste the contents back to the user.
