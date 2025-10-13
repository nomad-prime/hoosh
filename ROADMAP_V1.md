# Hoosh v1.0 Roadmap

---

## Critical Missing Features for Self-Coding v1.0

### 1. Multi-Backend Support ⚠️ HIGH PRIORITY

**Current State:** Only Together AI is implemented

**What's Missing:**

- **Anthropic (Claude) backend** - Essential for self-improvement since Claude excels at coding
- **OpenAI backend** - For GPT-4 support
- **Local model support** - Ollama, llama.cpp for offline operation

**Technical Requirements:**

- Implement `src/backends/anthropic.rs` following the `LlmBackend` trait
- Add streaming support for token-by-token responses
- Support Claude's tool use format (similar to OpenAI)
- Configuration via `~/.config/hoosh/config.toml`

---

### 2. Search/Grep Tools ⚠️ HIGH PRIORITY

**Current State:** Not implemented

**What's Missing:**

- `grep_tool` - Search for patterns in files (supports regex)
- `find_tool` - Find files by name/pattern
- `search_and_replace_tool` - Bulk replacements across files

**Why it matters:** For self-coding, the AI needs to search through its own codebase to understand existing patterns,
find similar implementations, and locate where changes need to be made.

**Technical Requirements:**

```rust
// Tool signatures
grep_tool(pattern: String, path: Option<String>, regex: bool) -> Vec<SearchResult>
find_tool(name_pattern: String, path: Option<String>) -> Vec<PathBuf>
search_and_replace_tool(search: String, replace: String, files: Vec<String>) -> ReplaceResult
```

### 3. Conversation Persistence 🔧 MEDIUM PRIORITY

**Current State:** Conversations are lost on exit

**What's Missing:**

- Save/load conversation history to disk
- Resume previous sessions
- Export/import conversations (JSON format)
- List saved conversations
- Delete old conversations
- also allow resetting conversation context
- lets add all of this as commands. Commands start with "/" like in claude code, Command completion should be done like
  file completion with @

**Why it matters:** Complex coding tasks span multiple sessions. The AI needs conversation continuity to remember
context and previous decisions.

**Technical Requirements:**

```rust
struct ConversationStore {
    fn save( & self,
    conversation: &Conversation,
    name: String) -> Result<PathBuf>
    fn load( & self,
    name: &str) -> Result<Conversation>
    fn list( & self ) -> Result<Vec<ConversationInfo> >
    fn delete( & self,
    name: &str) -> Result<() >
}
```

**Storage Location:** `~/.config/hoosh/conversations/`

---

### 4. Enhanced Error Recovery 🔧 MEDIUM PRIORITY

**Current State:** Basic error handling

**What's Missing:**

- Retry logic for failed operations (with exponential backoff)
- Better error messages with suggestions
- Graceful degradation (fallback strategies)
- Undo/rollback capabilities
- Error context preservation

**Why it matters:** When coding itself, hoosh will make mistakes. It needs robust error recovery to learn from failures.

**Technical Requirements:**

```rust
struct ErrorHandler {
    fn with_retry<T>( & self,
    operation: impl Fn() -> Result<T>,
    max_attempts: u32) -> Result<T>
    fn suggest_fix( & self,
    error: &Error) -> Vec<String>
    fn rollback( & self,
    checkpoint: Checkpoint) -> Result<() >
}
```

---

### 5. Multi-Agent System with ACE Orchestration 🔧 HIGH PRIORITY

**Current State:** Basic agent system exists in config (`default_agent`, agents defined in
`~/.config/hoosh/config.toml`)

**Vision:** Implement **ACE (Agentic Context Engineering)** framework from the paper "Agentic Context Engineering:
Evolving Contexts for Self-Improving Language Models" (Zhang et al., 2025). ACE enables self-improving agents through
evolving context playbooks that accumulate and refine strategies over time.

**What's Missing:**

- **Agent toggling UI** - Switch between planning/coding/reviewing agents on-the-fly (like Claude Code's code/plan
  toggle)
- **Default specialized agents** - Pre-configured agents for common roles:
    - `planner` - Breaks down complex tasks into steps (Generator role)
    - `coder` - Implements features and writes code (Generator role)
    - `reviewer` - Reviews code and extracts insights (Reflector role)
    - `debugger` - Analyzes errors and suggests fixes (Generator role)
    - `architect` - Makes design decisions for large changes (Generator role)
    - `orchestrator` - Decides which agent to use and when (Curator role)
- **ACE Framework Components:**
    - **Generator** - Current agent produces reasoning trajectories
    - **Reflector** - Analyzes trajectories to extract lessons/insights (iterative refinement)
    - **Curator** - Creates delta updates to evolving context playbook
- **Evolving Context Playbook** - Comprehensive, structured knowledge base that grows over time:
    - Strategies and hard rules
    - API usage patterns
    - Common mistakes to avoid
    - Domain-specific knowledge
    - Code snippets and formulas
    - Verification checklists
- **Incremental Delta Updates** - Add/update/remove specific bullets instead of rewriting entire context
- **Grow-and-Refine Mechanism** - Balance context expansion with deduplication and pruning
- **Agent Orchestration Modes:**
    - `manual` - User switches agents with `/agent` commands
    - `automatic` - AI decides when to switch agents based on task state
    - `hybrid` - AI suggests switches, user confirms
- **Agent context handoff** - Pass state/context when switching between agents
- **Multi-epoch adaptation** - Refine playbook over multiple passes on same task

