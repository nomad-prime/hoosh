# Shell Alias Contracts

**Feature**: 002-terminal-modes
**Purpose**: Define shell alias/function templates for @hoosh command integration

## Overview

The `hoosh setup` command installs a shell-specific alias or function that enables the `@hoosh` command for tagged mode usage. This document specifies the exact format for each supported shell.

## Requirements

All shell integrations must:
1. Define @hoosh as a command that calls `hoosh agent --mode tagged`
2. Pass all arguments to hoosh unchanged
3. Export $PPID environment variable (for session file lookup)
4. Be idempotent (safe to run multiple times)
5. Include a comment marker for easy identification and removal

## Supported Shells

### Bash

**Config File**: `~/.bashrc`

**Installation Location**: Append to end of file

**Template**:
```bash
# Added by hoosh setup - https://github.com/your-org/hoosh
@hoosh() {
    hoosh agent --mode tagged "$@"
}
export PPID  # Required for session file lookup
```

**Verification**:
```bash
type @hoosh
# Expected output: @hoosh is a function
```

**Notes**:
- Use function syntax (not alias) for proper argument handling with `"$@"`
- `export PPID` makes terminal PID available to hoosh subprocess
- Works in both Bash 3.x and 4.x+

---

### Zsh

**Config File**: `~/.zshrc`

**Installation Location**: Append to end of file

**Template**:
```zsh
# Added by hoosh setup - https://github.com/your-org/hoosh
@hoosh() {
    hoosh agent --mode tagged "$@"
}
export PPID  # Required for session file lookup
```

**Verification**:
```zsh
which @hoosh
# Expected output: @hoosh () { ... }
```

**Notes**:
- Identical to Bash template (Zsh is backward compatible)
- Works in Zsh 5.x+
- Respects Zsh word splitting rules with `"$@"`

---

### Fish

**Config File Option 1**: `~/.config/fish/config.fish` (legacy)
**Config File Option 2**: `~/.config/fish/functions/@hoosh.fish` (preferred)

**Installation Location**: Create dedicated function file (preferred method)

**Template** (`~/.config/fish/functions/@hoosh.fish`):
```fish
# Added by hoosh setup - https://github.com/your-org/hoosh
function @hoosh --description 'Hoosh tagged mode integration'
    # Fish doesn't have PPID env var, use built-in %self
    set -x HOOSH_TERMINAL_PID %self
    hoosh agent --mode tagged $argv
end
```

**Alternative Template** (if function file fails, append to `config.fish`):
```fish
# Added by hoosh setup - https://github.com/your-org/hoosh
function @hoosh
    set -x HOOSH_TERMINAL_PID %self
    hoosh agent --mode tagged $argv
end
```

**Verification**:
```fish
type @hoosh
# Expected output: @hoosh is a function...
```

**Notes**:
- Fish uses `$argv` instead of `$@`
- Fish doesn't expose PPID directly; use `%self` (Fish parent process ID)
- Hoosh must check `HOOSH_TERMINAL_PID` env var as fallback to PPID
- Function file approach is preferred (cleaner, easier to remove)

---

### PowerShell

**Config File**: `$PROFILE` (run `echo $PROFILE` to find path)

**Common Paths**:
- Windows: `C:\Users\{username}\Documents\PowerShell\Microsoft.PowerShell_profile.ps1`
- macOS/Linux: `~/.config/powershell/Microsoft.PowerShell_profile.ps1`

