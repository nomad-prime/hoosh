# Hoosh

A powerful command-line AI assistant built in Rust, designed to seamlessly integrate AI capabilities with your local development environment.

> **Hoosh** (هوش) means "intelligence", "intellect", or "mind" in Persian.

## Features

- **Multi-backend Support**: Currently supports Together AI (with more backends planned)
- **Tool Integration**: Execute system commands, file operations, and custom tools through AI
- **Conversation Management**: Maintain context across multiple interactions
- **Permission System**: Control what actions the AI can perform on your system
- **Configurable**: Customize behavior through TOML configuration files
- **CLI Interface**: Intuitive command-line interface with subcommands

## Installation

### Prerequisites

- Rust 2021 edition or later
- Cargo package manager

### Building from Source

```bash
git clone <repository-url>
cd hoosh
cargo build --release
```

The compiled binary will be available at `target/release/hoosh`.

## Usage

### Basic Chat

Start a conversation with the AI:

```bash
hoosh chat "Explain how this project works"
```

### Specify Backend

Choose a specific backend for your conversation:

```bash
hoosh chat --backend together-ai "Help me with this code"
```

### Directory Access

Allow the AI to access specific directories:

```bash
hoosh chat --add-dir ./src "Analyze this codebase"
```

### Configuration

Manage configuration settings:

```bash
# View current configuration
hoosh config show

# Set default verbosity
hoosh config set verbosity debug
```

## Project Structure

```
src/
├── backends/       # LLM backend implementations
├── cli/            # Command-line interface handling
├── config/         # Configuration management
├── console/        # Console output and logging
├── conversation/   # Conversation and message handling
├── parser/         # Message parsing utilities
├── permissions/    # Permission management system
├── system_prompts/ # System prompt management
├── tools/          # Built-in tools and tool registry
├── tool_executor/  # Tool execution engine
├── console.rs      # Console utilities
├── conversation.rs # Conversation handling
├── lib.rs          # Library exports
├── main.rs         # Main entry point
└── tool_executor.rs # Tool execution logic
```

## Configuration

Hoosh uses a TOML configuration file located at `~/.config/hoosh/config.toml`. You can customize various aspects of the application including:

- Default backend settings
- Verbosity levels
- Permission defaults
- System prompt configurations

## Development

This project follows specific coding conventions outlined in `CLAUDE.md`, including:

- Modular organization using `mod.rs` files
- Descriptive naming conventions (PascalCase for structs/traits, snake_case for functions/files)
- Minimal main.rs with CLI entry point only
- Proper error handling using `anyhow` crate

### Running Tests

```bash
cargo test
```

### Building

```bash
cargo build
```

## Dependencies

Key dependencies include:
- `clap` for CLI argument parsing
- `tokio` for async runtime
- `serde` for serialization/deserialization
- `reqwest` for HTTP client functionality (optional, feature-gated)
- Custom tooling and permission management systems

## License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.