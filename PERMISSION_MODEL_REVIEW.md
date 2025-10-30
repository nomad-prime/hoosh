## 1. Architecture Overview

## 3. Design Issues

### 3.1 🟡 TIGHT COUPLING TO EVENT SYSTEM

**Problem:** `PermissionManager` is tightly coupled to the event system.

**Location:** `src/permissions/mod.rs:98-108`

  ```rust
  pub struct PermissionManager {
    skip_permissions: bool,
    default_permission: PermissionLevel,
    event_sender: mpsc::UnboundedSender<crate::conversations::AgentEvent>,  // ⚠️ TIGHT COUPLING
    response_receiver: Arc<Mutex<mpsc::UnboundedReceiver<...>>>,
    // ...
}
  ```

**Issues:**

- Can't use PermissionManager without AgentEvents
- Hard to test in isolation
- Violates Dependency Inversion Principle
- Couples domain logic to event layer

**Impact:** MEDIUM - Reduces testability, flexibility

**Recommendation:**

- Extract permission decision interface
- Use trait for permission events: `trait PermissionEvents`
- Inject events implementation via dependency injection
- Make PermissionManager events-agnostic

  ---

### 3.2 🟡 OPERATION TYPE CONSTRUCTION IS VERBOSE AND ERROR-PRONE

**Problem:** Every tool must manually construct `OperationType` with 6 parameters including nested `OperationDisplay`.

**Location:** Example from `write_file.rs:158-173`

  ```rust
  Ok(OperationType::new(
"write_file",
normalized_path.clone(),
false,
is_destructive,
parent_dir,
OperationDisplay {
name: "Write".to_string(),
approval_title: format!("Write({})", args.path),
approval_prompt: format!("Write to file: {}", args.path),
persistent_approval: format!("don't ask me again..."),
},
))
  ```

**Issues:**

- Boilerplate repeated in every tool
- Easy to make mistakes (wrong flags, inconsistent messages)
- No compile-time validation
- Hard to maintain consistency

**Impact:** MEDIUM - Code duplication, maintenance burden

**Recommendation:**

- Use builder pattern: `OperationType::builder()`
- Provide sensible defaults
- Create helper methods for common patterns
- Consider deriving from tool metadata

  ---

### 3.3 🟡 PERMISSION STORAGE FORMAT IS NOT VERSIONED PROPERLY

**Problem:** While there's a `version` field, there's no migration logic or version checking.

**Location:** `src/permissions/storage.rs:9-15`

  ```rust
  pub struct PermissionsFile {
    pub version: u32,  // ⚠️ No migration logic
    pub allow: Vec<PermissionRule>,
    pub deny: Vec<PermissionRule>,
}
  ```

**Issues:**

- Version field exists but is never checked
- No migration path if format changes
- Could break on version mismatch
- No validation of loaded data

**Impact:** LOW-MEDIUM - Future maintenance burden

**Recommendation:**

- Add version validation on load
- Implement migration framework
- Handle unknown versions gracefully
- Add schema validation

  ---

### 3.4 🟡 PATTERN MATCHING LOGIC IS FRAGMENTED

**Problem:** Pattern matching logic exists in multiple places with different implementations.

**Locations:**

- `storage.rs:118-145` (PermissionRule::matches)
- `bash.rs:50-150` (dangerous command detection)
- File tools (path validation)

**Issues:**

- Inconsistent pattern syntax
- No shared pattern matching library
- Hard to maintain and test
- Potential for bugs in edge cases

**Impact:** MEDIUM - Maintenance burden, potential security issues

**Recommendation:**

- Create unified pattern matching module
- Use established glob library consistently
- Document pattern syntax clearly
- Add comprehensive pattern tests

  ---

## 4. Code Smells

### 4.1 🟡 GOD OBJECT: PermissionManager

**Problem:** `PermissionManager` does too many things.

**Responsibilities:**

1. Permission checking
2. User interaction (via events)
3. Persistence management
4. Cache management (via PermissionsFile)
5. Project root management
6. Request ID generation

**Lines of Code:** 350+ in single file

**Violation:** Single Responsibility Principle

**Recommendation:**

- Split into: `PermissionChecker`, `PermissionStorage`, `PermissionUI`
- Use composition over god object
- Each class has single, clear responsibility

  ---

### 4.3 🟡 SHOTGUN SURGERY

**Problem:** Changing permission behavior requires touching many files.

**Example:** To add a new permission scope type:

- `permissions/mod.rs` - Add enum variant
- `permissions/storage.rs` - Update pattern matching
- `tool_executor.rs` - Update permission checks
- Multiple tools - Update to_operation_type
- TUI handlers - Update UI

