```markdown
# Dynamic Tool Generation

## Concept

Enable LLM to create reusable bash-based tools during a session instead of repeating bash commands. Tools persist in
session and can be invoked by name.

## Implementation

### 1. Tool Registry

```rust
// src/tools/mod.rs
pub struct ToolRegistry {
    tools: HashMap<String, DynamicTool>,
}

pub struct DynamicTool {
    name: String,
    description: String,
    script: String,
    parameters: Vec<ToolParameter>,
}

pub struct ToolParameter {
    name: String,
    description: String,
    required: bool,
}

impl ToolRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, tool: DynamicTool) -> Result<()>;
    pub fn execute(&self, name: &str, args: &[String]) -> Result<String>;
    pub fn list(&self) -> Vec<&DynamicTool>;
    pub fn to_llm_schema(&self) -> String;
}
```

### 2. New Tool Functions

Add alongside existing bash_tool/file tools:

```rust
pub fn create_tool() -> Result<String>;

pub fn use_tool() -> Result<String>;

pub fn list_tools() -> Result<String>;
```

### 3. Tool Execution

In tool parameter substitution:

- Replace $1, $2, etc with provided args
- Execute via existing bash_tool
- Return output

### 4. Persistence

```rust
impl ToolRegistry {
    pub fn save_to_file(&self, path: &Path) -> Result<()>;
    pub fn load_from_file(path: &Path) -> Result<Self>;
}
```

Store in `~/.hoosh/tools/`

## Testing

- Create tool that wraps jq command
- Use tool multiple times in session
- List available tools
- Verify tool persists across messages in same session
- Test parameter substitution with multiple args

## Integration Points

- Session initialization: load tools
- Message loop: update system prompt with tool list
- Tool execution: route through registry before bash

```
