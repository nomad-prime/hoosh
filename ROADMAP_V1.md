# Hoosh v1.0 Roadmap

### 0. Commands and Command Completion System 🔧 CRITICAL

**Current State:** File completion with `@` exists (see `src/tui/completion.rs`), but no slash command system

**What's Missing:**

- **Slash Command System** - Commands start with `/` like in Claude Code
    - `/help` - Show available commands and usage
    - `/clear` - Clear conversation history
    - `/save [name]` - Save current conversation
    - `/load <name>` - Load a saved conversation
    - `/list` - List saved conversations
    - `/delete <name>` - Delete a conversation
    - `/reset` - Reset conversation context
    - `/config` - Show/edit configuration
    - `/tools` - List available tools
    - `/agents` - List available agents
    - `/agent <name>` - Switch to specific agent
    - `/toggle` - Toggle between plan/code modes or cycle agents
    - `/status` - Show current session status
    - `/export [format]` - Export conversation (JSON, markdown, etc.)
    - `/undo` - Undo last operation
    - `/redo` - Redo undone operation

- **Command Completion** - Tab completion like file completion with `@`
    - Trigger on `/` + Tab to show available commands
    - Show command descriptions inline
    - Complete command arguments (file paths, agent names, etc.)
    - History-based suggestions for command arguments

- **@ Mention System** - Reference files, symbols, and context
    - ✅ `@file.rs` - Reference a specific file (DONE - see src/tui/completion.rs)
    - ✅ Fuzzy search for file mentions (DONE)
    - `@src/` - Reference a directory (extend current implementation)
    - `@symbol_name` - Reference a function/struct/symbol (Post-v1, needs LSP)
    - `@conversation` - Reference previous conversation
    - Preview on hover/selection

- **Command Parser & Registry**
    - Modular command system (easy to add new commands)
    - Command validation and argument parsing
    - Command aliases and shortcuts
    - Command history (up/down arrows)
    - Command chaining (e.g., `/save && /clear`)

- **Interactive Command Mode**
    - Multi-line command input
    - Command prompt with syntax highlighting
    - Visual feedback for command execution
    - Error messages with suggestions

**Why it matters:** Commands provide a structured way to interact with the system, making it more user-friendly and
efficient. This is foundational for all other features - conversation management, agent switching, configuration, etc.

**Technical Requirements:**

```rust
// Reuse existing Completer trait from src/tui/completion.rs
// This trait is already implemented by FileCompleter for @ mentions

// CommandCompleter implements Completer for slash commands
struct CommandCompleter {
    registry: Arc<CommandRegistry>,
}

impl Completer for CommandCompleter {
    fn trigger_key(&self) -> char { '/' }
    async fn get_completions(&self, query: &str) -> Result<Vec<String>> {
        // Return matching commands based on query
    }
    fn format_completion(&self, item: &str) -> String {
        // Format: "/command - description"
    }
}

// SymbolCompleter implements Completer for symbol mentions (needs LSP)
struct SymbolCompleter {
    lsp_client: Arc<LspClient>,
}

impl Completer for SymbolCompleter {
    fn trigger_key(&self) -> char { '@' }  // Shares @ with files, but resolves symbols
    async fn get_completions(&self, query: &str) -> Result<Vec<String>> {
        // Return matching symbols from LSP
    }
}

// Command execution and registry
struct CommandRegistry {
    commands: HashMap<String, Box<dyn Command>>,

    fn register(&mut self, command: Box<dyn Command>) -> Result<()>
    fn execute(&self, input: &str, context: &mut Context) -> Result<CommandResult>
    fn get_help(&self, command_name: Option<&str>) -> String
    fn list_commands(&self) -> Vec<(&str, &str)>  // For completion
}

trait Command {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn aliases(&self) -> Vec<&str>;
    fn usage(&self) -> &str;
    fn execute(&self, args: Vec<String>, context: &mut Context) -> Result<CommandResult>;
}
```

**Architecture Note:** All completers (file, command) implement the same `Completer` trait from `src/tui/completion.rs`. This provides a unified interface for all completion types:
- `FileCompleter` - `@` trigger for file paths (✅ already exists)
- `CommandCompleter` - `/` trigger for slash commands (to be added)
- `SymbolCompleter` - `@` trigger for code symbols (Post-v1, needs LSP integration)