**Impact:** MEDIUM - High change cost, error-prone

**Recommendation:**

- Centralize permission logic
- Use visitor pattern or strategy pattern
- Reduce coupling between components

  ---

### 4.5 🟡 INCOMPLETE ABSTRACTION

**Problem:** `Tool` trait exposes implementation details.

**Location:** `src/tools/mod.rs:13-73`

  ```rust
  pub trait Tool: Send + Sync {
    fn to_operation_type(&self, args: &Value) -> Result<OperationType>;  // ⚠️ Leaky
    async fn check_permission(&self, args: &Value, pm: &PermissionManager) -> Result<bool>;
    fn read_only(&self) -> bool;  // ⚠️ Redundant with to_operation_type
    fn writes_safe(&self) -> bool;  // ⚠️ Redundant
}
  ```

**Issues:**

- Tool needs to know about PermissionManager (coupling)
- Redundant methods (`read_only` vs operation type flags)
- Leaks permission implementation details to tools
- Hard to change permission system without updating all tools

**Impact:** MEDIUM - Tight coupling, hard to evolve

**Recommendation:**

- Remove permission logic from Tool trait
- Use decorator pattern for permission checking
- Keep Tool focused on execution
- Let PermissionManager inspect tool metadata

  ---

## 5. Security Issues

### 5.1 🔴 PATH VALIDATOR: INCOMPLETE SYMLINK PROTECTION

**Problem:** Path validation doesn't fully protect against symlink attacks.

**Location:** `src/security/path_validator.rs:35-70`

**Issue:** For non-existent paths, only the parent is canonicalized. A symlink in the path chain could escape the
working directory.

**Attack scenario:**

  ```bash
  # Attacker creates symlink in working dir
  ln -s /etc evil_dir
  # Tool creates file in evil_dir/passwd
  # Path validator checks parent (working_dir) but file ends up in /etc/passwd
  ```

**Impact:** HIGH - Potential security bypass

**Recommendation:**

- Recursively resolve all path components
- Reject paths containing symlinks by default
- Add explicit symlink policy configuration
- Test with nested symlink scenarios

  ---

### 5.2 🟡 BASH TOOL: BYPASSABLE DANGEROUS COMMAND DETECTION

**Problem:** Blacklist approach to dangerous commands is easily bypassed.

**Location:** `src/tools/bash.rs:50-120`

**Bypass examples:**

  ```bash
  r\m -rf /              # Escaped character
  $(echo rm) -rf /       # Command substitution
  rm${IFS}-rf /          # Variable expansion
  'r''m' -rf /           # Quote concatenation
  ```

**Impact:** MEDIUM-HIGH - Security feature can be circumvented

**Recommendation:**

- Switch to whitelist: only allow safe commands
- Use shell parser (e.g., `shell-words` crate)
- Sandbox bash execution (containers, restricted shell)
- Add explicit dangerous command confirmation

  ---

### 5.3 🟡 NO RATE LIMITING ON PERMISSION REQUESTS

**Problem:** No protection against permission request spam.

**Scenario:** Malicious or buggy code could flood user with permission requests.

**Impact:** LOW-MEDIUM - UX degradation, potential DoS

**Recommendation:**

- Add rate limiting: max N requests per minute
- Batch similar requests
- Add "deny all for session" option

  ---

## 6. Testing Issues

### 6.1 🟡 INSUFFICIENT TEST COVERAGE

**Current state:**

- 14 unit tests for permissions
- Tests mostly focus on happy path
- Missing edge cases and error conditions
- No integration tests

**Missing test scenarios:**

- Concurrent permission requests
- Permission file corruption
- Race conditions in cache
- Symlink attack scenarios
- Pattern matching edge cases
- Error recovery

**Recommendation:**

- Achieve 80%+ code coverage
- Add property-based tests for pattern matching
- Add concurrency tests
- Test error paths explicitly

  ---

### 6.2 🟡 TESTS USE MOCKS INSTEAD OF REAL COMPONENTS

**Problem:** Tests create test-specific mocks rather than using production code.

**Example:** `src/permissions/mod.rs:397-420`

  ```rust
  fn create_test_manager() -> PermissionManager {
    let (event_tx, _) = mpsc::unbounded_channel();  // ⚠️ Fake channel
    let (_, response_rx) = mpsc::unbounded_channel();
    PermissionManager::new(event_tx, response_rx)
}
  ```

**Issues:**

- Tests don't exercise real behavior
- Mocks can drift from production
- Integration issues not caught

**
