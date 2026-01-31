# Quickstart: Terminal Display Modes

**Feature**: 002-terminal-modes
**Audience**: Hoosh users
**Status**: Implementation guide

## Overview

Hoosh supports three terminal display modes to optimize your experience across different terminal environments:

- **Inline** (default): Best for standard terminals with working scrollback
- **Fullview**: Best for VSCode terminals and environments with broken scrollback
- **Tagged**: Best for non-disruptive shell integration with @hoosh commands

---

## Quick Mode Selection Guide

### When to Use Each Mode

| Mode | Use When | Pros | Cons |
|------|----------|------|------|
| **Inline** | Standard terminal (iTerm2, Alacritty, Terminal.app) | Natural scrollback, minimal overhead | Breaks in VSCode |
| **Fullview** | VSCode integrated terminal, tmux/screen | Internal scrolling, works everywhere | Takes over screen |
| **Tagged** | Want shell integration, preserve terminal history | Non-hijacking, shell-native | Context limited to terminal session |

### Visual Comparison

**Inline Mode**:
```
$ hoosh
> Hello
Assistant: Hi there! How can I help?
> What's the weather?
Assistant: I don't have access to weather...
[continues with native terminal scrollback]
```

**Fullview Mode**:
```
┌─────────────────────────────────────────┐
│ Hoosh - Fullview Mode                   │
├─────────────────────────────────────────┤
│ > Hello                                 │
│ Assistant: Hi there! How can I help?    │
│ > What's the weather?                   │
│ Assistant: I don't have access to...    │
│                                         │
│ [Internal scrolling with j/k/arrows]    │
│                                         │
│ > _                                     │
└─────────────────────────────────────────┘
```

**Tagged Mode**:
```
$ ls
file1.txt  file2.txt
$ @hoosh what files are here?
🔄 Processing...
I can see two files: file1.txt and file2.txt
$ pwd
/Users/you/projects
$ @hoosh summarize file1.txt
🔄 Processing...
[Summary of file1.txt]
$ ls
file1.txt  file2.txt
```

---

## Getting Started

### 1. Choose Your Mode

#### Option A: Use CLI Flag (One-Time)
```bash
# Try fullview mode
hoosh --mode fullview

# Try tagged mode (after setup)
hoosh agent --mode tagged "hello"
```

#### Option B: Set in Config (Persistent)

**Project-specific** (`.hoosh/config.toml`):
```toml
terminal_mode = "fullview"  # or "inline" or "tagged"
```

**Global default** (`~/.hoosh/config.toml`):
```toml
terminal_mode = "inline"
session_context_enabled = true
```

### 2. Tagged Mode Setup (One-Time)

Tagged mode requires a shell alias. Run the setup command:

```bash
hoosh setup
```

This will:
1. Detect your shell (bash, zsh, fish, powershell)
2. Add @hoosh alias to your shell config
3. Prompt you to reload shell

**Manual verification**:
```bash
# Reload shell
source ~/.bashrc  # or ~/.zshrc for zsh

# Test alias
@hoosh hello

# Should respond and return to shell
```

---

## Mode-Specific Features

### Inline Mode

**Controls**:
- Type message, press Enter to send
- Ctrl+C to quit
- Use terminal scrollback (Command+↑ on Mac, Shift+PageUp on Linux)

**Best Practices**:
- Works great in standard terminals
- If you see visual corruption, switch to fullview
- Conversation scrolls naturally with terminal history

### Fullview Mode

**Controls**:
- Type message, press Enter to send
- **Scroll**: Arrow keys, Page Up/Down, j/k (vim-style), mouse wheel
- Ctrl+C to quit
- Terminal resize automatically adjusts viewport

**Keyboard Shortcuts**:
| Key | Action |
|-----|--------|
| ↑/k | Scroll up 1 line |
| ↓/j | Scroll down 1 line |
| Page Up / Ctrl+U | Scroll up half page |
| Page Down / Ctrl+D | Scroll down half page |
| Home / g | Scroll to top |
| End / G | Scroll to bottom |
| Mouse wheel | Scroll smoothly |

**Best Practices**:
- Recommended for VSCode users
- Use when native scrollback doesn't work
- Scrolls to bottom automatically on new messages

### Tagged Mode

**Usage**:
```bash
# Single query
@hoosh what is the current directory?

# Multi-word queries (quotes optional if no special chars)
@hoosh analyze error.log

# Slash commands work
@hoosh /commit

# Context preserved within terminal session
@hoosh what did we just do?
```

**Features**:
- Session context persists across @hoosh invocations in same terminal
- All output stays in terminal history (like any bash command)
- Permission prompts use simple text (y/n input)
- Returns control to shell after each query

**Best Practices**:
- Use for quick queries without hijacking terminal
- Context clears when terminal closes
- Disable context persistence in config if unwanted

---

## Configuration Reference

### Terminal Mode Settings

**Config File Location**:
- Project: `.hoosh/config.toml` (takes precedence)
- Global: `~/.hoosh/config.toml`

**Available Options**:
```toml
# Terminal display mode
# Options: "inline" (default), "fullview", "tagged"
terminal_mode = "inline"

# Enable session context persistence for tagged mode
# Default: true
# Set to false for ephemeral @hoosh sessions (no context between invocations)
session_context_enabled = true
```

