# CLAUDE.md

## Coding Style

### Comments
- Use `///` for public items and `//!` for module-level documentation
- Keep comments concise and descriptive
- Only use comments when necessary and add them where they provide clarity or context

e.g.
BAD: the comment does not provide any value
// This function calculates the sum of two numbers
fn add(a: i32, b: i32) -> i32 {
    a + b
}

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
        BackendType:TogetherAi => Ok(Box::new(TogetherAIBackend::new(config)?)),
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
- Document why each dependency is needed

## Performance Notes
- Use `tokio::spawn` for CPU-intensive tasks to avoid blocking
- Consider `rayon` for parallel processing of large datasets
- Profile with `cargo bench` for performance-critical paths
- Use `tokio-console` for async debugging in development
