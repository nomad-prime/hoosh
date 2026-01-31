# hoosh Development Guidelines

Auto-generated from all feature plans. Last updated: 2025-12-09

## Active Technologies
- N/A (no persistence, transient rendering only) (001-markdown-table-rendering)
- Rust 2024 edition + okio (async runtime), serde/toml (config), anyhow (errors), ratatui (TUI) (001-disable-conversation-storage)
- File-based (.hoosh/conversations/) - JSONL for messages, JSON for metadata/index (001-disable-conversation-storage)
- Rust 2024 edition (matches project `Cargo.toml:4`) + ratatui 0.29 (TUI), crossterm 0.27 (terminal control), tokio 1.0 (async runtime), clap 4.0 (CLI), serde/serde_json (serialization) (002-terminal-modes)
- Session files in ~/.hoosh/sessions/ (JSON format, keyed by terminal PID) (002-terminal-modes)
- Rust 2024 edition (matches Cargo.toml:4) (003-input-field-refinement)
- In-memory only (attachments are ephemeral, cleared after submission) (003-input-field-refinement)

- Rust 2024 edition (matches project `Cargo.toml:4`) (001-custom-commands)

## Project Structure

```text
src/
tests/
```

## Commands

cargo test [ONLY COMMANDS FOR ACTIVE TECHNOLOGIES][ONLY COMMANDS FOR ACTIVE TECHNOLOGIES] cargo clippy

## Code Style

Rust 2024 edition (matches project `Cargo.toml:4`): Follow standard conventions

## Recent Changes
- 003-input-field-refinement: Added Rust 2024 edition (matches Cargo.toml:4)
- 002-terminal-modes: Added Rust 2024 edition (matches project `Cargo.toml:4`) + ratatui 0.29 (TUI), crossterm 0.27 (terminal control), tokio 1.0 (async runtime), clap 4.0 (CLI), serde/serde_json (serialization)
- 001-disable-conversation-storage: Added Rust 2024 edition + okio (async runtime), serde/toml (config), anyhow (errors), ratatui (TUI)


<!-- MANUAL ADDITIONS START -->
# AGENTS.md

## Coding Style

### Comments

- DO NOT ADD comments stating the obvious in the code

### Module Organization

- Use `mod.rs` files for module declarations
- Group related functionality in modules (e.g., `backends/`, `cli/`, `config/`)
- Re-export public APIs through `lib.rs`
- Keep `main.rs` minimal - just CLI entry point

### Naming Conventions

- **Structs**: PascalCase (e.g., `LlmBackend`, `ChatMessage`)
- **Traits**: PascalCase with descriptive behavior (e.g., `MessageSender`, `ConfigProvider`)
- **Functions**: snake_case with descriptive verbs (e.g., `create_client`, `parse_response`)
- **Files**: snake_case (e.g., `togehter_ai.rs`, `chat_handler.rs`)
- **Constants**: SCREAMING_SNAKE_CASE (e.g., `DEFAULT_MODEL`, `API_VERSION`)

### Error Handling

```rust
use anyhow::{Context, Result};

// Use Result<T> as return type for fallible operations
fn process_message(input: &str) -> Result<String> {
    validate_input(input)
        .context("Failed to validate input")?;
    Ok(processed)
}

// Custom error types for domain-specific errors
#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("API request failed: {0}")]
    RequestFailed(String),
    #[error("Invalid configuration: {0}")]
    ConfigError(String),
}
```

### Trait Design

```rust
#[async_trait::async_trait]
pub trait LlmBackend: Send + Sync {
    async fn send_message(&self, message: &str) -> Result<String>;
    async fn stream_response(&self, message: &str) -> Result<ResponseStream>;
    fn backend_name(&self) -> &'static str;
}

// Factory pattern for backend creation
pub fn create_backend(config: &BackendConfig) -> Result<Box<dyn LlmBackend>> {
    match config.backend_type {
        BackendType: TogetherAi => Ok(Box::new(TogetherAIBackend::new(config)?)),
        BackendType::Anthropic => Ok(Box::new(AnthropicBackend::new(config)?)),
        BackendType::OpenAI => Ok(Box::new(OpenAIBackend::new(config)?)),
    }
}
```

### Configuration Pattern

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct AppConfig {
    pub default_backend: String,
    pub backends: HashMap<String, BackendConfig>,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            Ok(toml::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }
}
```

### CLI Structure

```rust
#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Chat {
        #[arg(short, long)]
        backend: Option<String>,
        message: Option<String>,
    },
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}
```

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    // Unit tests - test individual components in isolation
    #[tokio::test]
    async fn test_message_parsing() {
        let parser = MessageParser::new();
        let result = parser.parse("test message").await;
        assert!(result.is_ok());
    }

    // Integration tests - test component interactions
    #[tokio::test]
    async fn test_backend_integration() {
        let mock_client = MockClient::new();
        let backend = Backend::new_with_client(mock_client);
        // Test actual backend behavior
    }
}
```

#### Test Coverage Guidelines

When adding tests to complex components, focus on real-world use cases:

**Happy Path Tests:**
- Simple response handling
- Multiple conversation turns
- Builder pattern configuration

**Tool Execution Tests:**
- Agent handles responses with tool calls
- Tool calls are properly executed
- Results added to conversation

**Error Handling Tests:**
- Missing response content
- Backend errors propagate correctly
- Invalid tool call responses

