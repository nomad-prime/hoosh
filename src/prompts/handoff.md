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

Save the handoff to `.hoosh/handoffs/handoff_<UTC timestamp>.md` (e.g. `.hoosh/handoffs/handoff_20260607_124530.md`) using the `write_file` tool. Create the directory if it doesn't exist.

Once written, confirm only the path. Don't paste the contents back to the user.
