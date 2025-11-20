# Hoosh Codebase Overview

## Project Purpose
**Hoosh** (هوش - Persian for "intelligence") is a powerful command-line AI assistant built in Rust that integrates AI capabilities with local development environments. It provides an interactive TUI interface for developers to interact with multiple AI backends while executing tools and managing conversations.

## Core Technologies
- **Language**: Rust 2024 Edition
- **Async Runtime**: Tokio (full features)
- **CLI Framework**: Clap 4.0 (with derive macros)
- **TUI Framework**: Ratatui 0.29.0 + Crossterm
- **Serialization**: Serde + TOML/JSON
- **HTTP Client**: Reqwest (feature-gated)
- **Error Handling**: Anyhow + Thiserror

## Project Structure

```
hoosh/
├── src/
│   ├── main.rs                    # CLI entry point
│   ├── lib.rs                     # Public API exports
│   ├── session.rs                 # Session initialization & management
│   ├── tool_executor.rs           # Tool execution orchestration
│   ├── console.rs                 # Logging system with verbosity levels
│   │
│   ├── agent/                     # Core agent logic
│   │   ├── core.rs                # Agent implementation
│   │   ├── conversation.rs        # Conversation state & messages
│   │   └── agent_events.rs        # Event system for agent actions
│   │
│   ├── agent_definition/          # Agent type definitions & management
│   │   └── mod.rs                 # AgentDefinitionManager, loads from prompts/
│   │
│   ├── backends/                  # LLM backend implementations
│   │   ├── mod.rs                 # LlmBackend trait
│   │   ├── anthropic.rs           # Claude integration
│   │   ├── openai_compatible.rs   # OpenAI/compatible APIs
│   │   ├── together_ai.rs         # Together AI integration
│   │   ├── ollama.rs              # Local Ollama support
│   │   ├── backend_factory.rs     # Factory pattern for backends
│   │   └── strategy.rs            # Retry/error handling strategies
│   │
│   ├── tools/                     # Tool system
│   │   ├── mod.rs                 # Tool trait & registry
│   │   ├── bash/                  # Bash command execution
│   │   ├── file_ops/              # File read/write/edit/list
│   │   ├── glob.rs                # File pattern matching
│   │   ├── grep.rs                # Code search (ripgrep-style)
│   │   ├── task_tool.rs           # Subagent task execution
│   │   └── provider.rs            # Tool provider abstraction
│   │
│   ├── task_management/           # Subagent orchestration
│   │   ├── mod.rs                 # AgentType, TaskDefinition
│   │   ├── task_manager.rs        # Task execution engine
│   │   └── execution_budget.rs    # Time/step budgets for tasks
│   │
│   ├── permissions/               # Security & permission system
│   │   └── ...                    # Permission descriptors, manager
│   │
│   ├── tui/                       # Terminal UI
│   │   ├── app_state.rs           # Application state management
│   │   ├── app_loop.rs            # Event loop
│   │   ├── components/            # UI components
│   │   ├── handlers/              # Event handlers
│   │   └── markdown.rs            # Markdown rendering
│   │
│   ├── config/                    # Configuration management
│   ├── cli/                       # CLI argument parsing
│   ├── commands/                  # Slash commands (/help, /reset, etc.)
│   ├── storage/                   # Conversation persistence
│   ├── context_management/        # Token limit & context handling
│   ├── history/                   # Command history
│   └── prompts/                   # Built-in agent prompts
│       ├── hoosh_assistant.txt
│       ├── hoosh_coder.txt
│       ├── hoosh_planner.txt
│       ├── hoosh_reviewer.txt
│       └── hoosh_troubleshooter.txt
│
├── tests/                         # Integration tests
├── docs/                          # Documentation
├── Cargo.toml                     # Dependencies & features
└── example_config.toml            # Example configuration
```

## Key Components

### 1. **Agent System** (`src/agent/`)
- **Agent**: Core conversation handler, manages LLM interactions and tool execution
- **Conversation**: Maintains message history and context
- **AgentEvent**: Event system for real-time feedback (thinking, tool calls, results, errors)
- Supports tool calling with automatic retry and error recovery

### 2. **Backend System** (`src/backends/`)
Multi-provider LLM support with unified interface:
- **Anthropic**: Claude models (Sonnet 4.5, Opus 4)
- **OpenAI**: GPT-4 and compatible APIs
- **Together AI**: 200+ open-source models
- **Ollama**: Local offline models
- **Strategy Pattern**: Exponential backoff retry for transient errors
- **Feature-gated**: Each backend is optional via Cargo features

### 3. **Tool System** (`src/tools/`)
Extensible tool framework with async execution:
- **Built-in Tools**:
  - `bash`: Execute shell commands with streaming output
  - `read_file`, `write_file`, `edit_file`: File operations
  - `list_directory`: Directory listing
  - `glob`: Pattern-based file search
  - `grep`: Code search with ripgrep-like functionality
  - `execute_task`: Spawn subagents for complex tasks
- **Tool Trait**: Async execution, permission descriptors, result formatting
- **Tool Registry**: Dynamic tool registration and discovery
- **Permission System**: Safety checks for destructive operations

### 4. **Task Management** (`src/task_management/`)
Hierarchical agent orchestration:
- **AgentType**: `Plan` (analysis) and `Explore` (search) subagents
- **ExecutionBudget**: Time and step limits for subagents
- **TaskManager**: Spawns and manages subagent execution
- **Budget Tracking**: Real-time progress monitoring with warnings
- Subagents can use full tool suite within budget constraints

