# Hoosh MVP Release Task List

**Goal**: Prepare hoosh for local project usage with minimum viable features, clean code, and production readiness.

**Current Status**: Development mode, running via `cargo run`, basic features working.

---

## 🔴 CRITICAL - Must Fix Before Any Use

### 2. Security Audit - API Key Handling

- **Issue**: API keys stored in plaintext in `~/.config/hoosh/config.toml`
- **Risk**: HIGH - keys could be exposed if config file is accidentally committed or shared
- **Recommendations**:
    - Add warning in README about protecting config file
    - Consider using system keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service)
    - Add `.config/hoosh/config.toml` to example `.gitignore` patterns in documentation
    - Validate file permissions on config file (should be 0600)
- **Priority**: CRITICAL
- **Effort**: 2-4 hours for keychain integration, 30 minutes for documentation

### 3. Error Handling - Remove Unwrap() Calls

- **Issue**: 26 instances of `.unwrap()` found in codebase
- **Risk**: MEDIUM - potential panics in production
- **Action**: Audit all unwrap() calls and replace with proper error handling
- **Priority**: HIGH
- **Effort**: 2-3 hours
- **Files to check**: All `.rs` files with unwrap() calls

---

## 🟡 HIGH PRIORITY - MVP Blockers

### 4. Installation & Distribution

- **Missing**:
    - No install script or binary distribution
    - No `cargo install` support
    - No homebrew/package manager support
- **Required for MVP**:
    - Document `cargo install --path .` for local installation
    - Add binary to PATH instructions
    - Create release build instructions
    - Consider GitHub releases with pre-built binaries
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

### 6. Better Error Messages

- **Issue**: Some errors are cryptic (e.g., backend configuration errors)
- **Required**:
    - User-friendly error messages with actionable suggestions
    - Better validation on startup (check for API keys, validate config)
    - Graceful degradation when backend unavailable
- **Priority**: HIGH
- **Effort**: 3-4 hours

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

### 12. Performance Optimization

- **Issues**:
    - No caching of backend responses
    - File operations could be optimized
    - Large file handling not optimized
- **Required**:
    - Response caching (with TTL)
    - Stream large files instead of loading entirely
    - Lazy loading of agents
- **Priority**: MEDIUM
- **Effort**: 4-6 hours

---

## 🔵 LOW PRIORITY - Nice to Have

### 13. Testing Coverage

- **Current**: 57 tests, good coverage
- **Improvements**:
    - Integration tests for full workflows
    - Backend integration tests (with mocks)
    - TUI testing (if feasible)
    - Benchmark tests for performance
- **Priority**: LOW (but important for long-term)
- **Effort**: 8-10 hours

### 14. Code Quality Improvements

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

### 15. Better Agent System

- **Current**: Basic agent loading from files
- **Improvements**:
    - Hot-reload agents without restart
    - Agent validation on load
    - Better error messages when agent file missing
    - `/agent reload` command
- **Priority**: LOW
- **Effort**: 2-3 hours

### 16. Bash Tool Improvements

- **Current**: Good security checks
- **Improvements**:
    - Whitelist mode (only allow specific commands)
    - Command aliases for common operations
    - Better output formatting
    - Support for interactive commands (with timeout)
- **Priority**: LOW
- **Effort**: 3-4 hours

---

## 🚫 OUT OF SCOPE FOR MVP (Post-v1)

These are explicitly mentioned in ROADMAP as post-v1:

1. **Web Search Tool** - Requires API integration, rate limiting, etc.
2. **MCP (Model Context Protocol)** - Large feature, needs server integration
3. **LSP Integration** - Complex, needs language server management
4. **Project Indexing** - Requires AST parsing, symbol indexing
5. **Multi-Agent Orchestration (ACE)** - Advanced feature, needs reflection/curator agents
6. **Screenshot Tool** - Platform-specific, not critical
7. **Markdown Rendering in TUI** - Nice-to-have UI enhancement
8. **Multi-file Operations** - Can be done with multiple commands for MVP

---

## 📋 Recommended MVP Release Checklist

### Phase 1: Critical Fixes (Day 1)

