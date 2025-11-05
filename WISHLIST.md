### Issues

#### Add messages midflight

want to add messages as llm is working same as claude code

#### Error Log

Add error logs file, have option in hoosh to examine those logs

#### First Ctrl+C

should always cancel current operation instead of exiting the program, with second Ctrl+C exiting

#### Circuit breaker

for LLM calls when repeated failures occur

#### Memory-> and the tool to load

I find myself referencing previous conversations often, so having a way to load previous conversations into memory would
be helpful. Maybe a command like /load_conversation <conversation_id> that fetches and loads the conversation into the
current context.

#### Status Flaky

after approval rejection status line stucks on processing

#### Running Todos

currently there are no running todos like in claude code, this can create a better ux and system prompting for the model
CRUDing todos could also be a tool call for the model

### System Reminder

Claude code uses system reminder to observe the changes done in the system in realtime

### switch backend

currently switching models and backends is only possible through configs. lets make it into a command

### Tool Status

currently I add the tool and then append tool result (preview) in messages. Ideally there is a space above status and
keeps tool calls there
(especially because tool calls can be in parallel). one the tool call is complete I can add it to message history, till
then I keep it above status bar

### File Expansion

if a file is referenced in input, file read should be shown afterwards

### Approve Plan

very often AI creates a plan before moving on, this should be a

### ways forward (question and answer tool)

have llm give forks as to possible implementations, user chooses the way

### Tools

Core Development Tools

- Read - Read files from the filesystem (supports code, images, PDFs, Jupyter notebooks)
- Write - Create new files or overwrite existing ones
- Edit - Perform exact string replacements in files
- NotebookEdit - Edit Jupyter notebook cells
- Bash - Execute shell commands (git, npm, docker, etc.)
- BashOutput - Retrieve output from background bash shells
- KillShell - Terminate background bash shells

Search & Navigation Tools

- Glob - Find files using glob patterns (e.g., **/*.js)
- Grep - Search file contents using regex patterns (powered by ripgrep)
- Task - Launch specialized agents for complex tasks:
    - general-purpose - Multi-step tasks and research
    - Explore - Fast codebase exploration
    - Plan - Planning and analysis
    - statusline-setup - Configure status line settings

Web Tools

- WebSearch - Search the web for current information
- WebFetch - Fetch and analyze content from URLs

Planning & Organization

- TodoWrite - Create and manage task lists for tracking progress
- ExitPlanMode - Exit planning mode when ready to implement

User Interaction

- AskUserQuestion - Ask users questions with multiple choice options

Extensions

- Skill - Execute skills for specialized capabilities
- SlashCommand - Execute custom slash commands from .claude/commands/

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

---