**Storage Location:**

- Command history: `~/.config/hoosh/command_history`
- Command aliases: `~/.config/hoosh/aliases.toml`

**UI Requirements:**

- Status bar showing current mode (command/chat)
- Command palette (Ctrl+P / Cmd+P)
- Inline command suggestions
- Syntax highlighting for commands and mentions

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

### 7. Git Integration and Operations 🔧 LOW PRIORITY (Nice-to-Have)

**Current State:** Git operations work fine via Bash tool

**What's Missing:**

- **Dedicated Git Tool** - Native git integration beyond bash commands
  - Git status with visual diff display
  - Commit with auto-generated messages
  - Branch creation and switching
  - Merge conflict resolution assistance
  - Pull/push operations
  - Stash management
  - Rebase operations
  - Git history visualization
  - Blame annotations

- **Smart Git Operations**
  - Commit message generation based on changes
  - Auto-detect related changes for atomic commits
  - Suggest branch names based on task
  - Detect merge conflicts and suggest resolutions
  - Pre-commit hooks integration
  - Git ignore management

- **Repository Intelligence**
  - Detect repository type and structure
  - Understand branching strategy (gitflow, trunk-based, etc.)
  - Track uncommitted changes in status bar
  - Show current branch in UI

**Why it matters (but not critical for v1):** Bash commands work fine for git operations. Dedicated git integration would provide better UX and structured output for the AI, but this is polish, not a blocker. Can be deferred to v1.1+.

**Technical Requirements:**

```rust
struct GitTool {
    repo_path: PathBuf,

    fn status(&self) -> Result<GitStatus>
    fn commit(&self, message: String, files: Vec<PathBuf>) -> Result<String>
    fn create_branch(&self, name: String) -> Result<()>
    fn switch_branch(&self, name: String) -> Result<()>
    fn diff(&self, staged: bool) -> Result<String>
    fn generate_commit_message(&self, changes: &[FileChange]) -> Result<String>
    fn detect_conflicts(&self) -> Result<Vec<Conflict>>
}

struct GitStatus {
    current_branch: String,
    staged: Vec<PathBuf>,
    unstaged: Vec<PathBuf>,
    untracked: Vec<PathBuf>,
    ahead: usize,
    behind: usize,
}
```

---

### 8. MCP (Model Context Protocol) Support 🔧 CRITICAL

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

**Why it matters:** MCP is the standard for extending AI assistants with custom tools and integrations. This enables hoosh to integrate with any MCP-compatible service and significantly extends its capabilities without modifying core code.

**Technical Requirements:**

```rust
struct McpClient {
    servers: HashMap<String, McpServer>,

    fn connect(&mut self, config: McpServerConfig) -> Result<String>
    fn disconnect(&mut self, server_id: &str) -> Result<()>
    fn list_tools(&self, server_id: &str) -> Result<Vec<ToolSchema>>
    fn execute_tool(&self, server_id: &str, tool: &str, args: Value) -> Result<Value>
    fn list_resources(&self, server_id: &str) -> Result<Vec<Resource>>
    fn read_resource(&self, server_id: &str, uri: &str) -> Result<String>
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

### 9. LSP (Language Server Protocol) Integration 🔧 LOW PRIORITY (Post-v1)

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

**Why it matters:** LSP provides deep code understanding that enables intelligent refactoring, navigation, and error detection. This makes hoosh significantly more powerful for coding tasks.

**Technical Requirements:**

```rust
struct LspClient {
    servers: HashMap<String, LanguageServer>,

