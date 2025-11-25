# Architecture Review: Config & Agent Definition Modules

**Date:** 2025-11-25
**Modules:** `src/config/`, `src/agent_definition/`
**Status:** Well-structured with critical inconsistencies requiring attention

---

## Executive Summary

The configuration and agent definition system has a solid foundation with clear separation of concerns, comprehensive error handling, and good test coverage. However, there are **critical inconsistencies** in initialization timing, duplicate code, and missing validation that could lead to runtime errors and maintenance issues.

**Overall Grade:** B+ (Good architecture with fixable issues)

---

## Key Issues

### 1. Inconsistent Default Configuration Initialization ⚠️ CRITICAL

**Location:**
- `src/config/mod.rs:79-107` (`AppConfig::default()`)
- `src/agent_definition/mod.rs:51-106` (`initialize_default_agents()`)
- `src/tui/setup/setup_wizard_loop.rs:59-89` (`save_wizard_result()`)

**Problem:**

The `AppConfig::default()` implementation creates agents with per-agent core instruction files:

```rust
core_instructions_file: Some(format!("{}_core_instructions.txt", agent_name))
```

However, these files are NOT created during setup. The actual file writing only happens in `AgentDefinitionManager::new()` at `src/agent_definition/mod.rs:46`:

```rust
pub fn new() -> Result<Self> {
    let config = AppConfig::load()?;
    let agents_dir = AppConfig::agents_dir()?;
    Self::initialize_default_agents(&agents_dir)?;  // <-- Files only created here
    Ok(Self { config })
}
```

**Flow Analysis:**

1. User runs `hoosh setup`
2. Setup wizard calls `save_wizard_result()`
3. `save_wizard_result()` calls `AppConfig::load().unwrap_or_else(|_| AppConfig::default())`
4. If using default, config references agent files that don't exist yet
5. Files are only created when app actually runs and creates `AgentDefinitionManager`

**Impact:**
- Config file references non-existent files between setup and first use
- If someone inspects `~/.config/hoosh/config.toml` after setup, the agent configs look broken
- Potential race condition if multiple processes try to initialize agents

**Recommended Fix:**

```rust
// In src/config/mod.rs, add:
impl AppConfig {
    pub fn ensure_default_agents() -> ConfigResult<()> {
        let agents_dir = Self::agents_dir()?;

        let default_prompts = [
            ("hoosh_planner.txt", include_str!("../prompts/hoosh_planner.txt")),
            ("hoosh_coder.txt", include_str!("../prompts/hoosh_coder.txt")),
            // ... all default agents
        ];

        for (file_name, content) in default_prompts {
            let path = agents_dir.join(file_name);
            fs::write(&path, content).map_err(ConfigError::IoError)?;
        }

        Ok(())
    }

    pub fn save(&self) -> ConfigResult<()> {
        // Existing save logic...

        // Ensure agent files exist
        Self::ensure_default_agents()?;

        // Rest of save logic...
    }
}
```

---

### 2. Duplicate Default Agent List 🔄 HIGH PRIORITY

**Locations:**
- `src/config/mod.rs:9-15` (`DEFAULT_AGENT_FILES`)
- `src/agent_definition/mod.rs:52-97` (hardcoded array in `initialize_default_agents()`)

**Problem:**

Two separate lists define the same information:

```rust
// src/config/mod.rs:9-15
pub const DEFAULT_AGENT_FILES: &[&str] = &[
    "hoosh_planner.txt",
    "hoosh_coder.txt",
    "hoosh_reviewer.txt",
    "hoosh_troubleshooter.txt",
    "hoosh_assistant.txt",
];

// src/agent_definition/mod.rs:52-97
let default_prompts = [
    ("hoosh_planner.txt", include_str!("../prompts/hoosh_planner.txt")),
    ("hoosh_coder.txt", include_str!("../prompts/hoosh_coder.txt")),
    // ... etc
];
```

**Impact:**
- Adding a new default agent requires updating two locations
- Risk of forgetting one location and causing inconsistency
- Violates DRY principle

**Recommended Fix:**

