# hoosh

Guidelines for AI assistants and humans working in this repo. `CLAUDE.md` is a symlink to this file — there is one source of truth.

## Core Principles (non-negotiable)

These override other considerations. Violations need explicit justification.

### Tests come first

- Unit tests for business logic, isolated components, and pure functions
- Integration tests for component interactions and multi-module workflows
- Test names describe behavior (`agent_handles_simple_response`), not implementation (`test_internal_state`)
- Use millisecond sleeps in async tests — keep CI fast
- For complex modules use a sibling `_expanded_tests.rs` file via `#[path]`
- Mocks implement the same traits as real dependencies and simulate realistic behavior, not static returns

### Trait-based design

- Backend integrations and other replaceable dependencies are accessed via traits
- Async traits use `#[async_trait::async_trait]` and `Send + Sync` bounds
- Use the factory pattern (`fn create_backend(...) -> Box<dyn Trait>`) for runtime polymorphism
- Dependencies are injected through constructors — no globals, no hidden state
- Builder patterns improve readability of complex test setup

### Single responsibility

- One concern per module (`config.rs` for config, `chat_handler.rs` for chat)
- Functions do one thing with a verb-shaped name (`create_client`, `parse_response`)
- No god objects — large structs handling multiple concerns
- Separate business logic from I/O, parsing from validation

### Flat module structure

- Avoid nested modules. Top-level dirs (`backends/`, `cli/`, `config/`, `tui/`)
- `mod.rs` for module declarations; re-export public API through `lib.rs`
- Keep `main.rs` minimal — just CLI entry

### Clean code

- No comments stating the obvious — code should be self-documenting
- Naming: structs/traits PascalCase, functions/files snake_case, constants SCREAMING_SNAKE_CASE
- Errors: `anyhow::Result` at boundaries, `thiserror` for domain errors
- Idiomatic 2024 Rust: `Arc<T>` for shared async state, `Box<dyn Trait>` for trait objects, `async fn` over `-> impl Future`

### Quality gates (before merge)

- `cargo test` — all tests pass
- `cargo clippy` — no warnings
- `cargo fmt` — formatted
- New functionality covered by tests
- Principle violations are justified or removed

## Threat model

hoosh is not sandboxed. The threat model is "a confused agent makes a mistake," not "malicious input or malicious code." File-ops tools resolve relative paths against the working directory as a convenience, but do not enforce a filesystem boundary — `bash` is unrestricted and any agent can read or write anywhere the user can. For real isolation, run hoosh under OS-level sandboxing (`landlock`, `bwrap`, `firejail`) — don't add validators that pretend to enforce a boundary they can't.

## Coding Style

### Naming

- **Structs**: PascalCase (`LlmBackend`, `ChatMessage`)
- **Traits**: PascalCase with descriptive behavior (`MessageSender`, `ConfigProvider`)
- **Functions**: snake_case verbs (`create_client`, `parse_response`)
- **Files**: snake_case (`together_ai.rs`, `chat_handler.rs`)
- **Constants**: SCREAMING_SNAKE_CASE (`DEFAULT_MODEL`, `API_VERSION`)

### Error handling

```rust
use anyhow::{Context, Result};

fn process_message(input: &str) -> Result<String> {
    validate_input(input)
        .context("Failed to validate input")?;
    Ok(processed)
}

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("API request failed: {0}")]
    RequestFailed(String),
    #[error("Invalid configuration: {0}")]
    ConfigError(String),
}
```

### Trait design

```rust
#[async_trait::async_trait]
pub trait LlmBackend: Send + Sync {
    async fn send_message(&self, message: &str) -> Result<String>;
    async fn stream_response(&self, message: &str) -> Result<ResponseStream>;
    fn backend_name(&self) -> &'static str;
}

pub fn create_backend(config: &BackendConfig) -> Result<Box<dyn LlmBackend>> {
    match config.backend_type {
        BackendType::TogetherAi => Ok(Box::new(TogetherAIBackend::new(config)?)),
        BackendType::Anthropic => Ok(Box::new(AnthropicBackend::new(config)?)),
        BackendType::OpenAI => Ok(Box::new(OpenAIBackend::new(config)?)),
    }
}
```