**State & Events Tests:**
- Event emission during operation
- Token usage tracking
- Context manager integration
- Multiple agents operate independently
```

#### Test Organization

For complex modules, use a separate test file to keep tests organized:

```rust
// In src/component/core.rs
#[cfg(test)]
#[path = "core_expanded_tests.rs"]
mod tests;
```

Use descriptive test names that describe **what behavior is being verified**, not implementation details.

Good test names:
- `agent_handles_simple_response()` - Tests core behavior
- `title_generation_returns_valid_string()` - Tests public contract  
- `multiple_agents_operate_independently()` - Tests use case
- `agent_handles_tool_calls_with_execution()` - Tests real workflow

Bad test names:
- `test_internal_state()` - Tests implementation details
- `backend_call_count_increases()` - Tests internal mechanicsanics instead of behavior

#### Mock Objects

Create realistic mocks that simulate actual dependencies:

```rust
struct MockBackend {
    responses: Vec<LlmResponse>,
    call_count: Arc<AtomicUsize>,
}

impl MockBackend {
    fn new(responses: Vec<LlmResponse>) -> Self {
        Self {
            responses,
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }
}

#[async_trait]
impl LlmBackend for MockBackend {
    async fn send_message(&self, _message: &str) -> Result<String> {
        let index = self.call_count.fetch_add(1, Ordering::SeqCst);
        self.responses.get(index)
            .cloned()
            .ok_or_else(|| /* error */)
    }
    // ... other trait methods
}
```

#### Builder Pattern in Tests

Use builder pattern with test setup to make tests more readable:

```rust
fn create_test_agent(backend: Arc<dyn LlmBackend>) 
    -> (Agent, Arc<ToolRegistry>, Arc<ToolExecutor>) {
    let tool_registry = Arc::new(ToolRegistry::new());
    let (event_tx, _) = mpsc::unbounded_channel();
    let tool_executor = Arc::new(ToolExecutor::new(tool_registry.clone()));
    
    let agent = Agent::new(backend, tool_registry.clone(), tool_executor.clone());
    (agent, tool_registry, tool_executor)
}
```

### Async Patterns

- Use `tokio::main` for async main function
- Prefer `async fn` over `-> impl Future`
- Use `Arc` for shared state across async contexts
- Handle cancellation with `tokio::select!` when appropriate

### Memory Management

- Use `Box<dyn Trait>` for trait objects
- Prefer `Arc<T>` over `Rc<T>` for multi-threaded contexts
- Use `String` for owned strings, `&str` for borrowed
- Clone cheaply with `Arc::clone()` rather than `thing.clone()`

### Refactoring

When refactoring, create new modules alongside existing ones:

- For example, if refactoring `client.rs`, create `client_v2.rs`
- Update imports gradually, ensuring tests pass at each step
- Remove old implementation only after complete migration
- Use feature flags if gradual rollout is needed

## Color Palette

### Primary Colors Used

| Color | Usage | Files |
|-------|-------|-------|
| **Cyan** | Primary borders, titles, selected items background | All dialogs |
| **Black** | Selected item text, dialog background | All dialogs |
| **Yellow** | Warnings, descriptions, instructions | setup_wizard, markdown |
| **LightYellow** | Instructions (italic) | setup_wizard, init_permission |
| **Red** | Destructive operations, errors | permission_dialog, markdown |
| **Green** | Success markers, list bullets | setup_wizard (step 4), markdown |
| **Gray/DarkGray** | Secondary text, borders | markdown, subagent_results |
| **White** | Unselected items | completion_popup |
| **Magenta** | Markdown headings | markdown |
| **Blue** | Markdown links | markdown |

### Color Constants Pattern

Use centralized color constants from `src/tui/colors.rs`:

```rust
use ratatui::style::Color;

pub mod palette {
    use super::*;

    // Primary colors
    pub const PRIMARY_BORDER: Color = Color::Cyan;
    pub const SELECTED_BG: Color = Color::Cyan;
    pub const SELECTED_FG: Color = Color::Black;
    pub const DIALOG_BG: Color = Color::Black;

    // Semantic colors
    pub const DESTRUCTIVE: Color = Color::Red;
    pub const WARNING: Color = Color::Yellow;
    pub const SUCCESS: Color = Color::Green;
    pub const INFO: Color = Color::Cyan;

    // Text colors
    pub const PRIMARY_TEXT: Color = Color::White;
    pub const SECONDARY_TEXT: Color = Color::Gray;
    pub const DIMMED_TEXT: Color = Color::DarkGray;
}
```

## Commands

- `cargo run` - Run the CLI application
- `cargo build --release` - Build optimized binary
- `cargo test` - Run all tests
- `cargo clippy` - Run linter
- `cargo fmt` - Format code
- `cargo doc --open` - Generate and open documentation

## Dependencies Management

- Keep dependencies minimal and well-maintained
- Pin major versions in Cargo.toml
- Use `cargo audit` to check for security vulnerabilities

## Performance Notes

- Use `tokio::spawn` for CPU-intensive tasks to avoid blocking
- Consider `rayon` for parallel processing of large datasets
- Profile with `cargo bench` for performance-critical paths
- Use `tokio-console` for async debugging in development
- In unit tests only use sleeps in millisecond so the tests stay fast

<!-- MANUAL ADDITIONS END -->