```rust
// In src/config/mod.rs, expand the constant:
pub const DEFAULT_AGENTS: &[(&str, &str)] = &[
    ("hoosh_planner.txt", include_str!("../prompts/hoosh_planner.txt")),
    ("hoosh_coder.txt", include_str!("../prompts/hoosh_coder.txt")),
    ("hoosh_reviewer.txt", include_str!("../prompts/hoosh_reviewer.txt")),
    ("hoosh_troubleshooter.txt", include_str!("../prompts/hoosh_troubleshooter.txt")),
    ("hoosh_assistant.txt", include_str!("../prompts/hoosh_assistant.txt")),
];

pub const DEFAULT_CORE_INSTRUCTIONS: &[(&str, &str)] = &[
    ("hoosh_core_instructions.txt", include_str!("../prompts/hoosh_core_instructions.txt")),
    ("hoosh_coder_core_instructions.txt", include_str!("../prompts/hoosh_coder_core_instructions.txt")),
    // ... etc
];

// Then in src/agent_definition/mod.rs:
fn initialize_default_agents(agents_dir: &Path) -> Result<()> {
    for (file_name, content) in crate::config::DEFAULT_AGENTS {
        let agent_path = agents_dir.join(file_name);
        fs::write(&agent_path, content)
            .with_context(|| format!("Failed to write agent file: {}", file_name))?;
    }

    for (file_name, content) in crate::config::DEFAULT_CORE_INSTRUCTIONS {
        let path = agents_dir.join(file_name);
        fs::write(&path, content)
            .with_context(|| format!("Failed to write core instructions: {}", file_name))?;
    }

    Ok(())
}
```

---

### 3. Missing Core Instructions Validation 🔴 HIGH PRIORITY

**Location:** `src/config/mod.rs:274-291` (`load_core_instructions()`)

**Problem:**

The `load_core_instructions()` method attempts to load custom core instruction files but provides no validation during config save:

```rust
pub fn load_core_instructions(&self, agent_name: Option<&str>) -> ConfigResult<String> {
    // First, try agent-specific core instructions file
    if let Some(agent) = agent_name
        && let Some(agent_config) = self.agents.get(agent)
        && let Some(custom_file) = &agent_config.core_instructions_file
    {
        let agents_dir = Self::agents_dir()?;
        let path = agents_dir.join(custom_file);
        if let Ok(content) = fs::read_to_string(&path) {
            return Ok(content.trim().to_string());
        }
    }

    // Fall back to built-in core instructions
    Ok(include_str!("../prompts/hoosh_core_instructions.txt")
        .trim()
        .to_string())
}
```

**Impact:**
- Users can manually edit `config.toml` and set non-existent `core_instructions_file`
- No warning until runtime when agent is actually loaded
- Silent fallback to default may confuse users who expect their custom instructions

**Recommended Fix:**

```rust
impl AppConfig {
    fn validate(&self) -> ConfigResult<()> {
        // Existing validation...

        // Validate agent files and core instructions exist
        let agents_dir = Self::agents_dir()?;

        for (name, agent_config) in &self.agents {
            let agent_path = agents_dir.join(&agent_config.file);
            if !agent_path.exists() {
                eprintln!(
                    "Warning: Agent '{}' references missing file: {}",
                    name,
                    agent_config.file
                );
            }

            if let Some(core_file) = &agent_config.core_instructions_file {
                let core_path = agents_dir.join(core_file);
                if !core_path.exists() {
                    eprintln!(
                        "Warning: Agent '{}' references missing core instructions file: {}",
                        name,
                        core_file
                    );
                }
            }
        }

        Ok(())
    }
}
```

---

### 4. Inconsistent File Path Handling ⚠️ MEDIUM PRIORITY

**Locations:**
- `src/config/mod.rs:312-321` (`config_path()`)
- `src/config/mod.rs:293-306` (`agents_dir()`)
- `src/config/mod.rs:323-328` (`project_config_path()`)

**Problem:**

Multiple functions duplicate the logic to find HOME directory and build paths:

```rust
pub fn config_path() -> ConfigResult<PathBuf> {
    let path = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| ConfigError::NoHomeDirectory)?;
    let mut path = PathBuf::from(path);
    path.push(".config");
    path.push("hoosh");
    path.push("config.toml");
    Ok(path)
}

pub fn agents_dir() -> ConfigResult<PathBuf> {
    let path = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| ConfigError::NoHomeDirectory)?;
    let mut path = PathBuf::from(path);
    path.push(".config");
    path.push("hoosh");
    path.push("agents");
    // ...
}
```

**Impact:**
- Code duplication
- Risk of inconsistent path handling
- Harder to test path resolution logic

**Recommended Fix:**

