# How Claude Code manages memory — vs hoosh

Sourced from `../claude-code/src/memdir/`, `src/services/extractMemories/`,
`src/services/SessionMemory/`. Cross-referenced against hoosh's
`src/memory_mode/` (the only memory surface hoosh has today).

## TL;DR

CC ships **two distinct, complementary memory systems**:

1. **memdir** — persistent typed memory store, *across sessions*, written by
   a background fork agent, recalled by an LLM relevance selector.
2. **SessionMemory** — single rigid-structure markdown file that
   *supplements* the live transcript, used as a compaction-survival
   hand-off so the agent can pick up after context compression.

**Hoosh ships neither.** It has zero general-purpose memory — no cross-
session store, no compaction-survival hand-off. The user's corrections,
preferences, and project context vanish at the end of every `hoosh`
invocation unless `--resume` brings back the literal transcript.

### What hoosh's `MemoryMode` is NOT

`src/memory_mode/` and CC's memory systems are **orthogonal**.
`MemoryMode::{Conversation, Summary}` is a **context-management** feature:
it controls what the agent sees as its turn input.

- `Conversation` (default): agent sees the full prior transcript.
- `Summary`: prior transcript is *dropped* each turn (`clear_turn_history`);
  the agent maintains a single `summary.txt` as its only continuity
  mechanism *because it has no transcript to fall back on*.

CC's SessionMemory does the opposite — it sits **alongside** the full
transcript as a supplement, only earning its keep when compaction kicks in.
The two solve different problems and should not be compared item-for-item.

For the rest of this doc, `MemoryMode::Summary` is treated as out of scope.
The gap analysis is about general-purpose memory: persistent cross-session
storage and compaction-survival hand-offs. Hoosh has neither, regardless of
which `MemoryMode` is active.

## CC architecture at a glance

| Layer | Where | Purpose |
|---|---|---|
| **memdir** path resolution | `memdir/paths.ts` | `~/.claude/projects/<canonical-git-root>/memory/` with env + settings overrides; tilde-expansion validated against path-traversal |
| **memdir** prompt builders | `memdir/memdir.ts` | Injects the "what is memory / how to save / when to access" sections into the system prompt, plus `MEMORY.md` content (line+byte capped, warning appended) |
| **memdir** type taxonomy | `memdir/memoryTypes.ts` | Closed 4-type set: `user`, `feedback`, `project`, `reference` — each with `<description>`, `<when_to_save>`, `<how_to_use>`, `<body_structure>`, `<examples>` |
| **memdir** scan | `memdir/memoryScan.ts` | Walks the dir, reads frontmatter only (first 30 lines), sorts by mtime, caps at 200 files |
| **memdir** recall | `memdir/findRelevantMemories.ts` | At query time: scans, runs a Sonnet sidecall to pick up to 5 relevant memories by description match, excludes already-surfaced and tool-reference memories for actively-used tools |
| **extractMemories** | `services/extractMemories/extractMemories.ts` | Runs at end of each turn (when the model produces a final response with no tool calls). Perfect fork of the main conversation — shares prompt cache. 5-turn hard cap. Cursor (UUID) advances after success so each run only sees new messages. |
| **extractMemories** permissions | `createAutoMemCanUseTool` | Read/Grep/Glob unrestricted, Bash only if `isReadOnly`, Write/Edit only for paths matching `isAutoMemPath`. Everything else denied. |
| **extractMemories** mutual exclusion | `hasMemoryWritesSince` | If the main agent already wrote to a memory path this turn, the fork skips — the main agent's prompt has full save instructions; the fork is the safety net |
| **SessionMemory** | `services/SessionMemory/sessionMemory.ts` | Separate rigid-template markdown file maintained by a *different* forked subagent. Sections include: Session Title, Current State, Task Specification, Files and Functions, Workflow, Errors & Corrections, Codebase Documentation, Learnings, Key Results, Worklog. Periodic update, not per-turn. |

## Key design choices worth stealing

