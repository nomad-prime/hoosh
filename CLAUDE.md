# hoosh Development Guidelines

Auto-generated from all feature plans. Last updated: 2025-12-09

## Active Technologies
- N/A (no persistence, transient rendering only) (001-markdown-table-rendering)
- Rust 2024 edition + okio (async runtime), serde/toml (config), anyhow (errors), ratatui (TUI) (001-disable-conversation-storage)
- File-based (.hoosh/conversations/) - JSONL for messages, JSON for metadata/index (001-disable-conversation-storage)

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
- 001-disable-conversation-storage: Added Rust 2024 edition + okio (async runtime), serde/toml (config), anyhow (errors), ratatui (TUI)
- 001-markdown-table-rendering: Added Rust 2024 edition (matches project `Cargo.toml:4`)

- 001-custom-commands: Added Rust 2024 edition (matches project `Cargo.toml:4`)

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
