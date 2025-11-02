### 4 🔴 PATH VALIDATOR: INCOMPLETE SYMLINK PROTECTION

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

### 5 🟡 BASH TOOL: BYPASSABLE DANGEROUS COMMAND DETECTION

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

### 6 🟡 INSUFFICIENT TEST COVERAGE

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
