### Wishes

#### Add messages midflight

want to add messages as llm is working same as claude code

#### Error Log

Add error logs file, have option in hoosh to examine those logs

#### First Ctrl+C

should always cancel current operation instead of exiting the program, with second Ctrl+C exiting

#### Circuit breaker

for LLM calls when repeated failures occur

#### Memory-> and the tool to load

I find myself referencing previous conversations often, so having a way to load previous conversations into memory would
be helpful. Maybe a command like /load_conversation <conversation_id> that fetches and loads the conversation into the
current context.

#### Status Flaky

after approval rejection status line stucks on processing

#### Running Todos

currently there are no running todos like in claude code, this can create a better ux and system prompting for the model
CRUDing todos could also be a tool call for the model

### System Reminder

Claude code uses system reminder to observe the changes done in the system in realtime

### switch backend

currently switching models and backends is only possible through configs. lets make it into a command

### Tool Status

currently I add the tool and then append tool result (preview) in messages. Ideally there is a space above status and
keeps tool calls there
(especially because tool calls can be in parallel). one the tool call is complete I can add it to message history, till
then I keep it above status bar

### File Expansion

if a file is referenced in input, file read should be shown afterwards

### Approve Plan

very often AI creates a plan before moving on, this should be a

### ways forward (question and answer tool)

have llm give forks as to possible implementations, user chooses the way

### Tools

Core Development Tools

- Read - Read files from the filesystem (supports code, images, PDFs, Jupyter notebooks)
- Write - Create new files or overwrite existing ones
- Edit - Perform exact string replacements in files
- NotebookEdit - Edit Jupyter notebook cells
- Bash - Execute shell commands (git, npm, docker, etc.)
- BashOutput - Retrieve output from background bash shells
- KillShell - Terminate background bash shells

Search & Navigation Tools

- Glob - Find files using glob patterns (e.g., **/*.js)
- Grep - Search file contents using regex patterns (powered by ripgrep)
- Task - Launch specialized agents for complex tasks:
    - general-purpose - Multi-step tasks and research
    - Explore - Fast codebase exploration
    - Plan - Planning and analysis
    - statusline-setup - Configure status line settings

Web Tools

- WebSearch - Search the web for current information
- WebFetch - Fetch and analyze content from URLs

Planning & Organization

- TodoWrite - Create and manage task lists for tracking progress
- ExitPlanMode - Exit planning mode when ready to implement

User Interaction

- AskUserQuestion - Ask users questions with multiple choice options

Extensions

- Skill - Execute skills for specialized capabilities
- SlashCommand - Execute custom slash commands from .claude/commands/

### 8. Config Validation & Defaults

- **Issue**: Silent failures when config is invalid
- **Required**:
    - Validate config on load with helpful error messages
    - Better defaults (e.g., use mock backend if no API key)
    - Config migration/upgrade system for future versions
    - `hoosh config validate` command
- **Priority**: MEDIUM
- **Effort**: 2-3 hours

### 9. Command History Persistence

- **Status**: In-memory only (mentioned in ROADMAP)
- **Required**: Save command history to `~/.config/hoosh/command_history`
- **Why Useful**: Improve UX with persistent history across sessions
- **Priority**: MEDIUM
- **Effort**: 1-2 hours

### 10. Better Logging System

- **Issue**: Debug messages sent via AgentEvent but not used
- **Required**:
    - Proper logging framework (e.g., `tracing` or `env_logger`)
    - Log file at `~/.config/hoosh/logs/hoosh.log`
    - Configurable log levels
    - Log rotation
- **Priority**: MEDIUM
- **Effort**: 2-3 hours

### 11. Graceful Shutdown

- **Issue**: No cleanup on exit (e.g., save unsaved work)
- **Required**:
    - Prompt to save conversation if modified
    - Clean up temp files
    - Close backend connections gracefully
- **Priority**: MEDIUM
- **Effort**: 1-2 hours

---

### 1. Web Search Tool

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

### 4. Multi-Agent System with ACE Orchestration

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

### 7. MCP (Model Context Protocol) Support

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

### 8. LSP (Language Server Protocol) Integration

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

### 9. Project/Codebase Indexing and Understanding

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

### 11. Screenshot Tool for Visual Tasks

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

### 12. Markdown rendering in TUI

**Current State:** Plain text only in TUI
**What's Missing:**

- Render markdown formatting (bold, italics, code blocks, lists)
- Support for inline code highlighting
- Render links and allow opening in browser

### Config Pricing API Open AI Compatible

- add support for pricing API for open ai compatible (openrouter e.g.)

### Checkpoint stopping after 100 (configurable steps)

- when in autopilot, we should have llm stop

