We'll implement a Task tool that spawns autonomous sub-agents similar to Claude Code's architecture. The tool will fit
naturally into hoosh's existing patterns.

Architecture Decision: Option C - Task Manager System

Create a comprehensive task_management module that:

- Manages sub-agent lifecycle (spawn, monitor, cleanup)
- Handles communication between parent and child agents
- Supports multiple agent types (Plan, Explore, general-purpose)
- Integrates as both a Tool (callable by LLM) and potentially a Command (user-callable)

Key Components

1. Core Data Structures (src/task_management/mod.rs)

- TaskDefinition { agent_type, prompt, description, timeout, max_steps }
- TaskResult { success, output, events, token_usage }
- SubAgentConfig { specialized configs for Plan/Explore/general }
- TaskManager { tracks active tasks, handles concurrency }

2. TaskTool Implementation (src/tools/task_tool.rs)

- Implements the Tool trait
- JSON schema with: subagent_type, prompt, description, model (optional)
- Async execute() method that spawns sub-agent
- Returns consolidated report to parent agent

3. Sub-Agent Spawning Pattern

- Clone shared resources (Arc, Arc, PermissionManager)
- Create isolated ToolExecutor with sub-agent event channel
- Create new Agent instance with specialized configuration
- Create fresh Conversation with system message containing task prompt
- Run with tokio::time::timeout for safety
- Collect and consolidate results

4. Event Routing

- Sub-agents emit to their own event channel (isolated from parent)
- TaskManager aggregates sub-agent events
- Parent can optionally see high-level progress
- Final report returned as tool result

5. Agent Type Specializations

- Plan: Optimized for exploring codebase, medium thoroughness, focused on understanding
- Explore: Fast searches with configurable thoroughness (quick/medium/very thorough)
- general-purpose: Full tool access for complex multi-step tasks

6. Integration Points

- Register in BuiltinToolProvider::provide_tools() (src/tools/provider.rs)
- Uses existing permission system (sub-agents inherit or request permissions)
- Uses existing backend abstraction (works with any LLM provider)
- Leverages existing tool registry (sub-agents have access to same tools)

Implementation Flow

1. Design data structures and module layout
2. Create basic TaskTool skeleton implementing Tool trait
3. Implement sub-agent spawning with simple example
4. Add TaskManager for tracking multiple concurrent tasks
5. Add event routing and communication
6. Create specialized configurations for different agent types
7. Add safety: timeouts, max steps, resource limits
8. Register with tool system
9. Test with mock backend and real scenarios
10. Document and add examples

Key Files to Modify/Create

- src/task_management/mod.rs - New module (core types)
- src/task_management/task_manager.rs - New (lifecycle management)
- src/tools/task_tool.rs - New (Tool implementation)
- src/tools/provider.rs - Modify (register TaskTool)
- src/lib.rs - Modify (export new module)

Would you like me to start implementing this plan? I can begin with the core data structures and work through each
component systematically.

Todos
☐ Design Task tool architecture and data structures
☐ Create task_management module with core types
☐ Implement TaskTool as a Tool trait implementation
☐ Implement sub-agent spawning and lifecycle management
☐ Add TaskManager for tracking active sub-agents
☐ Register TaskTool with BuiltinToolProvider
☐ Add sub-agent event routing and communication
☐ Create specialized agent configurations (Plan, Explore, etc.)
☐ Add timeout and resource limits for sub-agents
☐ Test TaskTool with mock backend
