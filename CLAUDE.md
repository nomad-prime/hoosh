# hoosh Development Guidelines

Auto-generated from all feature plans. Last updated: 2025-12-09

## Active Technologies
- Rust 2024 edition with tokio async runtime + okio (async), serde (serialization), anyhow (error handling), async_trai (002-model-cascade)
- Configuration in TOML; conversation history in memory (Arc<Conversation>) (002-model-cascade)

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
- 002-model-cascade: Added Rust 2024 edition with tokio async runtime + okio (async), serde (serialization), anyhow (error handling), async_trai

- 001-custom-commands: Added Rust 2024 edition (matches project `Cargo.toml:4`)

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
