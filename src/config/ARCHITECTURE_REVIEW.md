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
