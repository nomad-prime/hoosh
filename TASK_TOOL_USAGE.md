# TaskTool Implementation

The TaskTool allows spawning autonomous sub-agents to handle complex tasks independently.

## Components

### Core Module: `src/task_management/`

- **AgentType**: Enum defining three agent types:
  - `Plan`: Analysis and planning (max 50 steps)
  - `Explore`: Fast codebase exploration (max 30 steps)
  - `GeneralPurpose`: Full-featured agent (max 100 steps)

- **TaskDefinition**: Defines a task with agent type, prompt, description, optional timeout and model

- **TaskResult**: Contains success status, output, events, and optional token usage

- **TaskManager**: Handles sub-agent lifecycle, spawning agents with isolated event channels

### Tool: `src/tools/task_tool.rs`

Implements the `Tool` trait to be called by LLMs. Takes parameters:
- `subagent_type`: "plan", "explore", or "general-purpose"
- `prompt`: The task instructions
- `description`: Short 3-5 word description
- `model` (optional): Override the model

### Provider: `src/tools/task_tool_provider.rs`

A `ToolProvider` that creates TaskTool instances. Must be registered after backend and permission_manager are created.

## Registration

To enable TaskTool in your agent:

```rust
use hoosh::{TaskToolProvider, ToolRegistry};
use std::sync::Arc;

// 1. Create base registry with standard tools
let mut tool_registry = ToolRegistry::new()
    .with_provider(Arc::new(BuiltinToolProvider::new(working_dir)));

// 2. Create TaskToolProvider with registry reference
// Sub-agents will use this registry (including TaskTool once registered)
let task_provider = Arc::new(TaskToolProvider::new(
    backend.clone(),
    Arc::new(tool_registry.clone()),
    permission_manager.clone(),
));

// 3. Register TaskTool in the registry
tool_registry.add_provider(task_provider);

// 4. Wrap final registry for use by agent
let tool_registry = Arc::new(tool_registry);
```

**Note**: Sub-agents receive a filtered tool registry without the TaskTool to prevent recursive sub-agent spawning. Only one layer of task delegation is supported.

## How It Works

1. LLM calls the `task` tool with agent type and prompt
2. TaskTool creates a TaskManager with shared resources
3. TaskManager creates a filtered registry (removing "task" tool) for the sub-agent
4. Sub-agent runs autonomously with isolated event channel and filtered tools
5. Results are collected and returned to parent agent

Sub-agents inherit the same permission manager and have access to all tools except TaskTool.

## Security

TaskTool is marked as `destructive` in permissions since sub-agents can execute arbitrary operations through their tool access.