- [ ] Fix clippy warning (1 min)
- [ ] Audit and fix unwrap() calls (2-3 hours)
- [ ] Add API key security documentation (30 min)
- [ ] Validate config file permissions (1 hour)

### Phase 2: Essential Features (Days 2-3)

- [ ] Implement conversation persistence (/save, /load, /list, /delete) (4-6 hours)
- [ ] Add installation documentation (2 hours)
- [ ] Improve error messages (3-4 hours)
- [ ] Add config validation (2-3 hours)

### Phase 3: Documentation & Polish (Day 4)

- [ ] Write QUICKSTART.md (2 hours)
- [ ] Write TROUBLESHOOTING.md (1 hour)
- [ ] Update README with examples (1 hour)
- [ ] Add command reference (1 hour)

### Phase 4: Quality Improvements (Day 5)

- [ ] Add logging system (2-3 hours)
- [ ] Implement graceful shutdown (1-2 hours)
- [ ] Command history persistence (1-2 hours)
- [ ] Performance optimization (basic) (2-3 hours)

### Phase 5: Final Testing (Day 6)

- [ ] Manual testing of all features
- [ ] Test on clean system (fresh install)
- [ ] Verify all documentation
- [ ] Create release build
- [ ] Tag release (v0.1.0)

---

## 🎯 Definition of "MVP Ready"

The MVP is ready when:

1. ✅ All clippy warnings resolved
2. ✅ All tests passing
3. ✅ No unwrap() calls that could panic in normal usage
4. ✅ API keys properly secured (or documented risks)
5. ✅ Conversation persistence working
6. ✅ Can install and run without `cargo run`
7. ✅ Documentation covers basic usage
8. ✅ Error messages are helpful
9. ✅ Config validation prevents common mistakes
10. ✅ Graceful handling of missing backends/agents

---

## 📊 Estimated Total Effort

- **Critical Fixes**: 3-5 hours
- **High Priority**: 15-20 hours
- **Medium Priority**: 10-15 hours
- **Total for MVP**: ~30-40 hours (1 week of focused work)

---

## 🔒 Security Considerations

### Current Security Posture

- ✅ Bash command sanitization (good)
- ✅ Permission system for file operations (good)
- ✅ Dangerous command blocking (good)
- ⚠️ API keys in plaintext (needs documentation/improvement)
- ⚠️ No audit logging (consider adding)
- ⚠️ No rate limiting on API calls (could cause unexpected costs)

### Recommendations

1. Add rate limiting for backend API calls
2. Track token usage and costs
3. Add audit log for sensitive operations
4. Consider sandboxing bash execution (chroot/containers)
5. Add SECURITY.md with responsible disclosure policy

---

## 🐛 Known Technical Debt

1. **Config path handling**: Repeated code in multiple places
2. **Backend creation**: Large match statement in main.rs
3. **Error types**: Mix of anyhow and thiserror
4. **Agent loading**: Synchronous file I/O in async context
5. **TUI state management**: Could be more modular
6. **Test coverage**: Missing integration tests
7. **Documentation**: Missing rustdoc on public APIs

---

## 🔄 Backward Compatibility

### Config File

- Current format is simple TOML
- Changes needed:
    - Add version field to config
    - Implement migration system for future changes
    - Validate config schema on load

### Conversation Format

- Not yet implemented, so no compatibility concerns
- Recommend JSON format with version field

### Agent Files

- Current format is plain text
- Consider adding metadata header (YAML front matter?)

---

## 📝 Notes

- Focus on stability and usability over features
- Keep scope minimal - v1.0 can add more features
- Prioritize user experience (error messages, documentation)
- Ensure clean upgrade path for future versions
- Consider user feedback loop (GitHub issues, discussions)

---

## 🚀 Post-MVP (v0.2.0 and beyond)

After MVP release and gathering user feedback:

1. Web search integration
2. Enhanced multi-file operations
3. Better performance monitoring
4. MCP support
5. LSP integration
6. Project indexing
7. ACE orchestration system
8. Cost tracking and budgets
9. Plugin system
10. Cloud sync for conversations

---

**Last Updated**: 2024-10-17
**Version**: 0.1.0-pre-release
**Status**: Development → MVP Preparation
