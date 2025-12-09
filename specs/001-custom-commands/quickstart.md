# Quickstart: Custom Commands

**Feature**: Custom Commands
**Audience**: Hoosh Users
**Date**: 2025-12-09

## What Are Custom Commands?

Custom commands allow you to extend Hoosh with your own slash commands. Define reusable prompts and workflows as simple markdown files, and invoke them with `/commandname` just like built-in commands.

**Example Use Cases**:
- Code analysis workflows (`/analyze`)
- Documentation generation (`/generate-docs`)
- Code review checklists (`/review-pr`)
- Project-specific commands (`/deploy-check`)

## Quick Start (30 Seconds)

### 1. Create Your First Command

Navigate to your project directory and create a command file:

```bash
# Hoosh automatically creates .hoosh/commands/ on first run
# But you can create it manually:
mkdir -p .hoosh/commands

# Create your first custom command
cat > .hoosh/commands/analyze.md << 'EOF'
---
description: Analyze codebase for technical debt
---

Please analyze the codebase in: $ARGUMENTS

Focus on:
1. Code complexity
2. Potential refactoring opportunities
3. Technical debt indicators

Provide specific, actionable recommendations.
EOF
```

### 2. Start Hoosh

```bash
hoosh
```

Hoosh automatically:
- Checks for `.hoosh/commands` directory
- Creates it if missing (zero setup!)
- Loads all `.md` files as custom commands

### 3. Use Your Command

```
You: /analyze src/
```

Hoosh processes your custom command and sends the prompt to the AI.

## Command File Format

### Basic Structure

```markdown
---
description: Brief description of what the command does
---

Your markdown content here.
Use $ARGUMENTS to reference command arguments.
```

### Complete Example

```markdown
---
description: Generate comprehensive API documentation
tags:
  - documentation
  - api
handoffs:
  - label: Review Documentation
    agent: review_agent
    prompt: Review the generated documentation for accuracy
    send: false
---

## Documentation Generation

Please generate API documentation for: $ARGUMENTS

**Requirements**:
- Include all public endpoints
- Document request/response formats
- Provide example requests
- Note authentication requirements

**Format**: Markdown with code examples
```

## Frontmatter Fields

### Required Fields

- **description** (string): Brief description shown in `/help` output

### Optional Fields

- **tags** (array of strings): Categorize your commands
  ```yaml
  tags:
    - analysis
    - code-quality
  ```

- **handoffs** (array of objects): Define follow-up actions
  ```yaml
  handoffs:
    - label: Next Step
      agent: target_agent
      prompt: What to do next
      send: false
  ```

## Using Arguments

### The $ARGUMENTS Placeholder

Use `$ARGUMENTS` in your command body to insert user-provided arguments:

**Command file** (`.hoosh/commands/test.md`):
```markdown
---
description: Run tests for specified module
---

Please run tests for: $ARGUMENTS

Show:
- Test results
- Coverage metrics
- Failed test details
```

**Usage**:
```
You: /test src/auth
```

**Processed prompt**:
```markdown
Please run tests for: src/auth

Show:
- Test results
- Coverage metrics
- Failed test details
```

**No arguments**:
```
You: /test
```

**Processed prompt** (`$ARGUMENTS` replaced with empty string):
```markdown
Please run tests for:

Show:
- Test results
- Coverage metrics
- Failed test details
```

## Command Discovery

### List All Commands

Use `/help` to see all available commands, including your custom ones:

```
You: /help
```

Output includes:
- **Built-in Commands**: exit, help, clear, etc.
- **Custom Commands**: Your commands from `.hoosh/commands/`

### Command Naming

- **Source**: Filename without `.md` extension
- **Format**: Lowercase, no spaces
- **Example**: `analyze-code.md` → `/analyze-code`

**Tip**: Use descriptive, action-oriented names like `review-pr`, `generate-docs`, `analyze-perf`

## Best Practices

### 1. Clear Descriptions

Good:
```yaml
description: Analyze codebase for technical debt and complexity
```

Bad:
```yaml
description: Analysis  # Too vague
```

### 2. Structured Prompts

Use markdown formatting for clarity:

```markdown
---
description: Code review checklist
---

## Code Review: $ARGUMENTS

**Review Criteria**:
1. **Functionality**: Does the code work as intended?
2. **Readability**: Is the code easy to understand?
3. **Tests**: Are there adequate tests?
4. **Performance**: Any performance concerns?

Provide feedback for each criterion.
```

### 3. Argument Guidance

Help users understand expected arguments:

```markdown
---
description: Deploy readiness check (usage: /deploy-check [environment])
---

## Deployment Check for: $ARGUMENTS

Verify:
- [ ] All tests passing
- [ ] Environment variables configured
- [ ] Database migrations ready
- [ ] Rollback plan documented
```

### 4. Project-Specific Commands

Create commands tailored to your project:

```markdown
---
description: Review changes following our team's code standards
tags:
  - team-process
---

## Code Review

Review the changes in: $ARGUMENTS

**Team Standards**:
- Follow Airbnb JavaScript style guide
- All functions must have JSDoc comments
- Test coverage must be >80%
- No console.log statements in production code

Highlight any violations and suggest improvements.
```

