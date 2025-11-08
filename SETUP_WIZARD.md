# Add Interactive Project Setup Wizard

## Overview

Implement an interactive CLI wizard that guides users through initial hoosh configuration, including backend selection,
API key setup, and default model configuration.

## Requirements

### Command Structure

- Add new subcommand: `hoosh init` or `hoosh setup`
- Should be idempotent - safe to run multiple times
- Detect if config already exists and offer to reconfigure or exit

### Wizard Flow

1. **Welcome Screen**
    - Display brief intro to hoosh
    - Show what will be configured
    - Option to skip wizard and use defaults

2. **Backend Selection**
    - Present list of available backends:
        - Anthropic (Claude)
        - OpenAI (GPT)
        - Together AI
        - Other supported backends
    - Allow arrow key navigation or number selection
    - Show brief description of each backend

3. **API Key Configuration**
    - Prompt for API key based on selected backend
    - Support pasting key directly or reading from file
    - Validate key format (basic check, not API call)
    - Option to set as environment variable or store in config
    - Warn about storing keys in config file vs env vars

4. **Model Selection**
    - List available models for chosen backend
    - Show recommended model as default
    - Allow custom model name input
    - Display brief model descriptions (context window, capabilities)

5. **Additional Settings** (optional)
    - Temperature/sampling parameters
    - Max tokens
    - System prompt customization
    - Leave at defaults with option to configure later

6. **Confirmation & Summary**
    - Display all configured settings
    - Confirm before writing config file
    - Show config file location
    - Provide next steps (e.g., "Run `hoosh chat` to start")

### Technical Implementation

**New Module**: `src/cli/wizard.rs`

```rust
pub struct SetupWizard {
    config: AppConfig,
}

impl SetupWizard {
    pub async fn run() -> Result<AppConfig>;
    fn select_backend() -> Result<BackendType>;
    fn configure_api_key(backend: BackendType) -> Result<String>;
    fn select_model(backend: BackendType) -> Result<String>;
    fn confirm_settings(config: &AppConfig) -> Result<bool>;
    fn save_config(config: &AppConfig) -> Result<()>;
}
```

**Dependencies to Add**:

- `dialoguer` - for interactive prompts (select, input, confirm)
- `console` - for terminal formatting and colors

**Error Handling**:

- Handle Ctrl+C gracefully (save partial config or exit cleanly)
- Provide clear error messages for invalid inputs
- Allow user to go back to previous step
- Don't crash on invalid API key format - just warn

### User Experience Considerations

- Use colors sparingly for better readability
- Show progress indicators for multi-step process
- Provide sensible defaults that work out of the box
- Include help text for each prompt
- Support both interactive mode and flags for automation:
  ```bash
  hoosh init --backend anthropic --api-key $KEY --model claude-sonnet-4
  ```

### Config File Output

Generate valid TOML config at `~/.config/hoosh/config.toml`:

```toml
default_backend = "anthropic"

[backends.anthropic]
api_key = "${ANTHROPIC_API_KEY}"  # or actual key if user chose to store
model = "claude-sonnet-4"
temperature = 0.7

# Include commented examples for other backends
# [backends.openai]
# api_key = "${OPENAI_API_KEY}"
# model = "gpt-4"
```

### Technical Details

we already have an initial permission layout in initial_permission_layout.rs. and init. You need to do something similar
that. Strive NOT to touch existing code in the implementation. If a module does not work for your usecase write a new
one for it, we will remove duplication later.

create these files

an InitAppState like AppState in app.rs
a layout like app_layout.rs and initial_permission_layout.rs
and a loop like app_loop.rs and init_permission_loop.rs

the component should go in components folder like tui/components/initial_permission_dialog
and the input handler to tui/handlers/initial_permission_handler.rs

### Testing Requirements

- Unit tests for each wizard step
- Test handling of existing config files

## Acceptance Criteria

- [ ] if config does not exist, running `hoosh` starts
- [ ] `hoosh init` command launches interactive wizard
- [ ] All major backends can be configured
- [ ] API keys can be stored as env var references or direct values
- [ ] Generated config file is valid TOML and loads correctly
- [ ] Wizard handles errors and invalid input gracefully
- [ ] Non-interactive mode works with command-line flags
- [ ] Existing configs are detected and user is prompted
- [ ] Help text is clear and informative
- [ ] Tests cover main wizard paths

## Out of Scope

- API key validation via actual API calls
- Multi-backend configuration in single wizard run (can run multiple times)
- Migration from old config formats
- Cloud-based config sync
