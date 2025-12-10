# Reuse of Existing Hoosh Infrastructure

## Overview

The cascade system integrates with existing Hoosh infrastructure rather than redefining common patterns. This document clarifies what is reused vs. what is new.

---

## Already Exists: Tool Trait & Ecosystem

**Location**: `src/tools/mod.rs`

**What it is**: Generic trait for all tools in Hoosh (bash, file ops, todo, etc.)

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    async fn execute(&self, args: &Value, context: &ToolExecutionContext) -> ToolResult<String>;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameter_schema(&self) -> Value;
    fn describe_permission(&self, target: Option<&str>) -> ToolPermissionDescriptor;
    // ... more methods
}
```

**Phase 1 Usage**: 
- The `escalate` tool will implement this trait
- Will be registered in `ToolRegistry` just like bash, grep, read_file, etc.
- Will receive `ToolExecutionContext` with `tool_call_id` for tracking

**Why reuse**: Consistent with Hoosh's tool architecture. No need to define a cascade-specific tool interface.

---

## Already Exists: Approval Response Type

**Location**: `src/agent/core.rs`

```rust
#[derive(Debug, Clone)]
pub struct ApprovalResponse {
    pub tool_call_id: String,
    pub approved: bool,
    pub rejection_reason: Option<String>,
}
```

**Current Usage**: Tool execution approval (user approves dangerous tool calls like bash, write_file)

**Phase 1 Usage**: 
- When agent calls `escalate` tool, system needs operator approval
- `escalate` tool call will receive `ApprovalResponse` via the existing approval channel
- No new response type needed

**Why reuse**: Escalation approval is just another form of tool approval. The same mechanism (request → user reviews → approve/reject) applies.

---

## Already Exists: TUI Approval Handler

**Location**: `src/tui/handlers/approval_handler.rs`

**What it is**: Keyboard event handler that processes user approval decisions in the TUI

```rust
pub struct ApprovalHandler {
    pub approval_response_tx: mpsc::UnboundedSender<crate::agent::ApprovalResponse>,
}

#[async_trait]
impl InputHandler for ApprovalHandler {
    async fn handle_event(&mut self, event: &Event, app: &mut AppState, ...) -> KeyHandlerResult {
        // Handles: y/a = approve, n/r/Esc = reject, Up/Down = select option
        // Sends ApprovalResponse via approval_response_tx
    }
}
```

**Current Usage**: User approves/rejects tool executions (bash, file ops, etc.)

**Phase 1 Usage**: 
- Same handler processes escalation approval requests
- TUI will display: "Escalate from Medium to Heavy? Reason: <escalation_reason>"
- User presses y/n to approve/reject
- Handler sends `ApprovalResponse` with tool_call_id = escalate tool call ID

**Why reuse**: The approval UI and key handling are generic. No cascade-specific approval UI needed.

---

## NEW: Cascade-Specific Components

### 1. ComplexityAnalyzer Trait
- **What**: Analyzes task descriptions to determine complexity (Light/Medium/Heavy)
- **Why new**: Domain-specific to cascade system; existing tools don't do this
- **Implementation**: `src/cascades/complexity.rs`

### 2. CascadeRouter Trait
- **What**: Maps complexity to model tier based on configuration
- **Why new**: Specific cascade routing logic; not part of existing tool/permission system
- **Implementation**: `src/cascades/router.rs`

### 3. CascadeEventLogger Trait
- **What**: Emits structured cascade lifecycle events for observability
- **Why new**: Cascade-specific observability; different from general logging
- **Implementation**: `src/cascades/events.rs`

### 4. EscalateTool Implementation
- **What**: Concrete `Tool` that allows agent to request escalation
- **Why new**: Cascade-specific tool; extends the existing `Tool` trait
- **Implementation**: `src/cascades/escalate_tool.rs`

---

## Integration Points

### 1. Tool Registry
```rust
// When cascade is enabled:
let escalate_tool = EscalateTool::new(cascade_system.clone());
tool_registry.register(Box::new(escalate_tool))?;
```

### 2. Agent Execution Loop
```rust
// Before executing with a specific model:
if cascades_enabled {
    let complexity = analyzer.analyze(&task)?;
    let tier = router.route(&complexity)?;
    selected_tier = Some(tier);
}
// Use selected_tier to choose which backend/model to use
```

### 3. Escalation Approval Flow
```
Agent calls escalate tool
    ↓
Tool executor sees it's a tool call
    ↓
Approval handler prompts user: "Escalate? [y/n]"
    ↓
User presses y → ApprovalResponse { approved: true }
    ↓
Tool executes: switches model tier + retries task
```

---

## Configuration

### Cascades Config (new section in .hoosh.toml)
```toml
[cascades]
enabled = true                     # OFF by default for safety
routing_policy = "multi-signal"
default_tier = "Medium"
escalation_needs_approval = true   # Must use existing approval mechanism

[[cascades.model_tiers]]
tier = "Light"
models = ["claude-3-5-haiku-20241022"]
max_cost_per_request_cents = 2

[[cascades.model_tiers]]
tier = "Medium"
models = ["claude-3-5-sonnet-20241022"]
max_cost_per_request_cents = 5

[[cascades.model_tiers]]
tier = "Heavy"
models = ["claude-3-opus-20250219"]
max_cost_per_request_cents = 15
```

### Routing Weights Config (cascade-specific)
```toml
[cascades.routing_weights]
structural_depth = 0.35
action_density = 0.35
code_signals = 0.30
confidence_threshold = 0.7  # Below this, default to Medium
```

---

## Architecture Diagram

```
Existing Infrastructure:
┌─────────────────────────────────────────────────┐
│ Tool Registry (bash, grep, read_file, etc.)    │
│ + EscalateTool (NEW for cascades)              │
└────────────────┬────────────────────────────────┘
                 │
                 ↓
         ┌───────────────┐
         │ Tool Executor │◄─── Approval Handler (EXISTING)
         └───────────────┘     - Processes y/n for escalate
                 │
                 ↓
         ┌───────────────────┐
         │ Agent Loop        │◄─── CascadeRouter (NEW)
         │ (Execute task)    │     - Tier routing based on complexity
         └───────────────────┘
                 │
                 ↓
      ┌──────────────────────────────────────┐
      │ NEW CASCADE COMPONENTS:              │
      ├──────────────────────────────────────┤
      │ - ComplexityAnalyzer (analyze tasks) │
      │ - CascadeRouter (select tier)        │
      │ - CascadeContext (track state)       │
      │ - CascadeEventLogger (emit events)   │
      │ - EscalateTool (allow agent escape)  │
      └──────────────────────────────────────┘
```

---

## Why This Design?

1. **Minimal Surface Area**: Reuse existing tool and approval mechanisms
2. **Consistency**: Escalation feels like other tool approvals to the user
3. **Low Risk**: No changes to existing Tool, ApprovalResponse, or ApprovalHandler
4. **Clear Boundaries**: Cascade-specific logic isolated in `src/cascades/` module
5. **Testability**: Each component has clear inputs/outputs; easy to mock

---

## Files NOT Modified in Phase 1

- `src/tools/mod.rs` — Tool trait (unchanged)
- `src/tools/task_tool.rs` — Existing tools (unchanged)
- `src/agent/core.rs` — ApprovalResponse (unchanged)
- `src/tui/handlers/approval_handler.rs` — Approval handler (unchanged)
- `src/tui/components/approval_dialog.rs` — Approval dialog UI (unchanged)

All cascade features are additive and contained in `src/cascades/` module.
