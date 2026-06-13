# Memory work — pickup doc for next session

You are continuing the general-purpose memory feature. Phase A landed in
commit `337b4f5`. Phase B is next. This doc is the cold-start brief.

## Background — read first

- `docs/cc-memory-architecture-vs-hoosh.md` — comparison of CC's memory
  architecture against hoosh, and the phased plan (A → D).
- `docs/additional-gaps-from-cc-source.md` — other CC findings; not the
  immediate focus, but useful context.

**Critical orthogonality** (do not conflate, the user has already
corrected this once): `src/memory_mode/` (`MemoryMode::{Conversation,
Summary}`) is a **context-management** feature for the no-history mode.
General-purpose memory (Phase A onward) is **orthogonal** — persistent
cross-session storage that exists regardless of `MemoryMode`. See
`memory/MEMORY.md` → `project_memory_features_are_orthogonal.md`.

## What's done (Phase A — `337b4f5`)

### Config (`src/config/mod.rs`)
- `AppConfig` and `ProjectConfig` gained `memory_storage:
  Option<ConversationStorageMode>`. Values: `"off" | "local" | "central"`.
- `AppConfig::memory_storage_mode()` falls back to `conversation_storage`
  if memory_storage unset, then to `Off`.
- `AppConfig::memory_storage_root(cwd)` resolves to the on-disk path or
  `None`.
- `ProjectConfig::merge` honors `memory_storage` override.

### Paths (`src/storage/mode.rs`)
- New `resolve_memory_root` mirrors `resolve_storage_root` but uses the
  `memory` subdir. Local → `<cwd>/.hoosh/memory/`. Central →
  `<data_dir>/projects/<encoded-cwd>/memory/`. Off → `None`.

### Memory module (`src/memory/`)
- `entrypoint.rs`: `ENTRYPOINT_NAME = "MEMORY.md"`, caps at 200 lines /
  25 KB. `truncate_entrypoint` line- then byte-truncates, appends a
  warning. `load_entrypoint(memory_root)` reads + truncates.
- `prompt.rs`: `build_memory_prompt(memory_root)` returns the
  system-prompt block (title, dir path, file-format note, index or
  "currently empty" placeholder).

### Session wiring (`src/session.rs`)
- At session start: `config.memory_storage_root(&working_dir)` resolves
  the dir, `create_dir_all` runs (errors swallowed).
- `load_or_create_conversation` accepts `memory_root: Option<&Path>` and
  injects the memory prompt as a system message **for new conversations
  only**. Resumed conversations keep their stored history.

### Tests
- 8 in `memory::tests`, 4 new in `config::mod_tests`. All passing
  (842/842 total at `337b4f5`).

### What Phase A does NOT do
- No `save_memory` tool registered. Prompt mentions it, but the model
  has no way to call it.
- No taxonomy guidance (`user/feedback/project/reference`) in the
  prompt.
- No "what NOT to save" rules.
- No drift caveat ("before recommending from memory").

## Phase B — what you're building

Phase B makes save actually work and gives the model save instructions.
Reading is already covered by existing `read_file`/`grep`/`glob`.

### 1. `SaveMemoryTool` (`src/memory/tool.rs`)

Pattern to copy: `src/memory_mode/tool.rs` (`UpdateSessionFileTool`).
That's the precedent — dedicated scoped-path tool with
`.into_write_safe()` permission.

Tool inputs:
- `name: String` — slug used as filename. Normalize:
  lowercase, spaces → underscores, strip non-`[a-z0-9_-]`. Reject empty.
- `type: String` — one of `"user" | "feedback" | "project" |
  "reference"`. Reject otherwise.
- `description: String` — one-line; goes into frontmatter.
- `body: String` — markdown body.

Tool behavior:
- Resolve memory root via `AppConfig::memory_storage_root` (you need to
  thread access — either via `ToolExecutionContext` or via a fresh
  `AppConfig::load()` like `update_session_file` does for cwd).
- Compute path: `<memory_root>/<normalized_name>.md`. Reject if path
  escapes memory_root (path-traversal guard, even with slug normalization
  in place — defense in depth).
- Render frontmatter + body:
  ```
  ---
  name: {{name}}
  description: {{description}}
  type: {{type}}
  ---

  {{body}}
  ```
- Write file (overwrite OK — updating an existing memory is normal).
- Append/update the entry in `MEMORY.md` (more on this below).
- `.into_write_safe()` for auto-approval.

**MEMORY.md update**: read existing file, look for line matching
`- [` followed by the same `name`. If found, replace that line; else
append. Keep the entry under 200 chars. Format:
`- [{name}](file.md) — {description}` (truncate description if needed).

