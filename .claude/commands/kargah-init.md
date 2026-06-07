---
description: Set up a project for the kickoff workflow — creates the kickoff/ directory with a brief skeleton and adds task-board artifacts to .gitignore
tags: [kargah, kickoff]
---

This is the one-time bootstrap for a kickoff workflow: a minimal alternative to spec/plan/tasks pipelines that produces seed tasks from a single one-page brief. Run it once per project.

Steps:

1. Verify the current directory is a git repo (`git rev-parse --is-inside-work-tree`). Task boards rely on git worktrees, so no git = no kickoff. Stop and tell the user if missing.

2. Create `kickoff/brief.md` with this exact skeleton (using `write_file`):

   ```markdown
   ## Outcome
   <!-- One paragraph: what does the world look like when this is done? Avoid HOW. -->

   ## Constraints
   <!-- Bullet list: non-negotiables the result must respect. -->

   ## Out of scope
   <!-- Bullet list: what we are deliberately not solving. Cheap drift guard. -->
   ```

   If `kickoff/brief.md` already exists, do not overwrite — tell the user it's already there.

3. Append the following lines to `.gitignore` (read it first, only append entries that aren't already present):

   ```
   .kargah/board.db
   .kargah/board.db-wal
   .kargah/board.db-shm
   .kargah/worktrees/
   .kargah/blobs/
   ```

   Do not gitignore `kickoff/` — briefs are checked in deliberately, so future readers can see how the work was seeded.

4. Print one line: `Kickoff scaffolding ready. Next: /kargah-brief to write the brief.`

Do not create `.kargah/` itself — that gets created later by `/kargah-kickoff`.
