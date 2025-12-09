# Feature Specification: Custom Commands

**Feature Branch**: `001-custom-commands`
**Created**: 2025-12-09
**Status**: Draft
**Input**: User description: "I want to have hoosh understand custom commands. when first running hoosh shoud check .hoosh/commands if folder does not exist, it should create. The user should be able to define custom commmand, we will use claude code convention (md files) for custom commands, look at @.claude/commands/ for reference"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Create Custom Command (Priority: P1)

A user wants to create a custom command that automates a repetitive task they perform frequently in Hoosh (e.g., running specific analysis, generating reports, or executing a workflow). They should be able to define this command using a simple markdown file format.

**Why this priority**: This is the core value proposition of the feature. Without the ability to create custom commands, the feature provides no user value. This enables users to extend Hoosh's functionality to match their specific workflows.

**Independent Test**: Can be fully tested by creating a markdown file in `.hoosh/commands/` directory, running Hoosh, and verifying the custom command is available and executes the defined behavior.

**Acceptance Scenarios**:

1. **Given** the user has Hoosh installed, **When** they create a file named `mycommand.md` in `.hoosh/commands/` with valid command syntax, **Then** Hoosh loads and recognizes `/mycommand` as an available command
2. **Given** a custom command is defined in `.hoosh/commands/analyze.md`, **When** the user types `/analyze` in Hoosh, **Then** the command executes according to the instructions in the markdown file
3. **Given** the user creates a custom command with parameters, **When** they invoke it with `/mycommand arg1 arg2`, **Then** the arguments are passed to the command handler correctly

---

### User Story 2 - Auto-Create Commands Directory (Priority: P1)

When a user runs Hoosh for the first time or on a new project, the `.hoosh/commands` directory doesn't exist yet. The system should automatically create this directory so users can immediately start adding custom commands without manual setup.

**Why this priority**: This is essential for user experience. Requiring manual directory creation creates friction and confusion. This is part of the MVP as it enables the primary use case (creating custom commands) without additional setup steps.

**Independent Test**: Can be fully tested by running Hoosh in a directory without `.hoosh/commands`, verifying the directory is created automatically, and confirming users can then add command files.

**Acceptance Scenarios**:

1. **Given** a user runs Hoosh in a directory without a `.hoosh/commands` folder, **When** Hoosh initializes, **Then** the `.hoosh/commands` directory is created automatically
2. **Given** the `.hoosh/commands` directory was auto-created, **When** the user adds a command file, **Then** the command is immediately available on next run
3. **Given** the `.hoosh/commands` directory already exists, **When** Hoosh starts, **Then** no error occurs and existing commands are loaded normally

---

### User Story 3 - List Available Custom Commands (Priority: P2)

Users need a way to discover what custom commands are available in their project. They should be able to see a list of all custom commands with their descriptions.

**Why this priority**: While not strictly necessary for the feature to work, discoverability is important for usability. Users can technically open the `.hoosh/commands` folder to see files, but an in-app listing improves the experience significantly.

**Independent Test**: Can be fully tested by adding multiple command files to `.hoosh/commands/`, running a help or list command, and verifying all custom commands appear with their descriptions.

**Acceptance Scenarios**:

1. **Given** the user has defined 3 custom commands in `.hoosh/commands/`, **When** they request a list of available commands, **Then** all 3 custom commands are displayed with their descriptions
2. **Given** no custom commands exist, **When** the user requests a list, **Then** an appropriate message indicates no custom commands are defined
3. **Given** a custom command file lacks a description, **When** listing commands, **Then** the command appears with a default description or indication that no description is provided

---

### User Story 4 - Command Validation and Error Handling (Priority: P3)

When users create custom command files, they may make syntax errors or use invalid formats. The system should validate command files and provide clear error messages when issues are detected.

**Why this priority**: While helpful for improving user experience, the feature can function without sophisticated validation. Users will learn from trial and error. However, good error messages significantly reduce frustration and improve adoption.

**Independent Test**: Can be fully tested by creating invalid command files (malformed markdown, missing required fields, syntax errors) and verifying that Hoosh reports clear, actionable error messages.

**Acceptance Scenarios**:

1. **Given** a custom command file has invalid markdown syntax, **When** Hoosh loads commands, **Then** an error message identifies the problematic file and describes the syntax issue
2. **Given** a command file is missing required metadata, **When** Hoosh attempts to load it, **Then** a clear error indicates which required fields are missing
3. **Given** multiple command files with errors exist, **When** Hoosh starts, **Then** all errors are reported without crashing, and valid commands still load successfully

---

### Edge Cases

- What happens when two custom command files have the same name (e.g., `analyze.md` in different subdirectories)?
- How does the system handle command files that are too large or cause performance issues?
- What if a user creates a custom command that conflicts with a built-in Hoosh command name?
- How does the system behave if the `.hoosh/commands` directory exists but isn't readable due to permissions?
- What happens when a command file is modified while Hoosh is running?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST check for the existence of `.hoosh/commands` directory on startup
- **FR-002**: System MUST automatically create `.hoosh/commands` directory if it doesn't exist
- **FR-003**: System MUST scan `.hoosh/commands` directory for markdown files (`.md` extension)
- **FR-004**: System MUST parse each markdown file following the same convention as `.claude/commands/` files
- **FR-005**: System MUST extract command metadata from markdown frontmatter (description, parameters, etc.)
- **FR-006**: System MUST register custom commands and make them available via slash command syntax (e.g., `/commandname`)
- **FR-007**: System MUST support passing arguments to custom commands
- **FR-008**: System MUST handle markdown files without frontmatter gracefully with appropriate defaults
- **FR-009**: System MUST provide clear error messages when command files are malformed or invalid
- **FR-010**: Custom commands MUST be loaded after built-in commands to allow built-in commands to take precedence in case of naming conflicts
- **FR-011**: System MUST support command descriptions that appear in help/list outputs
- **FR-012**: System MUST allow users to view all available custom commands and their descriptions

### Key Entities

- **Custom Command**: Represents a user-defined command loaded from a markdown file
  - Name: Derived from filename (without .md extension)
  - Description: Extracted from frontmatter metadata
  - Content: The markdown body containing instructions/prompts
  - Arguments: Optional parameters the command accepts

- **Command File**: Physical markdown file in `.hoosh/commands/` directory
  - Path: Location in filesystem
  - Frontmatter: YAML metadata section (description, handoffs, etc.)
  - Body: Markdown content defining command behavior

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can create a custom command and have it available in Hoosh within 30 seconds of creating the file (after restart/reload)
- **SC-002**: System successfully loads and executes custom commands in 100% of cases where valid markdown files are provided
- **SC-003**: Users can discover all available custom commands without needing to navigate filesystem manually
- **SC-004**: New Hoosh users can start using custom commands without manual directory creation (0 setup steps required)
- **SC-005**: 95% of syntax errors in custom command files result in clear, actionable error messages that help users fix the issue

## Assumptions

- Custom command markdown files follow the same format as Claude Code commands (YAML frontmatter + markdown body)
- The `.hoosh/commands` directory is located at the project root level (same directory where Hoosh is executed)
- Users understand basic markdown syntax or can reference existing examples in `.claude/commands/`
- Command names are derived from filenames and must be valid filesystem names
- Commands are loaded once at startup (dynamic reloading while running is out of scope for MVP)
- Built-in commands take precedence over custom commands if names conflict
