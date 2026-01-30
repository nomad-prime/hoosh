# Hoosh

A powerful command-line AI assistant built in Rust, designed to seamlessly integrate AI capabilities with your local
development environment.

> **Hoosh** (هوش) means "intelligence", "intellect", or "mind" in Persian.

NOTE: HOOSH is still in development and does not support backward compatibility with previous versions. 

## Features

- **Multi-backend Support**: Support for multiple AI providers
    - **OpenAI** (GPT-4, GPT-4-turbo)
    - **Anthropic** (Claude Sonnet, Claude Opus)
    - **Together AI** (200+ open source models)
    - **Ollama** (local models for offline operation)
- **Tool Integration**: Execute system commands, file operations, and custom tools through AI
- **Conversation Management**: Maintain context across multiple interactions
- **Permission System**: Control what actions the AI can perform on your system
- **Review/Autopilot Modes**: Toggle between reviewing every operation or running on autopilot (Shift+Tab)
- **Configurable**: Customize behavior through TOML configuration files
- **Graceful Error Handling**: Automatic retry with exponential backoff for transient errors (rate limits, server
  errors) with real-time user feedback through the event system
- **CLI Interface**: Intuitive command-line interface with subcommands

## Installation

### Prerequisites

- Rust 2024 edition or later
- Cargo package manager

### Building from Source

```bash
git clone https://github.com/nomad-prime/hoosh
cd hoosh
cargo build --release
```

The compiled binary will be available at `target/release/hoosh`.

## Quick Start

### Initial Setup

Run the interactive setup wizard to configure Hoosh:

```bash
hoosh setup
```

The wizard will guide you through:

- Selecting your preferred AI backend (Anthropic, OpenAI, Together AI, or Ollama)
- Configuring API credentials (environment variable or stored in config)
- Choosing your default model

You can re-run `hoosh setup` at any time to reconfigure your settings.

## Usage

### Basic Chat

Start a conversation with the AI (interactive TUI mode):

```bash
hoosh
```

### Specify Backend

Choose a specific backend for your conversation:

```bash
hoosh --backend anthropic
```

### Directory Access

Allow the AI to access specific directories:

```bash
hoosh --add-dir ./src
```

### Continue Last Conversation

Resume your previous conversation:

```bash
hoosh --continue
```

## Terminal Display Modes

Hoosh supports three terminal display modes to fit different workflows:

### Inline Mode (Default)

The standard interactive TUI mode that takes over your terminal:

```bash
hoosh
# or explicitly
hoosh --mode inline
```

- Full TUI interface with message history
- Scrolls with terminal scrollback
- Works best in standard terminal emulators

### Fullview Mode

Viewport-based mode with internal scrolling, ideal for VSCode terminals:

```bash
hoosh --mode fullview
```

