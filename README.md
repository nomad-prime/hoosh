# Hoosh

A powerful command-line AI assistant built in Rust, designed to seamlessly integrate AI capabilities with your local development environment.

> **Hoosh** (هوش) means "intelligence", "intellect", or "mind" in Persian.

## Features

- **Multi-backend Support**: Support for multiple AI providers
  - **OpenAI** (GPT-4, GPT-4-turbo)
  - **Anthropic** (Claude Sonnet, Claude Opus)
  - **Together AI** (200+ open source models)
  - **Ollama** (local models for offline operation)
  - **Groq** (ultra-fast inference)
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

# Set default backend
hoosh config set default_backend anthropic

# Set default verbosity
hoosh config set verbosity debug

# Configure backend API keys
hoosh config set openai_api_key sk-...
hoosh config set anthropic_api_key sk-ant-...
hoosh config set together_ai_api_key tgp_...
hoosh config set groq_api_key gsk_...

# Configure backend models
hoosh config set openai_model gpt-4
hoosh config set anthropic_model claude-sonnet-4.5
hoosh config set together_ai_model Qwen/Qwen3-Coder-480B-A35B-Instruct-FP8
hoosh config set ollama_model llama3
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

## Supported Backends

### OpenAI
Use GPT-4 and other OpenAI models for high-quality responses.

```bash
hoosh config set openai_api_key sk-...
hoosh config set openai_model gpt-4
hoosh config set default_backend openai
```

Get your API key: https://platform.openai.com/api-keys

### Anthropic (Claude)
Essential for self-improvement tasks, as Claude excels at coding.

```bash
hoosh config set anthropic_api_key sk-ant-...
hoosh config set anthropic_model claude-sonnet-4.5
hoosh config set default_backend anthropic
```

Get your API key: https://console.anthropic.com/settings/keys

Available models:
- `claude-sonnet-4.5` - Latest Sonnet model, best for coding
- `claude-opus-4` - Most capable model for complex tasks

### Together AI
Access 200+ open source models including Llama, Qwen, Mistral, and more.

```bash
hoosh config set together_ai_api_key tgp_...
hoosh config set together_ai_model Qwen/Qwen3-Coder-480B-A35B-Instruct-FP8
hoosh config set default_backend together_ai
```

Get your API key: https://api.together.xyz/settings/api-keys

### Ollama
Run models locally for offline operation and privacy.

```bash
# Install Ollama first: https://ollama.ai
# Pull a model: ollama pull llama3

hoosh config set ollama_model llama3
hoosh config set default_backend ollama
```

No API key required - runs completely offline!

### Groq
Ultra-fast inference with OpenAI-compatible API.

```bash
hoosh config set groq_api_key gsk_...
hoosh config set groq_model mixtral-8x7b-32768
hoosh config set default_backend groq
```

Get your API key: https://console.groq.com/keys

## Configuration

Hoosh uses a TOML configuration file located at `~/.config/hoosh/config.toml`. You can customize various aspects of the application including:

- Default backend settings
- Backend-specific API keys, models, and URLs
- Verbosity levels
- Permission defaults
- Agent configurations

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