### 2. Registration gate (`src/cli/agent.rs`)

Copy the pattern at `src/cli/agent.rs:82-84`:

```rust
if config.memory_storage_mode() != ConversationStorageMode::Off {
    let _ = tool_registry.register_tool(Arc::new(SaveMemoryTool::new(...)));
}
```

Off → no tool. Local/Central → tool registered.

### 3. Prompt update (`src/memory/prompt.rs`)

Replace the minimal prompt with the full CC-style block. Steal verbatim
from `../claude-code/src/memdir/memoryTypes.ts`:

- `TYPES_SECTION_INDIVIDUAL` (the 4-type taxonomy, lines 113-178)
- `WHAT_NOT_TO_SAVE_SECTION` (lines 183-195)
- `WHEN_TO_ACCESS_SECTION` (lines 216-222)
- `TRUSTING_RECALL_SECTION` (lines 240-256, "Before recommending from
  memory")

Adapt the "how to save" instructions for `save_memory` (CC tells the
model to use Write/Edit directly; we use a dedicated tool).

Final block order:
1. `# Memory` header + dir path + tool intro
2. `## Types of memory` (4-type taxonomy)
3. `## What NOT to save in memory`
4. `## How to save memories` (one-liner: call `save_memory`)
5. `## When to access memories`
6. `## Before recommending from memory`
7. `## MEMORY.md` index block (existing logic)

### 4. Tests

- Tool: valid save creates file with correct frontmatter, invalid type
  errors, name slugging works, path traversal rejected, MEMORY.md
  updates on save.
- Prompt: all four taxonomy sections present, dir path present, drift
  caveat present, empty vs non-empty index branches work.

## Implementation order (suggested)

1. Build `SaveMemoryTool` first against the existing minimal prompt —
   verify the write path end-to-end.
2. Add the prompt sections incrementally; each section is independent.
3. Gate the tool in `cli/agent.rs`.
4. Tests last? No — each section gets a test as it lands. Phase A
   pattern: 8 tests for the memory module + 4 for config, all green
   before merge.

## Useful precedents in the codebase

- `src/memory_mode/tool.rs:7` — `UpdateSessionFileTool`. Single-purpose
  scoped-path tool with `.into_write_safe()`. Same shape `SaveMemoryTool`
  should have.
- `src/cli/agent.rs:82-84` — gated tool registration pattern.
- `src/memory_mode/mod.rs` — `MemoryModeManager` shows how a memory-style
  module owns its own path resolution and tests.
- `src/prompts/memory_summary_instructions.txt` — terse instruction file
  for the existing summary mode. The general memory prompt sections
  can live in `src/prompts/memory_instructions.txt` if you prefer that
  over inlining in `prompt.rs`.

## Things to watch out for

- **No descriptive comments**. The user has corrected this twice
  (`feedback_no_descriptive_comments.md` in memory). No rustdoc on small
  helpers, no inline restate-the-code comments. Name + signature carries
  meaning.
- **Slug normalization vs path traversal**: even after lowercasing and
  stripping special chars, validate the resolved path is inside
  memory_root via `path.starts_with(memory_root)` after canonicalization.
- **`MemoryMode::Summary` is orthogonal**. Don't accidentally couple the
  memory tool to it.
- **Permission**: `.into_write_safe()` is what makes save frictionless.
  Verify on a real run that no permission prompt appears.

## After Phase B

Phase C is the background extractor fork — tougher because hoosh
doesn't have a fork-agent infrastructure CC-style. Don't start it
without explicit go-ahead. Phase D (LLM relevance recall) is even
further out.

## Live verification target

User runs hoosh on a real OpenRouter-backed config (`anthropic/claude-
sonnet-4.6`). The eventual smoke test (after Phase B): in `--mode
tagged --no-session-persistence` — actually, `--no-session-persistence`
disables conversation storage but should it disable memory? The
`memory_storage` field is independent of `--no-session-persistence`.
Worth asking the user before assuming. CC's `--bare` does turn off
auto-memory; the analog for hoosh is probably to make
`--no-session-persistence` only affect conversations, leaving memory
under its own config. Default: keep them independent.

## Commits since the parity work began (most recent first)

- `337b4f5` Phase A — memory paths + MEMORY.md prompt injection
- `c05b126` OpenAI-compatible reasoning budget wiring (item J)
- `b6c22c6` Anthropic extended thinking wiring (item J)
- `0d2cb9c` BackendConfig.thinking_budget field + TUI styling
- `712088f` Subagent prompt unification + core-instruction additions
- `9e5aa59` FileCompleter (predates this thread)