    fn start_server(&mut self, language: &str, root_path: PathBuf) -> Result<()>
    fn goto_definition(&self, file: &Path, position: Position) -> Result<Location>
    fn find_references(&self, file: &Path, position: Position) -> Result<Vec<Location>>
    fn hover(&self, file: &Path, position: Position) -> Result<String>
    fn diagnostics(&self, file: &Path) -> Result<Vec<Diagnostic>>
    fn symbols(&self, workspace: bool) -> Result<Vec<Symbol>>
    fn rename(&self, file: &Path, position: Position, new_name: String) -> Result<WorkspaceEdit>
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

### 10. Project/Codebase Indexing and Understanding 🔧 LOW PRIORITY (Post-v1)

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

**Why it matters:** Understanding the entire codebase enables better suggestions, refactoring, and feature development. The AI can make informed decisions about where to make changes.

**Technical Requirements:**

```rust
struct ProjectIndex {
    root: PathBuf,
    symbols: HashMap<String, Vec<Symbol>>,
    dependencies: DependencyGraph,
    file_tree: FileTree,

    fn build_index(&mut self) -> Result<()>
    fn search_symbols(&self, query: &str) -> Result<Vec<SymbolMatch>>
    fn find_related_files(&self, file: &Path) -> Result<Vec<PathBuf>>
    fn get_dependencies(&self, file: &Path) -> Result<Vec<Dependency>>
    fn suggest_context(&self, task: &str) -> Result<Vec<PathBuf>>
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

### 11. Testing Integration 🔧 LOW PRIORITY (Nice-to-Have)

**Current State:** Tests work fine via Bash tool (cargo test, npm test, pytest, etc.)

**What's Missing:**

- **Test Execution**
  - Detect test framework (pytest, jest, cargo test, etc.)
  - Run all tests or specific test files
  - Run tests matching a pattern
  - Watch mode for continuous testing
  - Parallel test execution

- **Test Result Analysis**
  - Parse test output and failures
  - Show failed tests with context
  - Suggest fixes for failing tests
  - Track test coverage
  - Compare test runs

- **Test Generation**
  - Generate test cases for functions
  - Create test scaffolding
  - Suggest edge cases to test
  - Generate mocks and fixtures

- **Test-Driven Development**
  - Write tests first, then implementation
  - Red-Green-Refactor cycle support
  - Test coverage goals

**Why it matters (but not critical for v1):** Testing works fine via bash commands. Dedicated test integration would provide structured output parsing and better UX, but this is polish, not a blocker. Can be deferred to v1.1+.

**Technical Requirements:**

```rust
struct TestRunner {
    framework: TestFramework,

    fn detect_framework(&self, path: &Path) -> Result<TestFramework>
    fn run_tests(&self, filter: Option<&str>) -> Result<TestResults>
    fn run_test_file(&self, file: &Path) -> Result<TestResults>
    fn watch(&self) -> Result<TestWatcher>
    fn generate_test(&self, target: &Symbol) -> Result<String>
}

struct TestResults {
    total: usize,
    passed: usize,
    failed: usize,
    skipped: usize,
    failures: Vec<TestFailure>,
    coverage: Option<Coverage>,
}
```

---

### 12. Performance Monitoring and Cost Tracking 🔧 MEDIUM PRIORITY

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

**Why it matters:** Understanding costs and performance helps users optimize their usage and avoid unexpected bills. Critical for production use.

**Technical Requirements:**

```rust
struct UsageTracker {
    session_stats: SessionStats,
    cumulative_stats: CumulativeStats,

    fn record_request(&mut self, backend: &str, tokens: TokenUsage, cost: f64)
    fn get_session_stats(&self) -> &SessionStats
    fn get_cumulative_stats(&self) -> &CumulativeStats
    fn export_report(&self, format: ReportFormat) -> Result<String>
}

struct TokenUsage {
    input: usize,
    output: usize,
    total: usize,
}
```

**Storage:** `~/.config/hoosh/usage.db` (SQLite)

---

### 13. Screenshot Tool for Visual Tasks 🔧 LOW PRIORITY

**Current State:** No screenshot capability
**What's Missing:**

- Integrate a screenshot capturing tool (e.g., `scrot` for Linux, `screencapture` for macOS)
- Tool to capture and save screenshots to a specified directory
- Option to annotate screenshots with text or arrows
- Configurable screenshot format (PNG, JPEG)
- Ability to capture specific windows or regions of the screen
- Option to include screenshots in conversation history
- Command to trigger screenshot capture (e.g., `/screenshot`)

**Why it matters:** Some coding tasks involve visual elements (e.g., UI design, bug reproduction). Screenshots allow the AI to access visual context and provide better assistance.