## Common Patterns

### Pattern 1: Analysis Commands

```markdown
---
description: Security audit for specified files
---

## Security Audit: $ARGUMENTS

Scan for:
- SQL injection vulnerabilities
- XSS attack vectors
- Insecure dependencies
- Exposed secrets or API keys

Provide severity ratings and remediation steps.
```

### Pattern 2: Generation Commands

```markdown
---
description: Generate boilerplate code for new feature
---

## Generate Feature: $ARGUMENTS

Create:
1. Component/module structure
2. Unit tests
3. Integration tests
4. Documentation stub

Follow project conventions in src/ directory.
```

### Pattern 3: Workflow Commands

```markdown
---
description: Pre-deployment checklist
---

## Deployment Checklist

Environment: $ARGUMENTS

**Steps**:
1. Run full test suite
2. Check for pending migrations
3. Verify environment config
4. Review recent changes
5. Confirm rollback procedure

Confirm each step before proceeding.
```

## Troubleshooting

### Command Not Loading

**Symptom**: Custom command doesn't appear in `/help` or isn't recognized

**Checklist**:
1. ✅ File has `.md` extension?
2. ✅ File is in `.hoosh/commands/` directory?
3. ✅ File has YAML frontmatter with `description`?
4. ✅ Frontmatter YAML is valid?
5. ✅ Restarted Hoosh after creating file?

**Debug**: Check Hoosh startup output for error messages about command loading.

### Malformed YAML Error

**Symptom**: Error message about YAML parsing

**Common Issues**:
- Missing `---` delimiters
- Incorrect indentation (YAML is whitespace-sensitive)
- Unquoted special characters

**Example Fix**:

Bad:
```yaml
---
description: Test command: for testing
---
```

Good (quote the value):
```yaml
---
description: "Test command: for testing"
---
```

### Name Conflict with Built-in

**Symptom**: Warning message at startup

```
Warning: Custom command 'help' conflicts with built-in, skipping
```

**Solution**: Rename your custom command file. Built-in commands take precedence.

### Empty Description Error

**Symptom**: Command not loaded, validation error

**Fix**: Ensure description is not empty:

Bad:
```yaml
---
description:
---
```

Good:
```yaml
---
description: My custom command
---
```

## Advanced: Handoffs

Handoffs allow your custom command to suggest follow-up actions.

**Example**:
```markdown
---
description: Analyze and propose refactoring
handoffs:
  - label: Generate Refactoring Plan
    agent: planner_agent
    prompt: Create detailed refactoring plan based on analysis
    send: false
---

Analyze: $ARGUMENTS

Identify refactoring opportunities and explain benefits.
```

**Note**: Handoff functionality depends on agent integration (may be future enhancement).

## Examples

### Example 1: Simple Analysis

File: `.hoosh/commands/complexity.md`
```markdown
---
description: Calculate code complexity metrics
---

Calculate cyclomatic complexity for: $ARGUMENTS

Report:
- Complexity score
- Hotspots (high complexity areas)
- Recommendations for simplification
```

Usage:
```
/complexity src/utils/parser.js
```

### Example 2: Documentation Generator

File: `.hoosh/commands/docs.md`
```markdown
---
description: Generate README.md for module
tags:
  - documentation
---

## Generate README for: $ARGUMENTS

**Sections**:
1. **Overview**: What the module does
2. **Installation**: How to install
3. **Usage**: Code examples
4. **API**: Public methods/functions
5. **Contributing**: How to contribute

Use clear, concise language suitable for new users.
```

Usage:
```
/docs src/auth-module/
```

### Example 3: Code Review

File: `.hoosh/commands/review.md`
```markdown
---
description: Structured code review
tags:
  - quality
  - review
---

## Code Review: $ARGUMENTS

**Review Areas**:
1. **Logic**: Correctness and edge cases
2. **Style**: Readability and conventions
3. **Performance**: Efficiency concerns
4. **Security**: Vulnerabilities
5. **Tests**: Coverage and quality

Provide ratings (1-5) and specific feedback for each area.
```

Usage:
```
/review src/payment-processor.py
```

## Next Steps

1. **Create your first command**: Start with a simple analysis or documentation command
2. **Experiment**: Try different prompt formats to see what works best
3. **Share**: Custom commands are project-local, perfect for team workflows
4. **Iterate**: Refine your commands based on results

## Tips

- **Start simple**: Begin with basic commands, add complexity as needed
- **Use templates**: Create a template command to copy for new commands
- **Version control**: Commit `.hoosh/commands/` to share with your team
- **Document**: Add comments in command body to explain complex prompts
- **Test**: Try commands with different arguments to ensure robustness

## Reference

- **Commands directory**: `.hoosh/commands/` (created automatically)
- **File format**: Markdown (`.md`) with YAML frontmatter
- **Reload**: Restart Hoosh to load new/updated commands (MVP)
- **Built-in precedence**: Built-in commands can't be overridden

Happy commanding! 🚀
