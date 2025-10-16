# Hoosh v1.0 Roadmap

### 0. Commands and Command Completion System 🔧 Missing pieces

- **@ Mention System** - Reference files, symbols, and context
    - `@src/` - Reference a directory (needs extension)
    - `@symbol_name` - Reference a function/struct/symbol (Post-v1, needs LSP)
    - `@conversation` - Reference previous conversation (needs implementation)
    - Preview on hover/selection (nice-to-have)

- **Command Parser & Registry** (see src/commands/)
    - Command history (up/down arrows) - not yet implemented
    - Command chaining (e.g., `/save && /clear`) - not yet implemented

- **Interactive Command Mode**
    - Multi-line command input - not needed for commands
    - Command prompt with syntax highlighting - nice-to-have

**What's Still Missing (Additional Commands):**

- `/save [name]` - Save current conversation
- `/load <name>` - Load a saved conversation
- `/list` - List saved conversations
- `/delete <name>` - Delete a conversation
- `/reset` - Reset conversation context (similar to /clear but preserves history)
- `/config` - Show/edit configuration
- `/agent <name>` - Switch to specific agent
- `/toggle` - Toggle between agents
- `/export [format]` - Export conversation (JSON, markdown, etc.)
- `/undo` - Undo last operation
- `/redo` - Redo undone operation

**Why it matters:** Commands provide a structured way to interact with the system, making it more user-friendly and
efficient. This is foundational for all other features - conversation management, agent switching, configuration, etc.

**Technical Implementation:** ✅ **COMPLETE**

The command system is implemented across these modules:

- **`src/commands/registry.rs`** - Command trait, CommandRegistry, and CommandContext
- **`src/commands/commands.rs`** - Default command implementations (help, clear, status, tools, agents, exit)
- **`src/tui/completion/mod.rs`** - Completer trait (unified interface for all completers)
- **`src/tui/completion/file_completer.rs`** - File completion with @ trigger
- **`src/tui/completion/command_completer.rs`** - Command completion with / trigger
- **`src/tui/actions.rs`** - Command execution logic
- **`src/tui/event_loop.rs`** - Event handling for commands and completions
- **`src/tui/input_handlers.rs`** - Keyboard input handling for commands and completions

```rust
// Implemented Completer trait from src/tui/completion/mod.rs
#[async_trait]
pub trait Completer: Send + Sync {
    fn trigger_key(&self) -> char;
    async fn get_completions(&self, query: &str) -> Result<Vec<String>>;
    fn format_completion(&self, item: &str) -> String;
    fn apply_completion(&self, input: &str, trigger_pos: usize, completion: &str) -> String;
}

// Implemented CommandRegistry from src/commands/registry.rs
pub struct CommandRegistry {
    commands: HashMap<String, Arc<dyn Command>>,
    aliases: HashMap<String, String>,
}

impl CommandRegistry {
    pub fn register(&mut self, command: Arc<dyn Command>) -> Result<()>
    pub async fn execute(&self, input: &str, context: &mut CommandContext) -> Result<CommandResult>
    pub fn get_help(&self, command_name: Option<&str>) -> String
    pub fn list_commands(&self) -> Vec<(&str, &str)>
}

// Implemented Command trait from src/commands/registry.rs
#[async_trait]
pub trait Command: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn aliases(&self) -> Vec<&str>;
    fn usage(&self) -> &str;
    async fn execute(&self, args: Vec<String>, context: &mut CommandContext) -> Result<CommandResult>;
}
```

**Architecture Note:** All completers implement the same `Completer` trait from `src/tui/completion/mod.rs`.
This provides a unified, extensible interface for all completion types:

- ✅ `FileCompleter` - `@` trigger for file paths (implemented in src/tui/completion/file_completer.rs)
- ✅ `CommandCompleter` - `/` trigger for slash commands (implemented in src/tui/completion/command_completer.rs)
- `SymbolCompleter` - `@` trigger for code symbols (Post-v1, needs LSP integration)

**Storage Location:**

- Command history: `~/.config/hoosh/command_history` (not yet implemented)
- Command aliases: Built into Command trait implementation (configurable via code)
- Saved conversations: `~/.config/hoosh/conversations/` (for future /save, /load commands)

**UI Status:**

- ✅ Command detection and routing (starts with '/')
- ✅ Inline command suggestions (completion dialog with up/down navigation)
- ✅ Visual feedback for command execution (via AgentEvent system)
- Status bar showing current mode - not yet implemented
- Command palette (Ctrl+P / Cmd+P) - not needed (use / trigger instead)
- Syntax highlighting for commands and mentions - nice-to-have

---

### 1. Web Search Tool 🔧 HIGH PRIORITY

**Current State:** No web search capability
**What's Missing:**