```rust
impl AppConfig {
    /// Get the hoosh configuration directory: ~/.config/hoosh/
    fn hoosh_config_dir() -> ConfigResult<PathBuf> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map_err(|_| ConfigError::NoHomeDirectory)?;

        let mut path = PathBuf::from(home);
        path.push(".config");
        path.push("hoosh");

        Ok(path)
    }

    pub fn config_path() -> ConfigResult<PathBuf> {
        let mut path = Self::hoosh_config_dir()?;
        path.push("config.toml");
        Ok(path)
    }

    pub fn agents_dir() -> ConfigResult<PathBuf> {
        let mut path = Self::hoosh_config_dir()?;
        path.push("agents");
        fs::create_dir_all(&path).map_err(ConfigError::IoError)?;
        Ok(path)
    }
}
```

---

### 5. Unused Constant: DEFAULT_CORE_INSTRUCTIONS_FILE 🔴 LOW PRIORITY

**Location:** `src/config/mod.rs:17`

**Problem:**

```rust
pub const DEFAULT_CORE_INSTRUCTIONS_FILE: &str = "hoosh_core_instructions.txt";
```

This constant is defined but **never used** anywhere in the codebase.

**Impact:**
- Dead code
- Unclear intent - should it be used?
- Confusing for maintainers

**Recommended Action:**

Either:
1. **Use it** in `load_core_instructions()` as the fallback filename
2. **Remove it** if it serves no purpose

```rust
// Option 1: Use it
pub fn load_core_instructions(&self, agent_name: Option<&str>) -> ConfigResult<String> {
    // ... agent-specific loading ...

    // Fall back to default core instructions file
    let agents_dir = Self::agents_dir()?;
    let default_path = agents_dir.join(DEFAULT_CORE_INSTRUCTIONS_FILE);
    if let Ok(content) = fs::read_to_string(&default_path) {
        return Ok(content.trim().to_string());
    }

    // Ultimate fallback: built-in
    Ok(include_str!("../prompts/hoosh_core_instructions.txt")
        .trim()
        .to_string())
}
```

---

### 6. Limited Custom Agent Support ⚠️ MEDIUM PRIORITY

**Location:** Overall system design

**Problem:**

Based on `config.md`:
> hoosh_agent files are built in and will be overwritten at every startup

This is good for built-in agents, but the system provides no clear path for custom agents:

1. No documentation on how to add custom agents
2. No validation that custom agent files exist during config load
3. No CLI command to create/register custom agents
4. Users must manually:
   - Create file in `~/.config/hoosh/agents/`
   - Edit `config.toml` to add agent entry
   - Ensure naming doesn't conflict with built-ins

**Impact:**
- Custom agents are possible but undiscoverable
- Users may accidentally name custom agents with `hoosh_*` prefix and have them overwritten
- No guidance on custom agent file format

**Recommended Enhancement:**

```rust
// Add CLI command: hoosh agent create <name>
pub fn create_custom_agent(name: &str, description: Option<String>) -> Result<()> {
    if name.starts_with("hoosh_") {
        return Err(anyhow::anyhow!(
            "Custom agent names cannot start with 'hoosh_' (reserved for built-in agents)"
        ));
    }

    let agents_dir = AppConfig::agents_dir()?;
    let filename = format!("{}.txt", name);
    let agent_path = agents_dir.join(&filename);

    if agent_path.exists() {
        return Err(anyhow::anyhow!(
            "Agent file already exists: {}",
            agent_path.display()
        ));
    }

    // Create template file
    let template = "You are a helpful AI assistant.\n\n[Add your custom instructions here]\n";
    fs::write(&agent_path, template)?;

    // Update config
    let mut config = AppConfig::load()?;
    config.agents.insert(
        name.to_string(),
        AgentConfig {
            file: filename,
            description,
            tags: vec![],
            core_instructions_file: None,
        },
    );
    config.save()?;

    println!("✓ Created custom agent: {}", name);
    println!("  Edit: {}", agent_path.display());

    Ok(())
}
```

---

## Positive Aspects ✅

### 1. Excellent Separation of Concerns
- **Config module** handles loading, saving, validation, and merging
- **Agent Definition module** handles agent file management and loading
- Clear boundaries between modules

### 2. Comprehensive Error Handling
- `ConfigError` enum covers all error cases with helpful messages
- Proper use of `Result` types throughout
- Good error context with `thiserror` derive

**Location:** `src/config/error.rs:4-32`

