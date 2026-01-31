# Terminal Display Modes - Usage Examples

## Quick Start

### Option 1: Use CLI Flag (One-Time)

```bash
# Try fullview mode
hoosh --mode fullview

# Try tagged mode
hoosh --mode tagged "what files are in this directory?"
```

### Option 2: Set in Configuration (Persistent)

**Global Configuration** (`~/.config/hoosh/config.toml`):
```toml
terminal_mode = "fullview"
```

**Project Configuration** (`.hoosh/config.toml`):
```toml
terminal_mode = "tagged"
session_context_enabled = true
```

## Mode Selection Examples

### Scenario 1: VSCode User (Recommended: Fullview)

VSCode terminals have broken scrollback. Use fullview mode:

```bash
# One-time usage
hoosh --mode fullview

# Or set in global config
echo 'terminal_mode = "fullview"' >> ~/.config/hoosh/config.toml
```

**What you get:**
- ✅ No visual corruption
- ✅ Internal scrolling (arrow keys, vim j/k, mouse wheel)
- ✅ Fullscreen TUI experience

### Scenario 2: Shell Integration User (Recommended: Tagged)

Want quick queries without hijacking your terminal:

```bash
# One-time setup
hoosh setup  # Installs @hoosh alias

# Reload shell
source ~/.bashrc  # or ~/.zshrc

# Use throughout your session
$ ls
file1.txt  file2.txt

$ @hoosh what files are here?
I can see two files: file1.txt and file2.txt

$ @hoosh summarize file1.txt
[Summary appears, then returns to shell]
```

**What you get:**
- ✅ Non-hijacking (returns control to shell)
- ✅ Context persists across invocations
- ✅ Terminal-native output (stays in history)
- ✅ Works with pipes: `@hoosh "list files" | grep .txt`

### Scenario 3: Standard Terminal User (Default: Inline)

iTerm2, Alacritty, Terminal.app users:

```bash
# Just launch hoosh (defaults to inline)
hoosh

# Or be explicit
hoosh --mode inline
```

**What you get:**
- ✅ Natural terminal scrollback
- ✅ Familiar behavior (current hoosh experience)
- ✅ Works everywhere

## Shell Setup (Tagged Mode)

### Automatic Setup

```bash
hoosh setup
```

This will:
1. Detect your shell (bash, zsh, fish, powershell)
2. Add @hoosh function to shell config
3. Show next steps

### Manual Setup (if needed)

**Bash/Zsh** (`~/.bashrc` or `~/.zshrc`):
```bash
@hoosh() {
    export PPID="$$"
    hoosh agent --mode tagged "$@"
}
```

**Fish** (`~/.config/fish/functions/@hoosh.fish`):
```fish
function @hoosh --description 'Hoosh AI assistant in tagged mode'
    set -x PPID %self
    hoosh agent --mode tagged $argv
end
```

## Configuration Precedence

Mode selection follows this order (highest to lowest):

1. **CLI flag**: `hoosh --mode fullview`
2. **Project config**: `.hoosh/config.toml`
3. **Global config**: `~/.config/hoosh/config.toml`
4. **Default**: `inline`

### Example: Per-Project Modes

```bash
# Project A: Use fullview for VSCode development
cd ~/projects/project-a
echo 'terminal_mode = "fullview"' > .hoosh/config.toml

# Project B: Use tagged for quick queries
cd ~/projects/project-b
echo 'terminal_mode = "tagged"' > .hoosh/config.toml

# Project C: Use default inline mode
cd ~/projects/project-c
# No config needed, uses global or default
```

## Advanced Usage

### Session Context Control (Tagged Mode)

**Enable/Disable Context Persistence:**
```toml
# In config.toml
session_context_enabled = true  # Context persists (default)
# OR
session_context_enabled = false  # Ephemeral mode
```

**View Session Files:**
```bash
ls -lh ~/.hoosh/sessions/
# session_12345.json
# session_67890.json
```

**Clear Session Manually:**
```bash
# Find your terminal PID
echo $$

# Remove session file
rm ~/.hoosh/sessions/session_$$.json
```

### Fullview Mode Controls

**Scrolling:**
- `↑/k` - Scroll up 1 line
- `↓/j` - Scroll down 1 line
- `Page Up` / `Ctrl+U` - Scroll up half page
- `Page Down` / `Ctrl+D` - Scroll down half page
- `Mouse wheel` - Smooth scrolling

**Other:**
- `Ctrl+C` - Quit hoosh
- Terminal resize automatically adjusts viewport

## Troubleshooting

### VSCode: Visual Corruption

**Problem**: Text overlaps or layout breaks in VSCode terminal

**Solution**:
```bash
hoosh --mode fullview
```

Or set permanently:
```toml
terminal_mode = "fullview"
```

### Tagged Mode: @hoosh Not Found

**Problem**: `@hoosh: command not found`

**Solution**:
```bash
# Run setup
hoosh setup

# Reload shell
source ~/.bashrc  # or ~/.zshrc

# Verify
type @hoosh
```

### Session Files Piling Up

**Problem**: Many old session files in `~/.hoosh/sessions/`

**Solution**: Automatic cleanup removes files >7 days old. Manual cleanup:
```bash
rm ~/.hoosh/sessions/session_*.json
```

## Best Practices

### When to Use Each Mode

| Situation | Recommended Mode | Reason |
|-----------|-----------------|---------|
| VSCode development | Fullview | Fixes scrollback issues |
| Quick shell queries | Tagged | Non-disruptive, preserves history |
| Long conversations | Inline or Fullview | TUI experience |
| Piping output | Tagged | Terminal-native, works with pipes |
| Screen recording | Fullview | Controlled viewport |

### Configuration Tips

1. **Global default + project overrides**:
   ```bash
   # Set global default
   echo 'terminal_mode = "inline"' > ~/.config/hoosh/config.toml

   # Override for VSCode projects
   cd ~/vscode-projects
   echo 'terminal_mode = "fullview"' > .hoosh/config.toml
   ```

2. **Team configurations**:
   ```bash
   # Commit project config for team consistency
   git add .hoosh/config.toml
   git commit -m "Configure hoosh for fullview mode"
   ```

3. **Quick mode switching**:
   ```bash
   # Temporarily try different mode
   hoosh --mode fullview  # Just for this session
   ```

## FAQ

**Q: Can I switch modes during a session?**
A: Not in v1. Mode is selected at startup. Quit (Ctrl+C) and restart with different mode.

**Q: Do session files contain sensitive data?**
A: Yes, they contain conversation messages. Stored in `~/.hoosh/sessions/` with standard file permissions.

**Q: Can I use @hoosh in different terminals simultaneously?**
A: Yes, each terminal has its own session file (keyed by PID).

**Q: Does tagged mode work with conversation storage?**
A: Yes, they're independent features. Both can be enabled simultaneously.
