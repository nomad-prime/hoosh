---
description: Read the most recent handoff and pick up where the previous session left off
tags: [session, handoff]
---

Look in `.hoosh/handoffs/` (relative to the current working directory) for handoff files. Files are named `handoff_<UTC timestamp>.md`. Pick the one with the most recent timestamp.

Read that file using the `read_file` tool. Treat its contents as your starting context for what we're picking up — goal, decisions, what's done, what's left, gotchas.

After reading, respond briefly with:

> Picked up handoff from `<filename>`. Ready when you are.

Don't re-summarize the handoff back to the user — they wrote it (or asked the previous agent to write it) and don't need to read it twice. Just wait for the next instruction.

If `.hoosh/handoffs/` doesn't exist, or it's empty, say so and ask what to work on instead.