**Why it matters:** Complex self-modifications require different thinking modes. Planning needs high-level reasoning,
coding needs attention to detail, reviewing needs critical analysis. ACE's approach allows:

- Self-improvement without labeled data (learns from execution feedback)
- Prevents "context collapse" where knowledge degrades over iterations
- Avoids "brevity bias" - maintains detailed domain knowledge instead of generic summaries
- 86.9% lower adaptation latency vs. alternatives
- Matches production-level agents using smaller open-source models

**Key Results from Paper:**

- +10.6% improvement on agent benchmarks (AppWorld)
- +8.6% improvement on domain-specific benchmarks
- Significantly lower cost and latency than alternatives

**Technical Requirements:**

See detailed implementation in [`ACE_ORCHESTRATION.md`](./ACE_ORCHESTRATION.md) which includes:

```rust
// Core ACE Framework
struct AceManager {
    current_agent: String,
    agents: HashMap<String, Agent>,
    playbook: Playbook,  // Evolving context
    orchestrator: Option<OrchestratorAgent>,

    // Three roles with separate backends
    generator_backend: Box<dyn LlmBackend>,
    reflector_backend: Box<dyn LlmBackend>,
    curator_backend: Box<dyn LlmBackend>,

    fn execute_task( & mut self,
    task: &str) -> Result<TaskResult>
    fn switch_agent( & mut self,
    agent_name: &str,
    handoff: AgentHandoff) -> Result<() >
}

// Evolving Context Playbook
struct Playbook {
    bullets: HashMap<BulletSection, Vec<Bullet>>,  // Organized by section
    metadata: PlaybookMetadata,
}

struct Bullet {
    id: String,
    section: BulletSection,
    content: String,  // Strategy, insight, code snippet, etc.
    metadata: BulletMetadata,  // helpful_count, harmful_count, timestamps
    tags: Vec<String>,
}

// Delta Updates (incremental, not monolithic rewrites)
struct DeltaUpdate {
    operations: Vec<Operation>,  // Add, Update, Remove bullets
    reasoning: String,
}
```

**Configuration Format (`~/.config/hoosh/config.toml`):**

```toml
default_agent = "coder"

[orchestration]
mode = "manual"  # "manual" | "automatic" | "hybrid"
reflector_backend = "anthropic"
curator_backend = "anthropic"
max_context_tokens = 100000
enable_grow_and_refine = true

[agents.planner]
file = "planner.txt"
description = "Breaks down complex tasks into actionable steps"
role = "generator"
tags = ["planning", "architecture"]
allowed_tools = ["read_file", "list_files", "grep_tool"]

[agents.reviewer]
file = "reviewer.txt"
description = "Reviews code and extracts insights"
role = "reflector"  # Acts as Reflector in ACE
tags = ["review", "quality"]

[agents.orchestrator]
file = "orchestrator.txt"
description = "Decides which agent to use and when"
role = "curator"  # Acts as Curator in ACE
tags = ["meta", "coordination"]
```

**Commands:**

- `/agents` - List all available agents with descriptions
- `/agent <name>` - Switch to specific agent
- `/toggle` - Cycle to next agent
- `/orchestrate` - Toggle automatic orchestration mode
- `/playbook` - Show playbook statistics
- `/playbook export` - Export playbook to file
- `/playbook import` - Import playbook from file

**UI Enhancement:**

- **Agent indicator:** Show current agent in status bar (e.g., `[planner]`, `[coder]`)
- **Visual feedback:** When agent switches, show transition message:
  ```
  🔄 Switching from [coder] to [reviewer]
  Context: Reviewing changes made in previous steps...
  ```
- **Playbook stats:** Show playbook size, version, and sections in status bar

### 6. Safety Guardrails for Self-Modification ⚠️ CRITICAL

**Current State:** Basic permission system exists

**What's Missing:**

- Backup/checkpoint before major changes
- Dry-run mode for proposed changes
- Review system for AI-generated commits
- Rollback mechanisms
- "Critical file" protection (don't modify core without explicit approval)
- Change impact analysis

**Why it matters:** Self-modification is dangerous. Hoosh needs strong safety nets to avoid breaking itself.

**Technical Requirements:**

```rust
struct SafetyGuard {
    critical_files: Vec<PathBuf>, // main.rs, lib.rs, etc.

    fn create_checkpoint( & self ) -> Result<Checkpoint>
    fn dry_run( & self,
    operations: Vec<Operation>) -> DryRunResult
    fn analyze_impact( & self,
    changes: Vec<FileChange>) -> ImpactAnalysis
    fn require_review( & self,
    change: &Change) -> bool
}
```

**Critical Files (require extra approval):**

- `src/main.rs`
- `src/lib.rs`
- `src/backends/mod.rs`
- `Cargo.toml`

---

### 7. Multi-file Operations 🔧 MEDIUM PRIORITY

**Current State:** Tools work on single files

**What's Missing:**

- Batch operations across multiple files
- Refactoring tools (rename symbol across files)
- Multi-file diff viewing
- Project-wide search and replace

**Technical Requirements:**

```rust
struct MultiFileOp {
    fn rename_symbol( & self,
    old_name: &str,
    new_name: &str) -> Result<Vec<FileChange> >
    fn batch_edit( & self,
    edits: Vec<FileEdit>) -> Result<() >
    fn multi_diff( & self,
    files: Vec<PathBuf>) -> Result<String>
}
```