### 1. Persistent + per-project + git-root-canonicalized
Path is `~/.claude/projects/<sanitized-git-root>/memory/`. All worktrees of
the same repo share one directory (PR #24382). Survives across hoosh
invocations from any cwd inside the repo.

### 2. Four-type taxonomy with explicit "do NOT save" rules
Memories are categorized; categories drive *when* to save and *how* to format
the body. The taxonomy explicitly excludes code patterns, architecture, git
history, fix recipes, and CLAUDE.md content — that prevents the model from
saving derivable noise. Even on explicit "save this" requests, the prompt
deflects to "what was surprising or non-obvious about it?" (eval-validated:
0/2 → 3/3).

### 3. Background extractor with mutual exclusion
The model's main prompt has the save instructions, but a forked agent runs
at turn end as a safety net. The fork's tool registry is restricted via
`canUseTool` to read-anywhere + write-only-inside-memory-dir. The fork is
skipped if the main agent already saved (cursor advances).

Why this matters: it stops the failure mode where the model "forgets" to save
because the task ran long and the save instruction fell out of attention. The
fork is a separate context window dedicated to one job.

### 4. Recall is not "load everything"
At query time, `findRelevantMemories` runs a sidecall asking Sonnet to pick
up to 5 relevant files by description. The full bodies are then loaded.
`MEMORY.md` (the index) is *always* loaded, capped at 200 lines / 25KB with
truncation warning that tells the model how to fix it ("keep entries to one
line under ~200 chars; move detail into topic files").

### 5. "Before recommending from memory" drift caveat
A whole prompt section teaches the model that a memory naming a specific
function/file/flag is a claim about what existed *when the memory was
written*. Before recommending it: check the file exists, grep for the
function, treat repo-state summaries as frozen. Eval-validated (0/3 → 3/3
when surfaced as its own section vs. buried as a bullet).

### 6. Memory dir creation is the harness's job
`ensureMemoryDirExists` runs once per session, so the model's prompt can
truthfully say "This directory already exists — write to it directly, don't
run `mkdir` or check for existence." Saves real turns the model would
otherwise burn on `ls`/`mkdir -p`.

### 7. Manifest pre-injection on extraction
Before the extraction fork runs, the manifest of existing memory files (path
+ description + mtime + type) is pre-injected into the user prompt. Saves
the fork from burning a turn on `ls`. Same scan also powers recall.

### 8. Cache safety on fork
The fork shares the parent's prompt cache via `createCacheSafeParams`. Tool
lists must match the parent's — that's why the extractor uses
`canUseTool` to deny tools at execution time rather than removing them
from the registry (removing would invalidate the cache).

## Hoosh today

Nothing. There is no persistent memory directory, no MEMORY.md, no
extractor, no recall, no compaction-survival hand-off, no taxonomy, no
drift caveats. The system prompt makes no claim about memory existing.

`src/memory_mode/` exists but solves a different problem (see the
"What hoosh's `MemoryMode` is NOT" section above) and is treated as
out of scope here.

## Gaps, in order of leverage

| # | Gap | Effort | Why it matters |
|---|---|---|---|
| **1** | **No persistent cross-session memory** | medium | Biggest one. The user's "stop summarizing what you did" preference from yesterday is forgotten today. Project context, deadlines, dashboards — all gone every session. This is the single thing CC's memory system buys that hoosh can't replicate without it. |
| **2** | **No background extractor safety net** | medium | Even if hoosh adds memdir, relying on the main agent to remember to save means missed saves under task pressure. A turn-end fork with restricted writes catches what the main agent dropped. |
| **3** | **No 4-type taxonomy** | small (prompt + frontmatter) | Carving observations into `user/feedback/project/reference` makes recall sharper and makes "what NOT to save" enforceable. Pure structure, no infrastructure. |
| **4** | **No relevance recall** | medium (LLM sidecall) | Once #1 lands, you can't dump every memory into every prompt. CC's per-query Sonnet sidecall picks ≤5; without it the memdir prompt would grow unbounded. |
| **5** | **No compaction-survival hand-off** | medium | When the conversation gets compacted (or grows past a context limit), CC's SessionMemory gives the post-compaction agent a structured hand-off file (Goal / Current State / Files / Workflow / Errors / Next). Hoosh today has no equivalent — once context is dropped, structured continuity is lost. Note this is *also* what `MemoryMode::Summary` solves, but for a different trigger (no-history mode, not compaction). |
| **6** | **No "before recommending from memory" drift caveat** | trivial (prompt only) | A few lines in core instructions, eval-validated by CC. Stops the model from confidently recommending files/functions that were renamed since the memory was written. |
| **7** | **No `omitClaudeMd`-style scope control** | small | Once hoosh has memdir, deciding which agents/subagents read memory is a design choice. CC gates per-agent. Plan/Explore subagents probably don't need it. |

## Recommended sequence (if pursued)

A real implementation lands in three phases, not one:

**Phase A — schema and paths** (small):
- Add a `~/.config/hoosh/memory/<canonical-git-root>/` directory (mirror CC's
  per-git-root scoping; canonicalize via the same logic so worktrees share).
- Define the 4-type frontmatter (`name`, `description`, `type`).
- Define `MEMORY.md` index format with line + byte caps.
- Wire `loadMemoryPrompt()` into the system prompt construction so the model
  is told the directory exists and what to do with it.

**Phase B — main-agent save** (small):
- Extend the system prompt with CC's `TYPES_SECTION_INDIVIDUAL` (single-dir
  variant). Steal verbatim; the eval work behind it isn't reproducible in
  hoosh's timeframe.
- The model writes via existing `write_file`/`edit_file` tools; no new tools
  needed.
- Recall is **read the entrypoint + grep on demand** — no LLM sidecall yet.

**Phase C — background extractor** (medium):
- Forked subagent at turn end, restricted permissions, 5-turn cap.
- Mutual exclusion against direct main-agent writes.
- Skip if `--no-session-persistence` (the parity-doc tier-3.1 flag).

**Phase D — relevance recall** (medium):
- LLM sidecall picks ≤5 by description match. Only worth it once #C creates
  enough memories for "dump everything" to be a real cost.

Phases A + B are the wedge — they alone make hoosh competitive on
cross-session continuity. Phases C + D are polish that pays off at scale.

## Considered and skipped

- **Team memory** (CC's TEAMMEM): private/team `<scope>` tags, shared memdir
  via secret scanning + watcher. Useful when multiple humans share a repo;
  hoosh's threat model and user-base don't motivate it yet.
- **KAIROS daily-log mode**: append-only date-named log files distilled
  nightly. Only relevant for long-lived assistant sessions (CC's autonomous
  mode). Hoosh has no equivalent execution model.
- **`MEMORY_SHAPE_TELEMETRY`**: CC logs memory recall hit/miss to tune the
  selector prompt. hoosh has no telemetry pipeline; cargo-culting analytics
  events would be dead code.
- **`autoDream` / `/dream` / `/remember` skills**: CC's user-facing memory
  controls. Adding these makes sense after Phase B; before that there's
  nothing to dream over.
