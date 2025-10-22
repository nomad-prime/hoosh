# Hoosh MVP Release Task List

### 4. Installation & Distribution

- **Issue**: Currently requires `cargo run` to execute
- **Required**: use `cargo-dist` to create distributable binaries for major platforms (Linux, macOS, Windows)
- **Documentation**: Add installation instructions to README.md
- **Priority**: HIGH
- **Effort**: 2-3 hours

### 5. Conversation Persistence

- **Status**: Not implemented (mentioned in ROADMAP)
- **Required Commands**:
    - `/save [name]` - Save current conversation
    - `/load <name>` - Load saved conversation
    - `/list` - List saved conversations
    - `/delete <name>` - Delete conversation
- **Storage**: `~/.config/hoosh/conversations/` (JSON format)
- **Why MVP**: Essential for multi-session work
- **Priority**: HIGH
- **Effort**: 4-6 hours
- **Dependencies**: Command system (✅ already implemented)

### 7. Documentation Improvements

- **Missing**:
    - Quick start guide for first-time users
    - Troubleshooting section
    - Example workflows
    - Command reference
    - Agent system explanation
- **Required Files**:
    - QUICKSTART.md
    - TROUBLESHOOTING.md
    - Update README.md with better examples
- **Priority**: HIGH
- **Effort**: 3-4 hours

---

## 🟢 MEDIUM PRIORITY - Quality of Life

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

## 🔵 LOW PRIORITY - Nice to Have

### 12. Testing Coverage

- **Current**: 57 tests, good coverage
- **Improvements**:
    - Integration tests for full workflows
    - Backend integration tests (with mocks)
    - TUI testing (if feasible)
    - Benchmark tests for performance
- **Priority**: LOW (but important for long-term)
- **Effort**: 8-10 hours

### 13. Code Quality Improvements

- **Issues**:
    - Some functions are too long (e.g., `create_backend` in main.rs)
    - Repeated code patterns (e.g., config path handling)
    - Missing documentation comments on public APIs
- **Required**:
    - Refactor large functions
    - Extract common utilities
    - Add rustdoc comments
    - Run `cargo fmt` and `cargo clippy --all-features` regularly
- **Priority**: LOW
- **Effort**: 4-6 hours

### 14. Better Agent System

- **Current**: Basic agent loading from files
- **Improvements**:
    - Hot-reload agents without restart
    - Agent validation on load
    - Better error messages when agent file missing
    - `/agent reload` command
    - cycle between agents in TUI
- **Priority**: LOW
- **Effort**: 2-3 hours

### 15. Bash Tool Improvements

- **Current**: Good security checks
- **Improvements**:
    - Whitelist mode (only allow specific commands)
    - Command aliases for common operations
    - Better output formatting
    - Support for interactive commands (with timeout)
- **Priority**: LOW
- **Effort**: 3-4 hours

---