### 3. Outstanding Test Coverage
- 741 lines of unit tests in `src/config/mod_tests.rs`
- Tests cover default values, serialization, deserialization, merging, validation
- Good test organization with clear test names

### 4. Security Conscious
- File permission validation (0600) for config files on Unix systems
- Warning messages for insecure permissions
- Automatic secure permission setting on file creation

**Location:** `src/config/mod.rs:154-197`

### 5. Flexible Project-Level Overrides
- Global config: `~/.config/hoosh/config.toml`
- Project config: `.hoosh/config.toml`
- Project settings override global settings
- Smart merging logic that preserves global defaults

**Location:** `src/config/mod.rs:330-360` (`merge()`)

### 6. Agent-Specific Core Instructions
- Each agent can have custom core instructions
- Fallback to shared default instructions
- Built-in instructions embedded in binary
- Supports custom instruction files

**Location:** `src/config/mod.rs:274-291`

### 7. Verbosity Level Handling
- Clean enum-based verbosity levels
- Config-based defaults with CLI override capability
- Proper fallback to Normal level

**Location:** `src/config/mod.rs:249-264`

---

## Architecture Strengths

### Data Flow

```
┌─────────────────────────────────────────────────────────────┐
│                        User Setup                            │
│                     hoosh setup command                      │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                  Setup Wizard (TUI)                          │
│              Collects backend, model, API key                │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│              save_wizard_result()                            │
│        Creates/updates ~/.config/hoosh/config.toml           │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                   Application Start                          │
│                    AppConfig::load()                         │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│              AgentDefinitionManager::new()                   │
│         Writes built-in agent files (overwrites)             │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                   Agent Execution                            │
│          Loads agent content and core instructions           │
└─────────────────────────────────────────────────────────────┘
```

### Module Responsibilities

**`src/config/mod.rs` (388 lines)**
- ✅ Configuration struct definitions
- ✅ Loading from TOML files
- ✅ Saving to TOML files
- ✅ Merging global and project configs
- ✅ Validation logic
- ✅ Path resolution
- ✅ Backend management
- ✅ Verbosity handling

**`src/agent_definition/mod.rs` (177 lines)**
- ✅ Agent definition struct
- ✅ Default agent file initialization
- ✅ Agent loading by name
- ✅ List available agents
- ✅ Core instructions loading

**Dependency Direction:** `agent_definition` → `config` (correct, config is more foundational)

---

## Recommendations

### High Priority (Fix Immediately)