### Priority Order

Mode selection follows this priority (highest first):
1. CLI flag: `--mode fullview`
2. Project config: `.hoosh/config.toml`
3. Global config: `~/.hoosh/config.toml`
4. Default: `inline`

**Example**:
```bash
# Force fullview even if config says inline
hoosh --mode fullview

# Use config setting
hoosh  # Reads from .hoosh/config.toml or ~/.hoosh/config.toml
```

---

## Troubleshooting

### Inline Mode Issues

**Problem**: Visual corruption or broken layout in VSCode
**Solution**: Switch to fullview mode
```bash
hoosh --mode fullview
```

**Problem**: Can't scroll back through conversation
**Solution**: This is normal for inline mode. Use fullview for internal scrolling.

### Fullview Mode Issues

**Problem**: Scrolling doesn't work
**Solution**: Check your terminal supports mouse events. Try keyboard shortcuts instead (j/k, arrows).

**Problem**: Text wrapping is weird after resize
**Solution**: Press Ctrl+L to force redraw, or restart hoosh.

### Tagged Mode Issues

**Problem**: @hoosh command not found
**Solution**: Run `hoosh setup` and reload shell
```bash
hoosh setup
source ~/.bashrc  # or ~/.zshrc
```

**Problem**: Context not persisting between @hoosh calls
**Solution**: Check `session_context_enabled = true` in config. Verify session file exists:
```bash
ls ~/.hoosh/sessions/
```

**Problem**: Session files piling up
**Solution**: Cleanup runs automatically (7-day threshold). Manual cleanup:
```bash
hoosh cleanup-sessions  # Planned command
```

---

## Advanced Usage

### Switching Modes Mid-Project

You can change modes by updating config:

```bash
# Switch to fullview for this project
echo 'terminal_mode = "fullview"' >> .hoosh/config.toml

# Next hoosh invocation uses fullview
hoosh
```

### Session Context Management

**View session files**:
```bash
ls -lh ~/.hoosh/sessions/
```

**Disable context for specific terminal**:
```bash
# Set in environment before launching hoosh
SESSION_CONTEXT_ENABLED=false @hoosh query
```

**Clear session manually**:
```bash
# Find your terminal PID
echo $$

# Remove session file
rm ~/.hoosh/sessions/session_$$.json
```

### Shell Integration Customization

The `hoosh setup` command adds this to your shell config:

**Bash/Zsh** (`~/.bashrc` or `~/.zshrc`):
```bash
# Added by hoosh setup
@hoosh() {
    hoosh agent --mode tagged "$@"
}
export PPID  # Needed for session file lookup
```

**Fish** (`~/.config/fish/config.fish`):
```fish
# Added by hoosh setup
function @hoosh
    hoosh agent --mode tagged $argv
end
```

You can customize this if needed (e.g., add default flags).

---

## Examples

### Example 1: VSCode User Workflow

```bash
# First time setup
cd my-project
echo 'terminal_mode = "fullview"' > .hoosh/config.toml

# Launch hoosh
hoosh

# Use fullview controls
# j/k to scroll, mouse wheel, arrows
# Ctrl+C to quit
```

### Example 2: Shell Integration Workflow

```bash
# One-time setup
hoosh setup
source ~/.bashrc

# Use throughout your terminal session
$ @hoosh what's in this directory?
[Response with file listing]

$ ls -la
[Shell command output]

$ @hoosh explain error.log
[Analysis of error.log]

$ @hoosh what errors did you find?
[Uses context from previous query]
```

### Example 3: Mixed Mode Usage

```bash
# Quick query in tagged mode
@hoosh what is 2+2?

# Complex task in fullview mode
hoosh --mode fullview
> Help me refactor main.rs
[Long conversation with scrolling]
[Ctrl+C to quit]

# Back to shell
```

---

## Performance Notes

- **Session file I/O**: <1ms per read/write (negligible overhead)
- **Fullview resize**: <200ms reflow time
- **Tagged mode return**: <1s from query to shell prompt

---

## Next Steps

1. Try all three modes to see which fits your workflow
2. Set your preferred mode in `.hoosh/config.toml`
3. For shell integration, run `hoosh setup` and start using @hoosh
4. Report issues or feedback via hoosh GitHub repository

---

## FAQ

**Q: Can I switch modes during a hoosh session?**
A: Not in v1. Mode is selected at startup. You can quit (Ctrl+C) and restart with different mode.

**Q: Do session files contain sensitive data?**
A: Yes, they contain your conversation messages. They're stored in `~/.hoosh/sessions/` with standard file permissions. Future versions may add encryption.

**Q: What happens if I delete my session file?**
A: Next @hoosh invocation starts a fresh context (like a new conversation).

**Q: Can I use @hoosh in different terminals simultaneously?**
A: Yes, each terminal has its own session file (keyed by terminal PID).

**Q: Does tagged mode work with conversation storage?**
A: Yes, they're independent. Session files (tagged mode) and conversation storage can both be enabled.

**Q: How do I uninstall the @hoosh alias?**
A: Edit your shell config (`~/.bashrc`, `~/.zshrc`, etc.) and remove the lines added by `hoosh setup`.