### 5. **TUI System** (`src/tui/`)
Rich terminal interface built with Ratatui:
- **AppState**: Manages UI state, messages, input
- **Event Loop**: Handles keyboard, agent events, rendering
- **Components**: Modular UI widgets (header, chat, input, dialogs)
- **Markdown Rendering**: Syntax highlighting, code blocks
- **Modes**: Review (approve each tool) vs Autopilot (auto-execute)
- **Permission Dialogs**: Interactive security prompts

### 6. **Configuration** (`src/config/`)
TOML-based configuration at `~/.config/hoosh/config.toml`:
- Backend selection and API keys
- Model preferences per backend
- Agent definitions and defaults
- Verbosity levels
- Project-specific overrides

### 7. **Permission System** (`src/permissions/`)
Security-focused operation control:
- **Permission Descriptors**: Metadata for risky operations
- **Permission Manager**: Centralized authorization
- **Trust Modes**: Project-wide trust (session-only)
- **Safe by Default**: File reads allowed, writes/deletes require permission

## Key Features

### Multi-Mode Operation
1. **Interactive TUI**: Full-featured terminal interface
2. **Review Mode**: Approve each tool call before execution
3. **Autopilot Mode**: Auto-execute tools (toggle with Shift+Tab)
4. **Trust Project**: Grant blanket permissions within project directory

### Conversation Management
- Persistent conversation storage
- Continue previous sessions with `--continue`
- Conversation list and history browsing
- Context window management with token limits

### Advanced Capabilities
- **Streaming Responses**: Real-time LLM output
- **Subagent Delegation**: Spawn specialized agents for subtasks
- **Budget Awareness**: Time/step limits for long-running tasks
- **Event System**: Rich feedback for all operations
- **Error Recovery**: Automatic retry with exponential backoff
- **Graceful Degradation**: Best-effort completion on resource limits

### Developer Experience
- **Setup Wizard**: Interactive configuration (`hoosh setup`)
- **Slash Commands**: `/help`, `/reset`, `/switch`, `/trust`, etc.
- **Syntax Highlighting**: Code blocks with theme support
- **Clipboard Integration**: Copy code snippets
- **Completion**: Tab completion for commands and files

## Architecture Patterns

### Design Principles
1. **Trait-Based Abstraction**: `LlmBackend`, `Tool`, `Command` traits
2. **Factory Pattern**: Backend and tool creation
3. **Event-Driven**: Async event channels for UI updates
4. **Builder Pattern**: Configuration and test setup
5. **Strategy Pattern**: Retry logic, context management
6. **Feature Flags**: Optional dependencies via Cargo features

### Error Handling
- **anyhow::Result** for application errors
- **thiserror** for domain-specific error types
- Context preservation with `.context()`
- Graceful fallbacks for non-critical failures

### Async Architecture
- **Tokio** runtime with full features
- **async-trait** for async trait methods
- **mpsc channels** for event communication
- Non-blocking UI with async agent execution

## Data Flow

```
User Input (TUI)
    ↓
CLI Parser / Command Registry
    ↓
Agent (conversation loop)
    ↓
Backend (LLM API call)
    ↓
Tool Calls → ToolExecutor → Tools → Permission Check
    ↓                                       ↓
    ↓                                   [Approve/Deny]
    ↓                                       ↓
Tool Results ← ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘
    ↓
Agent (process results)
    ↓
Backend (continue conversation)
    ↓
Final Response → TUI Display
```

## Testing Strategy
- **Unit Tests**: Component isolation with mocks
- **Integration Tests**: Multi-component interactions
- **Mock Backends**: Simulated LLM responses for testing
- **Temp Directories**: Isolated file operation tests
- **Fast Tests**: Millisecond sleeps, avoid blocking

## Configuration & Setup

### First Run
```bash
hoosh setup  # Interactive wizard
```

### Example Configuration
```toml
default_backend = "anthropic"
default_agent = "hoosh_coder"
verbosity = "normal"

[backends.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
model = "claude-sonnet-4.5"
temperature = 1.0

[agents.hoosh_coder]
file = "hoosh_coder.txt"
tags = ["code", "implementation"]
```

## Entry Points

### Main Entry
- `src/main.rs`: CLI parsing, config loading, session initialization
- Subcommands: `config`, `conversations`, `setup`
- Default: Launch interactive agent session

### Session Lifecycle
1. Load configuration
2. Initialize backend (factory pattern)
3. Create tool registry with available tools
4. Setup permission manager
5. Initialize TUI or run command
6. Start event loop
7. Process agent interactions
8. Save conversation on exit

## Development Commands
```bash
cargo run                    # Run interactive mode
cargo run -- setup          # Configuration wizard
cargo run -- config show    # View settings
cargo build --release       # Production build
cargo test                  # Run all tests
cargo clippy                # Linting
```

## Notable Implementation Details
- **Budget System**: Subagents have time/step limits with graceful degradation
- **Context Management**: Sliding window + summarization for long conversations
- **Permission Scopes**: Operation-level (read/write) + path-based validation
- **Event Streaming**: Tools can emit progress events during execution
- **Retry Logic**: Exponential backoff for rate limits and server errors
- **Modular Architecture**: Each component in separate module with `mod.rs`

## Future Extensibility
The architecture supports:
- Adding new backends (implement `LlmBackend` trait)
- Custom tools (implement `Tool` trait)
- New agent types (add to `AgentType` enum)
- Additional commands (register in `CommandRegistry`)
- Custom UI components (add to `tui/components/`)

---

**License**: GNU General Public License v3.0  
**Repository**: https://github.com/nomad-prime/hoosh