1. **Fix initialization order** (Issue #1)
   - Move agent file writing to `AppConfig::save()` or setup flow
   - Ensure files exist before config references them
   - **Estimated effort:** 2-3 hours

2. **Consolidate default agent lists** (Issue #2)
   - Create single source of truth in config module
   - Use constants with embedded content
   - Update `AgentDefinitionManager` to use shared constants
   - **Estimated effort:** 1-2 hours

3. **Add file existence validation** (Issue #3)
   - Validate agent files and core instruction files in `validate()`
   - Provide helpful warnings for missing files
   - **Estimated effort:** 1 hour

### Medium Priority (Plan for Next Sprint)

4. **Refactor path construction** (Issue #4)
   - Create `hoosh_config_dir()` helper function
   - Reduce duplication
   - **Estimated effort:** 30 minutes

5. **Improve custom agent support** (Issue #6)
   - Add CLI command: `hoosh agent create <name>`
   - Document custom agent creation process
   - Add validation to prevent `hoosh_*` prefix conflicts
   - **Estimated effort:** 4-6 hours

6. **Add validation for agent references**
   - When loading config, check that all referenced agents can be loaded
   - Provide actionable error messages
   - **Estimated effort:** 1 hour

### Low Priority (Technical Debt)

7. **Remove or use `DEFAULT_CORE_INSTRUCTIONS_FILE`** (Issue #5)
   - Decide on intent and implement or delete
   - **Estimated effort:** 15 minutes

8. **Consider builder pattern for AppConfig**
   - Many optional fields make construction verbose
   - Builder pattern could improve ergonomics
   - **Estimated effort:** 3-4 hours

9. **Add integration tests**
   - Current tests are all unit tests
   - Add tests for full flow: setup → load → agent execution
   - Test project config overrides
   - **Estimated effort:** 4-6 hours

---

## Testing Recommendations

### Current Coverage ✅
- Unit tests for all config operations
- Serialization/deserialization tests
- Merge logic tests
- Validation tests

### Missing Coverage ⚠️

1. **Integration tests for setup flow**
   ```rust
   #[tokio::test]
   async fn test_setup_creates_all_required_files() {
       // Run setup wizard
       // Verify config.toml exists
       // Verify all agent files exist
       // Verify config can be loaded
   }
   ```

2. **Tests for agent file initialization**
   ```rust
   #[test]
   fn test_initialize_default_agents_creates_all_files() {
       // Create temp directory
       // Call initialize_default_agents
       // Verify all expected files exist
       // Verify file contents match embedded prompts
   }
   ```

3. **Tests for project config override**
   ```rust
   #[test]
   fn test_project_config_overrides_global() {
       // Create global config
       // Create project config with overrides
       // Load config
       // Verify project settings take precedence
   }
   ```

---

## Performance Considerations

### Current Implementation
- ✅ Lazy loading: Agent files only read when needed
- ✅ Embedded fallbacks: Built-in prompts compiled into binary
- ✅ Minimal I/O: Config loaded once at startup

### Potential Improvements
- **Cache loaded agents**: Currently `get_agent()` reads from disk every time
  ```rust
  pub struct AgentDefinitionManager {
      config: AppConfig,
      cache: HashMap<String, AgentDefinition>,  // Add cache
  }
  ```
- **Watch config files**: Could detect config changes and hot-reload
  - Probably overkill for CLI application
  - Could be useful for long-running daemon mode

---

## Security Considerations

### Current Security Measures ✅
1. File permissions validation (Unix: 0600)
2. Secure permission setting on file creation
3. No command injection risks (using `fs` module directly)
4. API keys stored in user config, not version controlled

### Potential Improvements
1. **Warn about API keys in project config**
   ```rust
   // .hoosh/config.toml is often version controlled
   if project_path.exists() {
       let content = fs::read_to_string(&project_path)?;
       if content.contains("api_key") {
           eprintln!("⚠️  Warning: API key found in project config");
           eprintln!("   Project config may be version controlled!");
           eprintln!("   Consider moving API key to ~/.config/hoosh/config.toml");
       }
   }
   ```

2. **Support environment variable overrides**
   ```rust
   if let Ok(api_key) = std::env::var("HOOSH_API_KEY") {
       config.api_key = Some(api_key);
   }
   ```

---

## Code Quality Metrics

| Metric | Value | Assessment |
|--------|-------|------------|
| **Lines of Code** | config: 388, agent_definition: 177 | ✅ Reasonable size |
| **Test Coverage** | config: 741 test lines | ✅ Excellent |
| **Cyclomatic Complexity** | Low-Medium | ✅ Easy to understand |
| **Documentation** | Minimal inline docs | ⚠️ Could be improved |
| **Error Handling** | Comprehensive | ✅ Excellent |
| **Code Duplication** | Some path handling | ⚠️ See Issue #4 |

---

## Migration Path (If Refactoring)

If implementing all recommendations:

### Phase 1: Critical Fixes (Week 1)
1. Fix initialization order (Issue #1)
2. Consolidate agent lists (Issue #2)
3. Add validation (Issue #3)

**Risk:** Low - These are mostly internal changes
**Testing:** Existing tests should pass with minimal changes

### Phase 2: Improvements (Week 2)
4. Refactor path handling (Issue #4)
5. Remove unused constant (Issue #5)
6. Add integration tests

**Risk:** Low - Refactoring with good test coverage
**Testing:** New integration tests validate behavior

### Phase 3: Features (Week 3)
7. Custom agent support (Issue #6)
8. Enhanced validation
9. Documentation

**Risk:** Medium - New user-facing features
**Testing:** Manual testing + new feature tests

---

## Conclusion

The config and agent definition architecture is **fundamentally sound** with excellent error handling and test coverage. The primary issues are:

1. **Initialization timing** - Agent files should exist before config references them
2. **Code duplication** - Agent lists and path construction are duplicated
3. **Missing validation** - No checks for file existence at config save time

These are all **fixable without major architectural changes**. The system follows good practices with separation of concerns, proper error handling, and security consciousness.

**Recommendation:** Implement High Priority fixes immediately, then plan Medium Priority improvements for the next iteration.

**Next Steps:**
1. Create GitHub issues for each High Priority item
2. Implement fixes in order of priority
3. Add integration tests as changes are made
4. Update documentation to reflect custom agent creation process
