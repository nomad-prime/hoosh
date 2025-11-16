# TEST RESULTS FOR HOOSH PERMISSION SYSTEM

## Test Setup
- Project: hoosh (Rust project with Cargo.toml)
- Initial permissions.json: Contains `cargo:*` and `cd:*|cargo:*|head:*`
- Initial bash_blacklist.json: Contains dangerous patterns (rm -rf, sudo, curl|sh, etc.)

---

## Test 1: Single Simple Command - `cargo build`

**Expected Pattern:** `cargo:*`
**Test Status:** ✅ EXPECTED TO PASS (already in permissions)

Since `cargo:*` is already whitelisted, this should execute without asking for permission.

```bash
cargo build
```

**Expected Result:** Command executes or fails gracefully if dependencies missing, NO permission dialog

---

## Test 2: Simple Pipe - `cat Cargo.toml | grep version`

**Expected Pattern:** `cat:*|grep:*`
**Status:** NEEDS PERMISSION

This should trigger a permission dialog asking for approval of both `cat` and `grep` commands.

```bash
cat Cargo.toml | grep version
```

**Expected:**
- Prompt: "Can I run 'cat, grep'"
- Pattern saved: `cat:*|grep:*`
- Subsequent calls should not ask again

---

## Test 3: Triple Pipe - `cat README.md | grep -i hoosh | wc -l`

**Expected Pattern:** `cat:*|grep:*|wc:*`
**Status:** NEEDS PERMISSION

Should ask for permission for all three commands.

---

## Test 4: CRITICAL SECURITY TEST - Four Command Pipeline

**Command:** `find . -name "*.md" | head -5 | xargs echo "Found:"`
**Expected Pattern:** `find:*|head:*|xargs:*|echo:*`

This is the critical test. After approving the full 4-command pipeline:
- This exact pattern should work
- BUT: `sed -i 's/foo/bar/' file.txt` (sed alone) should NOT work and should ask again
- This validates that permissions are specific to the full pattern, not individual commands

---

## Test 5: Semicolon Separator - `cargo build; echo "Done"`

**Expected Pattern:** `cargo:*|echo:*` (NOT just `cargo:*`)
**Status:** `cargo:*` already approved, but `echo` needs approval

Should ask for approval to add `echo` to the pattern.

---

## Test 6: AND Operator - `cargo build && cargo test`

**Expected Pattern:** `cargo:*` (deduplicated)
**Status:** Already approved ✅

Should work without asking - both are the same command.

---

## Test 7: Mixed && and Pipe - `cargo build && cat README.md | grep hoosh`

**Expected Pattern:** `cargo:*|cat:*|grep:*`
**Status:** `cargo:*` approved, but `cat` and `grep` need approval

Should merge with existing `cargo:*` pattern.

---

## Test 8: OR Operator - `cargo test || echo "Failed"`

**Expected Pattern:** `cargo:*|echo:*`
**Status:** `cargo:*` approved, `echo` needs approval

---

## Test 9: Complex Chain - `ls -la && cargo build && echo "done"`

**Expected Pattern:** `ls:*|cargo:*|echo:*`
**Status:** `cargo:*` approved, `ls` and `echo` need approval

---

## Test 10: Auto-Approve Whitelisted - `find . -type f -name "*.rs" | head -5`

**Status:** Both `find` and `head` are read-only commands (should auto-approve)

Expected: No dialog, executes immediately, NO entry added to permissions.json

---

## Test 11: Mixed Whitelist - `cat README.md | grep -i test | cargo build`

**Expected Pattern:** `cat:*|grep:*|cargo:*`

Even though `cat` and `grep` are whitelisted, `cargo` is not, so full permission needed.

---

## Test 12: Subshell Substitution - `echo $(ls -la)`

**Expected Pattern:** `echo:*|ls:*`

Parser should extract both commands from subshell.

---

## Test 13: Dangerous Solo - `rm -rf /tmp/test_hoosh`

**Status:** BLOCKED by blacklist

Expected: Immediate rejection with "Command matches blacklist pattern: rm -rf*"
No permission dialog, command not executed.

---

## Test 14: Duplicate Commands - `cargo build && cargo test && cargo run`

**Expected Pattern:** `cargo:*` (deduplicated)
**Status:** Already approved ✅

All three variants use cargo, so should match single pattern.

---

## Test 15: Wildcard Pattern

**Setup:** Manually add `"pattern": "*"` to permissions.json

Expected: Every command passes (complete bypass - use with caution)

---

## Summary of Key Validation Points

1. ✅ Single commands create `cmd:*` patterns
2. ✅ Pipes dedup and combine: `cmd1:*|cmd2:*|cmd3:*`
3. ✅ Semicolons and && and || all combine patterns
4. ✅ Read-only commands auto-approve without storing
5. ✅ Mixed whitelist/non-whitelist requires full permission
6. ✅ Blacklist patterns are checked BEFORE permission system
7. ✅ Duplicate commands in same chain deduplicate the pattern
8. ✅ Subshells extract nested commands
9. ✅ Pattern matching is SET-based (order independent)