- Integrate a web search API (e.g., Bing Search API, Google Custom Search)
- Tool to fetch and summarize search results
- Rate limiting and caching of search results
- Handle ambiguous queries
- Support for multiple search engines
- Configurable search depth (number of results to fetch)
- Ability to cite sources in responses
- Option to disable web search for privacy
- Search query refinement based on conversation context
- Summarization of long articles
- Extract key points from search results
- Filter results by date or relevance
- Handle pagination for search results
- Support for different content types (news, images, videos)
- Allow user to specify preferred search engine in config

**Why it matters:** Many coding tasks require up-to-date information. Web search allows the AI to access current
documentation, libraries, and best practices.

### 2. Conversation Persistence 🔧 MEDIUM PRIORITY

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

### 3. Enhanced Error Recovery 🔧 MEDIUM PRIORITY

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

### 4. Multi-Agent System with ACE Orchestration 🔧 HIGH PRIORITY

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

---

### 6. Multi-file Operations 🔧 MEDIUM PRIORITY

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

---

### 7. MCP (Model Context Protocol) Support 🔧 CRITICAL

**Current State:** No MCP support

**What's Missing:**

- **MCP Server Integration**
    - Connect to MCP servers for extended capabilities
    - Support for stdio, HTTP, and WebSocket transports
    - Server lifecycle management (start, stop, restart)
    - Server discovery and registration

- **MCP Tools**
    - Dynamic tool registration from MCP servers
    - Tool schema validation and conversion
    - Tool execution with proper error handling
    - Tool permissions and sandboxing

- **MCP Resources**
    - Access to remote resources (files, databases, APIs)
    - Resource caching and invalidation
    - Resource permissions

- **MCP Prompts**
    - Import prompts from MCP servers
    - Prompt templates and composition
    - Dynamic prompt generation

- **Standard MCP Servers**
    - File system server (enhanced file operations)
    - Git server (repository operations)
    - Database server (SQL queries)
    - Web server (HTTP requests, web scraping)
    - Slack server (team communication)
    - GitHub server (issues, PRs, etc.)

**Why it matters:** MCP is the standard for extending AI assistants with custom tools and integrations. This enables
hoosh to integrate with any MCP-compatible service and significantly extends its capabilities without modifying core
code.

**Technical Requirements:**

```rust
struct McpClient {
    servers: HashMap<String, McpServer>,

    fn connect( & mut self,
    config: McpServerConfig) -> Result<String>
    fn disconnect( & mut self,
    server_id: &str) -> Result<() >
    fn list_tools( & self,
    server_id: &str) -> Result<Vec<ToolSchema> >
    fn execute_tool( & self,
    server_id: &str,
    tool: &str,
    args: Value) -> Result<Value>
    fn list_resources( & self,
    server_id: &str) -> Result<Vec<Resource> >
    fn read_resource( & self,
    server_id: &str,
    uri: &str) -> Result<String>
}

struct McpServer {
    id: String,
    name: String,
    transport: Transport,
    capabilities: ServerCapabilities,
}
```

**Configuration:**

```toml
[[mcp.servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/workspace"]

[[mcp.servers]]
name = "github"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_TOKEN = "${GITHUB_TOKEN}" }
```

---

### 8. LSP (Language Server Protocol) Integration 🔧 LOW PRIORITY (Post-v1)

**Current State:** No LSP support, limited code intelligence
**Note:** Not required for v1 - can be added in future releases

**What's Missing:**

- **LSP Client Implementation**
    - Connect to language servers for various languages
    - Support for common LSP features:
        - Go to definition
        - Find references
        - Hover information
        - Code completion suggestions
        - Diagnostics (errors, warnings)
        - Symbol search (workspace-wide)
        - Rename symbol
        - Code actions (quick fixes)
        - Document formatting

- **Multi-language Support**
    - Rust (rust-analyzer)
    - Python (pyright, pylsp)
    - JavaScript/TypeScript (typescript-language-server)
    - Go (gopls)
    - Java (jdtls)
    - C/C++ (clangd)
    - Auto-detect language from file extension

- **Code Intelligence Features**
    - Semantic symbol search (find all usages of a function)
    - Type information and signatures
    - Documentation on hover
    - Import management
    - Code navigation breadcrumbs

- **Integration with Tools**
    - Enhance @ mentions with symbol search
    - Use diagnostics to guide error fixing
    - Suggest code actions when editing

**Why it matters:** LSP provides deep code understanding that enables intelligent refactoring, navigation, and error
detection. This makes hoosh significantly more powerful for coding tasks.

**Technical Requirements:**

```rust
struct LspClient {
    servers: HashMap<String, LanguageServer>,

    fn start_server( & mut self,
    language: &str,
    root_path: PathBuf) -> Result<() >
    fn goto_definition( & self,
    file: &Path,
    position: Position) -> Result<Location>
    fn find_references( & self,
    file: &Path,
    position: Position) -> Result<Vec<Location> >
    fn hover( & self,
    file: &Path,
    position: Position) -> Result<String>
    fn diagnostics( & self,
    file: &Path) -> Result<Vec<Diagnostic> >
    fn symbols( & self,
    workspace: bool) -> Result<Vec<Symbol> >
    fn rename( & self,
    file: &Path,
    position: Position,
    new_name: String) -> Result<WorkspaceEdit>
}

struct LanguageServer {
    language: String,
    process: Child,
    capabilities: ServerCapabilities,
}
```