### Configuration

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

### CLI

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

### Async patterns

- `tokio::main` for async main
- Prefer `async fn` over `-> impl Future`
- `Arc` for shared state across async contexts
- `tokio::select!` for cancellation handling
- `tokio::spawn` for CPU-intensive work to avoid blocking

### Memory

- `Box<dyn Trait>` for trait objects
- `Arc<T>` over `Rc<T>` in multi-threaded contexts
- `String` for owned, `&str` for borrowed
- `Arc::clone()` rather than `thing.clone()`

### Refactoring

In place when local. Side-by-side (`client_v2.rs`) only when the migration is non-trivial. Remove old as soon as the new one carries traffic. Use feature flags if rollout is gradual.

## Testing

### Layout

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn agent_handles_simple_response() {
        // ...
    }
}
```

For complex modules, use a sibling file:

```rust
// src/component/core.rs
#[cfg(test)]
#[path = "core_expanded_tests.rs"]
mod tests;
```

### Coverage

What tests need to hit:

- **Happy path**: simple response handling, multi-turn conversations, builder configuration
- **Tool execution**: tool calls executed and results added to conversation
- **Error handling**: missing content, backend errors propagating, invalid responses
- **State & events**: event emission, token tracking, context manager integration, multi-agent independence

### Mocks

Mocks implement the trait and simulate behavior. Avoid static returns:

```rust
struct MockBackend {
    responses: Vec<LlmResponse>,
    call_count: Arc<AtomicUsize>,
}

#[async_trait]
impl LlmBackend for MockBackend {
    async fn send_message(&self, _: &str) -> Result<String> {
        let i = self.call_count.fetch_add(1, Ordering::SeqCst);
        self.responses.get(i).cloned()
            .ok_or_else(|| anyhow!("MockBackend out of responses"))
    }
}
```

### Builders

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

## Color palette

Centralized in `src/tui/colors.rs`:

| Color | Usage |
|-------|-------|
| **Cyan** | Primary borders, titles, selected item background |
| **Black** | Selected item text, dialog background |
| **Yellow** | Warnings, descriptions, instructions |
| **LightYellow** | Italic instructions |
| **Red** | Destructive operations, errors |
| **Green** | Success markers, list bullets |
| **Gray / DarkGray** | Secondary text, borders |
| **White** | Unselected items |
| **Magenta** | Markdown headings |
| **Blue** | Markdown links |

```rust
pub mod palette {
    use ratatui::style::Color;

    pub const PRIMARY_BORDER: Color = Color::Cyan;
    pub const SELECTED_BG: Color = Color::Cyan;
    pub const SELECTED_FG: Color = Color::Black;
    pub const DIALOG_BG: Color = Color::Black;

    pub const DESTRUCTIVE: Color = Color::Red;
    pub const WARNING: Color = Color::Yellow;
    pub const SUCCESS: Color = Color::Green;
    pub const INFO: Color = Color::Cyan;

    pub const PRIMARY_TEXT: Color = Color::White;
    pub const SECONDARY_TEXT: Color = Color::Gray;
    pub const DIMMED_TEXT: Color = Color::DarkGray;
}
```

## Commands

- `cargo run` — run the CLI
- `cargo build --release` — release build
- `cargo test` — all tests
- `cargo clippy` — lint
- `cargo fmt` — format
- `cargo doc --open` — docs

## Dependencies

- Minimal and well-maintained
- Pin major versions in `Cargo.toml`
- `cargo audit` for vulnerability checks

## Performance

- `tokio::spawn` for CPU-intensive work
- `rayon` for parallel processing of large datasets
- `cargo bench` for performance-critical paths
- `tokio-console` for async debugging
- Use millisecond sleeps in unit tests