- Takes over entire terminal viewport
- Internal scrolling (doesn't rely on terminal scrollback)
- Fixes visual corruption in VSCode and other limited terminals
- Navigate with arrow keys, Page Up/Down, or vim-style j/k

### Tagged Mode (Shell Integration)

Non-hijacking mode that integrates seamlessly with your shell workflow:

#### Installation

Set up the `@hoosh` alias in your shell:

```bash
hoosh alias install
```

This automatically detects your shell (bash, zsh, or fish) and adds the `@hoosh` function to your shell configuration file.

After installation, restart your terminal or run:

```bash
# For bash/zsh
source ~/.bashrc  # or ~/.zshrc

# For fish
source ~/.config/fish/config.fish
```

#### Usage

Once installed, use `@hoosh` directly in your terminal:

```bash
# Ask questions inline
@hoosh "what files are in this directory?"

# Get coding help
@hoosh "fix the bug in main.rs"

# Use with pipes
git diff | @hoosh "explain these changes"

# Slash commands work too
@hoosh /commit
```

#### Features

- **Non-hijacking**: Returns control to your shell immediately after completion
- **Context preservation**: Conversation history persists across multiple `@hoosh` invocations within the same terminal session
- **Terminal-native output**: All output flows naturally like any bash command, no TUI
- **Visual feedback**: Shows braille spinner while processing

#### How It Works

- The `@hoosh` function is a shell alias that calls `hoosh --mode tagged`
- Conversation context is saved to `~/.hoosh/sessions/session_[PID].json`
- Context is tied to your terminal process (cleared when you close the terminal)
- Old session files are automatically cleaned up after 7 days

### Review vs Autopilot Mode

Hoosh operates in two modes to control how operations are executed:

#### Review Mode (Default)

- Shows an approval dialog for **every tool call** before execution
- Displays the tool name and allows you to approve or reject
- Provides maximum control and visibility over AI actions
- Ideal when you want to inspect each operation

#### Autopilot Mode

- Automatically executes all tool calls without approval dialogs
- Still respects the permission system (asks for risky operations)
- Ideal for faster iteration when you trust the AI's actions

The current mode is always displayed in the status bar:

- `[Review]` - You'll see an approval dialog for each tool call
- `[Autopilot]` - Tool calls execute automatically (after permission checks)

### Permission Management

Hoosh includes a security-focused permission system to control risky operations.

#### Permission Dialogs

When the AI attempts a potentially risky operation (file writes, deletions, bash commands), you'll see a permission
dialog with three options:

- **[y] Yes, once** - Allow this specific operation only
- **[n] No** - Deny this operation
- **[t] Trust project** - Grant permission for all operations within the current project directory

#### Trust Project Mode

The "Trust Project" option provides convenient authorization for trusted codebases:

1. When prompted for permission, select "Trust project" (press `T` or select and press Enter)
2. All risky operations within the current project directory will be automatically approved
3. A visual indicator (`🔓 Project Trusted`) appears in the TUI header
4. Trust is session-only and cleared when you exit Hoosh

**Revoking Trust:**

Use the `/untrust` command to revoke project-wide trust mid-session:

```
/untrust
```

This will re-enable permission dialogs for all operations.

#### Security Considerations

- Trust is limited to the current working directory and its subdirectories
- Trust is never persisted to disk - each session starts fresh
- Operations outside the trusted directory still require permission
- Safe operations (reading files, listing directories) are always allowed
- Permission checks happen **in addition to** approval dialogs in Review mode

### Configuration

Manage configuration settings:

```bash
# View current configuration
hoosh config show

# Set default backend
hoosh config set default_backend anthropic

# Set default agent
hoosh config set default_agent hoosh_coder

# Set default verbosity (quiet, normal, verbose, debug)
hoosh config set verbosity debug

# Configure backend API keys
hoosh config set openai_api_key sk-...
hoosh config set anthropic_api_key sk-ant-...
hoosh config set together_ai_api_key tgp_...

# Configure backend models
hoosh config set openai_model gpt-4
hoosh config set anthropic_model claude-sonnet-4.5
hoosh config set together_ai_model Qwen/Qwen3-Coder-480B-A35B-Instruct-FP8
hoosh config set ollama_model llama3

# Configure backend base URLs (for custom endpoints)
hoosh config set openai_base_url https://api.openai.com/v1
hoosh config set ollama_base_url http://localhost:11434

# Configure backend temperature (0.0-2.0)
hoosh config set openai_temperature 0.7
hoosh config set anthropic_temperature 1.0
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

## Configuration File

Hoosh uses a TOML configuration file located at `~/.config/hoosh/config.toml`. You can customize various aspects of the
application including:

- **default_backend**: The backend to use by default (e.g., "anthropic", "openai", "ollama")
- **default_agent**: The default agent to use for conversations
- **verbosity**: Logging verbosity level (quiet, normal, verbose, debug)
- **backends**: Backend-specific configurations
    - **api_key**: Authentication key for the backend
    - **model**: Model to use for this backend
    - **base_url**: Custom API endpoint URL
    - **temperature**: Sampling temperature (0.0-2.0)
- **agents**: Custom agent configurations with prompts and tags

> ⚠️ **Security Warning**: This configuration file contains sensitive API keys. Ensure the file permissions are set to
> 0600 (owner read/write only) to prevent unauthorized access. Never commit this file to version control.

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