**Installation Location**: Append to end of file (create if doesn't exist)

**Template**:
```powershell
# Added by hoosh setup - https://github.com/your-org/hoosh
function @hoosh {
    $env:HOOSH_TERMINAL_PID = $PID
    hoosh agent --mode tagged $args
}
```

**Verification**:
```powershell
Get-Command @hoosh
# Expected output: CommandType: Function, Name: @hoosh
```

**Notes**:
- PowerShell uses `$args` for all arguments
- `$PID` is PowerShell's process ID variable (equivalent to PPID)
- Create $PROFILE file if it doesn't exist (run `New-Item -Path $PROFILE -ItemType File -Force`)
- Works in PowerShell 5.1+ and PowerShell Core 7+

---

## Installation Algorithm

The `hoosh setup` command follows this algorithm:

1. **Detect Shell**:
   - Check `$SHELL` environment variable
   - Parse shell binary name (e.g., `/bin/bash` → `bash`)
   - Verify shell is supported (bash, zsh, fish, pwsh)

2. **Locate Config File**:
   - Use shell-specific default paths
   - Verify file exists (create if needed for PowerShell)
   - Backup original file to `.bak` before modification

3. **Check for Existing Installation**:
   - Search config file for "Added by hoosh setup" marker
   - If found, prompt user: "Already installed. Reinstall? [y/N]"
   - If reinstalling, remove old block and install new one

4. **Install Alias**:
   - Append shell-specific template to config file
   - For Fish: prefer function file, fallback to config.fish
   - Add newline before and after for readability

5. **Verify Installation**:
   - Source config file in subprocess
   - Test that @hoosh command exists
   - Warn if verification fails

6. **Prompt to Reload**:
   - Display: "Installation complete! Reload shell with:"
   - Show shell-specific reload command (e.g., `source ~/.bashrc`)

## Removal/Uninstallation

Users can remove @hoosh alias manually by:

1. Open shell config file in editor
2. Find lines between `# Added by hoosh setup` and next blank line
3. Delete those lines
4. Save and reload shell

Future enhancement: `hoosh setup --uninstall` command

## Error Handling

### Installation Errors

| Error | Cause | Resolution |
|-------|-------|------------|
| "Unsupported shell" | Shell not in [bash, zsh, fish, pwsh] | Provide manual instructions for custom shells |
| "Config file not writable" | Permission denied | Run with sudo or fix permissions |
| "Alias conflict" | @hoosh already defined elsewhere | Prompt user to resolve manually |
| "Verification failed" | Shell can't find @hoosh after install | Check PATH includes hoosh binary |

### Runtime Errors

| Error | Cause | Resolution |
|-------|-------|------------|
| "PPID not set" | Missing export or env var | Reinstall with `hoosh setup` |
| "hoosh: command not found" | hoosh not in PATH | Add hoosh to PATH or use full path in alias |
| "Session file permission denied" | ~/.hoosh/sessions/ not writable | Create directory or fix permissions |

## Testing Checklist

For each shell integration:

- [ ] Install on fresh config (no existing @hoosh)
- [ ] Reinstall over existing installation (update case)
- [ ] Verify PPID/PID environment variable is set
- [ ] Test argument passing: `@hoosh echo "hello world"` shows correct args
- [ ] Test multi-word arguments: `@hoosh analyze error.log`
- [ ] Verify session file created in ~/.hoosh/sessions/
- [ ] Test uninstall (manual removal from config)
- [ ] Verify shell reload instructions are correct

## Example Session Flows

### Bash/Zsh Installation
```bash
$ hoosh setup
Detected shell: bash
Config file: /Users/user/.bashrc
Installing @hoosh alias...
✓ Installation complete!

Reload your shell with:
  source ~/.bashrc

Or restart your terminal.

$ source ~/.bashrc
$ @hoosh hello
🔄 Processing...
Hello! How can I help you today?
$ echo $PPID
12345
$ ls ~/.hoosh/sessions/
session_12345.json
```

### Fish Installation
```fish
$ hoosh setup
Detected shell: fish
Installing @hoosh function...
✓ Created /Users/user/.config/fish/functions/@hoosh.fish

Reload your shell with:
  source ~/.config/fish/config.fish

Or restart your terminal.

$ @hoosh test
🔄 Processing...
Test message received.
$ echo $HOOSH_TERMINAL_PID
67890
```

### PowerShell Installation
```powershell
PS> hoosh setup
Detected shell: pwsh
Config file: C:\Users\user\Documents\PowerShell\Microsoft.PowerShell_profile.ps1
Installing @hoosh function...
✓ Installation complete!

Reload your shell with:
  . $PROFILE

Or restart PowerShell.

PS> . $PROFILE
PS> @hoosh hello
🔄 Processing...
Hello! How can I assist?
PS> $env:HOOSH_TERMINAL_PID
45678
```

## Security Considerations

1. **Code Injection**: The alias templates are static (no user input interpolation)
2. **Permission Escalation**: Installation requires write access to user's config file only
3. **Path Hijacking**: Use full path `hoosh` if not in PATH (or verify PATH integrity)
4. **Environment Pollution**: Only export PPID/HOOSH_TERMINAL_PID (minimal impact)

## Future Enhancements

1. **Auto-reload**: Automatically source config after installation
2. **Conflict detection**: Warn if @hoosh already exists as alias/function/binary
3. **Custom prefix**: Allow users to choose prefix other than @ (e.g., `!hoosh`)
4. **Uninstall command**: `hoosh setup --uninstall` to remove integration
5. **Multiple shells**: Support for nushell, xonsh, elvish