**Configuration:**

```toml
[lsp.rust]
command = "rust-analyzer"
args = []

[lsp.python]
command = "pyright-langserver"
args = ["--stdio"]

[lsp.typescript]
command = "typescript-language-server"
args = ["--stdio"]
```

---

### 9. Project/Codebase Indexing and Understanding 🔧 LOW PRIORITY (Post-v1)

**Current State:** No project indexing, limited codebase understanding
**Note:** Not required for v1 - can be added in future releases

**What's Missing:**

- **Codebase Indexing**
    - Build AST (Abstract Syntax Tree) for source files
    - Index symbols (functions, classes, variables)
    - Track dependencies and imports
    - Build call graph
    - Detect project structure and conventions

- **Semantic Search**
    - Search by concept, not just keywords
    - Find similar code patterns
    - Detect code duplication
    - Identify related files

- **Project Analysis**
    - Detect frameworks and libraries used
    - Identify project architecture (MVC, microservices, etc.)
    - Find entry points and configuration files
    - Detect test files and test frameworks
    - Understand build system (Cargo, npm, Maven, etc.)

- **Context Building**
    - Auto-include relevant files in context
    - Smart file selection based on task
    - Detect which files need to be modified for a feature
    - Track file relationships and dependencies

**Why it matters:** Understanding the entire codebase enables better suggestions, refactoring, and feature development.
The AI can make informed decisions about where to make changes.

**Technical Requirements:**

```rust
struct ProjectIndex {
    root: PathBuf,
    symbols: HashMap<String, Vec<Symbol>>,
    dependencies: DependencyGraph,
    file_tree: FileTree,

    fn build_index( & mut self ) -> Result<() >
    fn search_symbols( & self,
    query: &str) -> Result<Vec<SymbolMatch> >
    fn find_related_files( & self,
    file: &Path) -> Result<Vec<PathBuf> >
    fn get_dependencies( & self,
    file: &Path) -> Result<Vec<Dependency> >
    fn suggest_context( & self,
    task: &str) -> Result<Vec<PathBuf> >
}

struct Symbol {
    name: String,
    kind: SymbolKind, // Function, Class, Variable, etc.
    location: Location,
    signature: Option<String>,
    documentation: Option<String>,
}
```

---

### 10. Performance Monitoring and Cost Tracking 🔧 MEDIUM PRIORITY

**Current State:** No monitoring or cost tracking

**What's Missing:**

- **Token Usage Tracking**
    - Track tokens per request (input/output)
    - Track tokens per session
    - Show token usage in status bar
    - Alert when approaching context limits

- **Cost Estimation**
    - Track API costs per backend
    - Show cost per session
    - Cumulative cost tracking
    - Budget alerts and limits

- **Performance Metrics**
    - Response time tracking
    - Tool execution time
    - Cache hit rates
    - Request success/failure rates

- **Usage Analytics**
    - Most used tools
    - Most used backends
    - Session duration
    - Export usage reports

**Why it matters:** Understanding costs and performance helps users optimize their usage and avoid unexpected bills.
Critical for production use.

**Technical Requirements:**

```rust
struct UsageTracker {
    session_stats: SessionStats,
    cumulative_stats: CumulativeStats,

    fn record_request( & mut self,
    backend: &str,
    tokens: TokenUsage,
    cost: f64)
    fn get_session_stats( & self ) -> & SessionStats
    fn get_cumulative_stats( & self ) -> & CumulativeStats
    fn export_report( & self,
    format: ReportFormat) -> Result<String>
}

struct TokenUsage {
    input: usize,
    output: usize,
    total: usize,
}
```

**Storage:** `~/.config/hoosh/usage.db` (SQLite)

---

### 11. Screenshot Tool for Visual Tasks 🔧 LOW PRIORITY

**Current State:** No screenshot capability
**What's Missing:**

- Integrate a screenshot capturing tool (e.g., `scrot` for Linux, `screencapture` for macOS)
- Tool to capture and save screenshots to a specified directory
- Option to annotate screenshots with text or arrows
- Configurable screenshot format (PNG, JPEG)
- Ability to capture specific windows or regions of the screen
- Option to include screenshots in conversation history
- Command to trigger screenshot capture (e.g., `/screenshot`)

**Why it matters:** Some coding tasks involve visual elements (e.g., UI design, bug reproduction). Screenshots allow the
AI to access visual context and provide better assistance.

### 12. Markdown rendering in TUI 🔧 LOW PRIORITY

**Current State:** Plain text only in TUI
**What's Missing:**

- Render markdown formatting (bold, italics, code blocks, lists)
- Support for inline code highlighting
- Render links and allow opening in browser

