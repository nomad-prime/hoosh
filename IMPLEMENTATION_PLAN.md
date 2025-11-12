# Implementation Plan: Subagent Action Summaries in TUI

## Executive Summary

This plan adds visibility into subagent internal actions by surfacing subagent step progress through new `AgentEvent` variants. The key insight is that `TaskManager` currently collects `TaskEvent` objects representing subagent steps but doesn't communicate these events to the main agent event system. This plan bridges that gap by allowing `TaskManager` to emit events that update the active tool call's running summary in real-time.

## Problem Analysis

### Current State
1. **TaskTool** invokes `TaskManager::execute_task()` 
2. **TaskManager** spawns a subagent which generates internal `TaskEvent` objects (via event channel)
3. **TaskManager** collects these events but they're only returned in `TaskResult.events` at completion
4. **Active tool calls** in the TUI only show final result summary from `TaskTool`, not intermediate progress
5. **Result**: Users cannot see what the subagent is doing while it's running

### Root Cause
- The event channel from `TaskManager` only collects events locally
- These events aren't emitted back through the main `AgentEvent` system
- `ActiveToolCall::result_summary` only gets populated after task completion
- No mechanism exists to update running summary during execution

## Architecture Overview

### Current Component Relationships
```
TaskTool (Tool)
    ↓ calls
TaskManager
    ├─ spawns Agent (subagent)
    │   ├─ sends AgentEvents → event_tx (internal channel)
    │   └─ executes ToolExecutor
    │
    └─ collects TaskEvent (debug string only)
        └─ returned in TaskResult after completion

AppState
    └─ handles_agent_event()
        └─ updates active_tool_calls
```

### After Implementation
```
TaskTool (Tool)
    ↓ calls
TaskManager (receives event_tx from caller)
    ├─ spawns Agent (subagent)
    │   └─ sends AgentEvents → internal_event_tx
    │
    ├─ bridges events: internal_event_tx → external_event_tx
    │   └─ transforms to SubagentStepProgress
    │
    └─ emits SubagentStepProgress → caller's event_tx

AppState
    └─ handle_agent_event()
        └─ processes SubagentStepProgress
            └─ updates active_tool_calls.result_summary (running)
```

## Detailed Implementation Steps

### Phase 1: Event System Enhancement

#### Step 1.1: Add New AgentEvent Variants (src/agent/agent_events.rs)

Create two new variants in the `AgentEvent` enum to represent subagent progress:

```rust
#[derive(Debug, Clone)]
pub enum AgentEvent {
    // ... existing variants ...
    
    /// Emitted when a subagent step completes (tool execution, thought, etc.)
    /// Allows parent agent to track progress of subagent task execution
    SubagentStepProgress {
        tool_call_id: String,        // The task tool call being executed
        step_number: usize,          // Sequential step counter (1-indexed)
        action_type: String,         // "tool_execution", "thinking", etc.
        description: String,         // Brief description of what happened
        timestamp: std::time::SystemTime,
    },
    
    /// Emitted when a subagent reaches completion
    /// Allows clearing of step progress summary
    SubagentTaskComplete {
        tool_call_id: String,
        total_steps: usize,
    },
}
```

**Rationale:**
- `SubagentStepProgress`: Tracks each meaningful step within the subagent execution
  - `step_number`: Enables progress indication (e.g., "Step 3/10")
  - `action_type`: Categorizes events for display (tool calls, thoughts, etc.)
  - `description`: Human-readable summary of the action
  
- `SubagentTaskComplete`: Signals clean transition to final result display

#### Step 1.2: Update ActiveToolCall Structure (src/tui/app_state.rs)

Enhance `ActiveToolCall` to track running subagent progress:

```rust
#[derive(Clone, Debug)]
pub struct ActiveToolCall {
    pub tool_call_id: String,
    pub display_name: String,
    pub status: ToolCallStatus,
    pub preview: Option<String>,
    pub result_summary: Option<String>,
    
    // NEW: For subagent tasks
    pub subagent_steps: Vec<SubagentStepSummary>,  // Running list of steps
    pub current_step: usize,                        // For progress display
    pub is_subagent_task: bool,                     // Flag to distinguish task tool calls
}

#[derive(Clone, Debug)]
pub struct SubagentStepSummary {
    pub step_number: usize,
    pub action_type: String,  // "tool", "thinking", "file_operation", etc.
    pub description: String,
}
```

**Rationale:**
- Separate tracking of subagent steps from final result
- `current_step` enables "Step X/N" progress display
- `is_subagent_task` allows conditional rendering logic
- `subagent_steps` provides historical record for detailed display

#### Step 1.3: Implement Helper Methods on ActiveToolCall (src/tui/app_state.rs)

```rust
impl ActiveToolCall {
    pub fn add_subagent_step(&mut self, step: SubagentStepSummary) {
        self.subagent_steps.push(step);
        self.current_step = self.subagent_steps.len();
    }
    
    pub fn get_running_summary(&self) -> String {
        if self.subagent_steps.is_empty() {
            return String::new();
        }
        
        // Show last 3 steps or all if fewer
        let start = self.subagent_steps.len().saturating_sub(3);
        let recent = &self.subagent_steps[start..];
        
        let step_descriptions = recent
            .iter()
            .map(|s| s.description.clone())
            .collect::<Vec<_>>()
            .join(" → ");
        
        format!("[{}] {}", self.current_step, step_descriptions)
    }
    
    pub fn get_progress_indicator(&self) -> String {
        if self.subagent_steps.is_empty() {
            return "0%".to_string();
        }
        // Rough estimate: steps don't have max, but can show recent activity
        format!("⊙ Step {}", self.current_step)
    }
}
```

**Rationale:**
- `add_subagent_step`: Central point for step accumulation
- `get_running_summary`: Shows recent steps (last 3) to avoid clutter
- `get_progress_indicator`: Visual indicator of ongoing work

### Phase 2: TaskManager Enhancement

#### Step 2.1: Modify TaskManager to Accept Event Sender (src/task_management/task_manager.rs)

Change the `TaskManager` structure to accept and use an external event sender:

```rust
pub struct TaskManager {
    backend: Arc<dyn LlmBackend>,
    tool_registry: Arc<ToolRegistry>,
    permission_manager: Arc<PermissionManager>,
    event_tx: Option<mpsc::UnboundedSender<AgentEvent>>,  // NEW
}

impl TaskManager {
    pub fn new(
        backend: Arc<dyn LlmBackend>,
        tool_registry: Arc<ToolRegistry>,
        permission_manager: Arc<PermissionManager>,
    ) -> Self {
        Self {
            backend,
            tool_registry,
            permission_manager,
            event_tx: None,
        }
    }
    
    pub fn with_event_sender(mut self, tx: mpsc::UnboundedSender<AgentEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }
}
```

**Rationale:**
- Uses builder pattern already established in codebase
- Optional event sender maintains backward compatibility
- Allows TaskManager to emit events to parent agent

#### Step 2.2: Modify TaskManager Structure and Signature (src/task_management/task_manager.rs)

Update TaskManager to accept and track the tool_call_id:

```rust
pub struct TaskManager {
    backend: Arc<dyn LlmBackend>,
    tool_registry: Arc<ToolRegistry>,
    permission_manager: Arc<PermissionManager>,
    event_tx: Option<mpsc::UnboundedSender<AgentEvent>>,
    tool_call_id: Option<String>,  // NEW: Track which tool call this task belongs to
}

impl TaskManager {
    pub fn new(
        backend: Arc<dyn LlmBackend>,
        tool_registry: Arc<ToolRegistry>,
        permission_manager: Arc<PermissionManager>,
    ) -> Self {
        Self {
            backend,
            tool_registry,
            permission_manager,
            event_tx: None,
            tool_call_id: None,
        }
    }
    
    pub fn with_event_sender(mut self, tx: mpsc::UnboundedSender<AgentEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }
    
    pub fn with_tool_call_id(mut self, id: String) -> Self {
        self.tool_call_id = Some(id);
        self
    }
}
```

#### Step 2.3: Bridge Subagent Events in execute_task (src/task_management/task_manager.rs)

Modify the event collection loop to emit transformed events:

```rust
pub async fn execute_task(&self, task_def: TaskDefinition) -> Result<TaskResult> {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    
    let sub_agent_registry = Arc::new(self.tool_registry.without("task"));
    let tool_executor = Arc::new(
        ToolExecutor::new(
            (*sub_agent_registry).clone(),
            (*self.permission_manager).clone(),
        )
        .with_event_sender(event_tx.clone()),
    );
    
    let agent = Agent::new(self.backend.clone(), sub_agent_registry, tool_executor)
        .with_max_steps(task_def.agent_type.max_steps())
        .with_event_sender(event_tx);

    let mut conversation = Conversation::new();
    let system_message = task_def.agent_type.system_message(&task_def.prompt);
    conversation.add_user_message(system_message);

    // NEW: Prepare event bridging
    let parent_event_tx = self.event_tx.clone();
    let tool_call_id = self.tool_call_id.clone();
    
    let event_collector = tokio::spawn(async move {
        let mut collected_events = Vec::new();
        let mut step_count = 0;
        
        while let Some(event) = event_rx.recv().await {
            let event_string = format!("{:?}", event);
            
            // Collect event for result
            collected_events.push(TaskEvent {
                event_type: event_string
                    .split('(')
                    .next()
                    .unwrap_or("Unknown")
                    .to_string(),
                message: event_string.clone(),
                timestamp: std::time::SystemTime::now(),
            });
            
            // NEW: Bridge filtered events to parent
            if let (Some(ref tx), Some(ref tcid)) = (&parent_event_tx, &tool_call_id) {
                if should_emit_to_parent(&event) {
                    step_count += 1;
                    if let Ok(progress_event) = transform_to_subagent_event(
                        &event,
                        tcid,
                        step_count,
                    ) {
                        let _ = tx.send(progress_event);
                    }
                }
            }
        }
        collected_events
    });

    // ... rest of execution unchanged ...
    
    // After completion, emit SubagentTaskComplete
    if let (Some(ref tx), Some(ref tcid)) = (&self.event_tx, &self.tool_call_id) {
        let _ = tx.send(AgentEvent::SubagentTaskComplete {
            tool_call_id: tcid.clone(),
            total_steps: events.len(),
        });
    }
    
    Ok(TaskResult::success(final_response).with_events(events))
}

fn should_emit_to_parent(event: &AgentEvent) -> bool {
    matches!(
        event,
        AgentEvent::AssistantThought(_)
            | AgentEvent::ToolExecutionStarted { .. }
            | AgentEvent::ToolExecutionCompleted { .. }
            | AgentEvent::ToolResult { .. }
    )
}

fn transform_to_subagent_event(
    event: &AgentEvent,
    tool_call_id: &str,
    step_number: usize,
) -> Result<AgentEvent, String> {
    let (action_type, description) = match event {
        AgentEvent::AssistantThought(content) => {
            let preview = if content.len() > 50 {
                format!("{}...", &content[..50])
            } else {
                content.clone()
            };
            ("thinking", preview)
        }
        AgentEvent::ToolExecutionStarted { tool_name, .. } => {
            ("tool_starting", format!("Executing {}", tool_name))
        }
        AgentEvent::ToolExecutionCompleted { tool_name, .. } => {
            ("tool_completed", format!("Completed {}", tool_name))
        }
        AgentEvent::ToolResult { summary, .. } => {
            let preview = if summary.len() > 50 {
                format!("{}...", &summary[..50])
            } else {
                summary.clone()
            };
            ("tool_result", preview)
        }
        _ => return Err("Event not bridged".to_string()),
    };

    Ok(AgentEvent::SubagentStepProgress {
        tool_call_id: tool_call_id.to_string(),
        step_number,
        action_type: action_type.to_string(),
        description,
        timestamp: std::time::SystemTime::now(),
    })
}
```

**Rationale:**
- Tool call ID passed explicitly from TaskTool to TaskManager
- Selective event bridging (only meaningful progress events)
- Step counting provides sequence information
- Text truncation (50 chars) prevents excessive logging while maintaining readability
- `SubagentTaskComplete` signals transition from running to completed state

### Phase 3: TaskTool Integration

#### Step 3.1: Extend Tool Trait for Context Awareness (src/tools/mod.rs or create new extension)

Create a new optional trait method that allows tools to receive execution context:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    // ... existing methods ...
    
    /// Optional: Execute with tool execution context
    /// Falls back to execute() if not implemented
    async fn execute_with_context(
        &self,
        args: &Value,
        context: Option<ToolExecutionContext>,
    ) -> ToolResult<String> {
        // Default implementation ignores context
        self.execute(args).await
    }
}

#[derive(Clone)]
pub struct ToolExecutionContext {
    pub tool_call_id: String,
    pub event_tx: Option<mpsc::UnboundedSender<AgentEvent>>,
}
```

**Rationale:**
- Extends Tool interface without breaking existing implementations
- Default implementation provides backward compatibility
- Only TaskTool needs context; other tools don't care

#### Step 3.2: Modify ToolExecutor to Pass Context (src/tool_executor.rs)

Update the tool execution call site:

```rust
// In execute_tool_call(), around line 115-120, replace:
// let result = tool.execute(&args).await;
// with:

let context = ToolExecutionContext {
    tool_call_id: tool_call_id.clone(),
    event_tx: self.event_sender.clone(),
};

let result = tool.execute_with_context(&args, Some(context)).await;
```

**Why this approach:**
- Zero impact on existing tools
- Tools opt-in to context via method override
- ToolExecutor doesn't need to know about TaskTool specifics

#### Step 3.3: Implement execute_with_context in TaskTool (src/tools/task_tool.rs)

```rust
#[async_trait]
impl Tool for TaskTool {
    async fn execute(&self, args: &Value) -> ToolResult<String> {
        // Fallback implementation without context
        self.execute_impl(args, None).await
    }
    
    async fn execute_with_context(
        &self,
        args: &Value,
        context: Option<ToolExecutionContext>,
    ) -> ToolResult<String> {
        self.execute_impl(args, context).await
    }
}

impl TaskTool {
    async fn execute_impl(
        &self,
        args: &Value,
        context: Option<ToolExecutionContext>,
    ) -> ToolResult<String> {
        let args: TaskArgs = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::InvalidArguments {
                tool: "task".to_string(),
                message: e.to_string(),
            })?;

        let agent_type = AgentType::from_name(&args.subagent_type)
            .map_err(|e| ToolError::InvalidArguments {
                tool: "task".to_string(),
                message: e.to_string(),
            })?;

        let mut task_def = TaskDefinition::new(agent_type, args.prompt, args.description);
        if let Some(model) = args.model {
            task_def = task_def.with_model(model);
        }

        let mut task_manager = TaskManager::new(
            self.backend.clone(),
            self.tool_registry.clone(),
            self.permission_manager.clone(),
        );
        
        // NEW: Pass event sender and tool_call_id to TaskManager
        if let Some(ctx) = context {
            if let Some(tx) = ctx.event_tx {
                task_manager = task_manager
                    .with_event_sender(tx)
                    .with_tool_call_id(ctx.tool_call_id);
            }
        }

        let result = task_manager
            .execute_task(task_def)
            .await
            .map_err(|e| ToolError::execution_failed(e.to_string()))?;

        if result.success {
            Ok(result.output)
        } else {
            Err(ToolError::execution_failed(result.output))
        }
    }
}
```

**Rationale:**
- Maintains existing `execute()` as fallback
- `execute_with_context()` provides full capability
- TaskManager receives both event_tx and tool_call_id

### Phase 4: AppState Event Handling

#### Step 4.1: Add New Event Handlers to AppState (src/tui/app_state.rs)

Update `handle_agent_event` to process subagent events:

```rust
impl AppState {
    pub fn handle_agent_event(&mut self, event: AgentEvent) {
        match event {
            // ... existing event handlers ...
            
            AgentEvent::SubagentStepProgress {
                tool_call_id,
                step_number,
                action_type,
                description,
                ..
            } => {
                // Find the active tool call and add the step
                if let Some(tool_call) = self.get_active_tool_call_mut(&tool_call_id) {
                    tool_call.is_subagent_task = true;
                    
                    let step = SubagentStepSummary {
                        step_number,
                        action_type,
                        description,
                    };
                    tool_call.add_subagent_step(step);
                }
            }
            
            AgentEvent::SubagentTaskComplete {
                tool_call_id,
                total_steps,
            } => {
                // Mark subagent as complete, preserve step history
                if let Some(tool_call) = self.get_active_tool_call_mut(&tool_call_id) {
                    self.add_debug_message(format!(
                        "Subagent completed: {} steps",
                        total_steps
                    ));
                }
            }
            
            // ... rest of existing handlers ...
        }
    }
}
```

**Rationale:**
- `SubagentStepProgress` updates running summary in real-time
- `SubagentTaskComplete` marks end of step collection
- Flag `is_subagent_task` enables conditional rendering
- Steps preserved for detailed display if needed

#### Step 4.2: Update ToolCallStatus Display Logic

Enhance the rendering logic to show different visual indicators for subagent tasks:

```rust
// In active_tool_calls.rs render method
let status_indicator = match &tool_call.status {
    ToolCallStatus::Starting => Span::styled("○", Style::default().fg(Color::Gray)),
    ToolCallStatus::AwaitingApproval => {
        Span::styled("◎", Style::default().fg(Color::Yellow))
    }
    ToolCallStatus::Executing => {
        if tool_call.is_subagent_task && !tool_call.subagent_steps.is_empty() {
            // Use different indicator for active subagent
            Span::styled("⊙", Style::default().fg(Color::Blue))
        } else {
            Span::styled("●", Style::default().fg(Color::Cyan))
        }
    }
    ToolCallStatus::Completed => Span::styled("✓", Style::default().fg(Color::Green)),
    ToolCallStatus::Error(_) => Span::styled("✗", Style::default().fg(Color::Red)),
};
```

### Phase 5: Component Rendering Enhancement

#### Step 5.1: Update active_tool_calls.rs to Display Subagent Progress (src/tui/components/active_tool_calls.rs)

Enhance rendering to show subagent step progress:

```rust
impl Component for ActiveToolCallsComponent {
    type State = AppState;

    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer) {
        if state.active_tool_calls.is_empty() {
            return;
        }

        let mut lines = Vec::new();

        for tool_call in &state.active_tool_calls {
            let status_indicator = match &tool_call.status {
                ToolCallStatus::Starting => Span::styled("○", Style::default().fg(Color::Gray)),
                ToolCallStatus::AwaitingApproval => {
                    Span::styled("◎", Style::default().fg(Color::Yellow))
                }
                ToolCallStatus::Executing => {
                    if tool_call.is_subagent_task && !tool_call.subagent_steps.is_empty() {
                        Span::styled("⊙", Style::default().fg(Color::Blue))
                    } else {
                        Span::styled("●", Style::default().fg(Color::Cyan))
                    }
                }
                ToolCallStatus::Completed => Span::styled("✓", Style::default().fg(Color::Green)),
                ToolCallStatus::Error(_) => Span::styled("✗", Style::default().fg(Color::Red)),
            };

            let mut spans = vec![
                status_indicator,
                Span::raw(" "),
                Span::raw(&tool_call.display_name),
            ];

            match &tool_call.status {
                ToolCallStatus::AwaitingApproval => {
                    spans.push(Span::styled(
                        " [Awaiting Approval]",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::ITALIC),
                    ));
                }
                ToolCallStatus::Error(err) => {
                    spans.push(Span::styled(
                        format!(" [Error: {}]", err),
                        Style::default().fg(Color::Red),
                    ));
                }
                ToolCallStatus::Executing if tool_call.is_subagent_task => {
                    // Show progress for active subagent tasks
                    if !tool_call.subagent_steps.is_empty() {
                        let progress = tool_call.get_progress_indicator();
                        spans.push(Span::styled(
                            format!(" {}", progress),
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::ITALIC),
                        ));
                    }
                }
                _ => {}
            }

            lines.push(Line::from(spans));

            // Show subagent steps if executing
            if tool_call.is_subagent_task && tool_call.status == ToolCallStatus::Executing {
                if let Some(running_summary) = tool_call.get_running_summary() {
                    if !running_summary.is_empty() {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled("├─ ", Style::default().fg(Color::DarkGray)),
                            Span::styled(running_summary, Style::default().fg(Color::Gray)),
                        ]));
                    }
                }
            } else if let Some(summary) = &tool_call.result_summary {
                // Show final result summary (non-subagent or completed)
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("⎿ ", Style::default().fg(Color::DarkGray)),
                    Span::styled(summary, Style::default().fg(Color::Gray)),
                ]));
            }
        }

        let paragraph = Paragraph::new(lines);
        paragraph.render(area, buf);
    }
}
```

**Key Changes:**
- Checks `is_subagent_task` to determine rendering style
- Shows progress indicator (⊙) for active subagents
- Displays running summary via `get_running_summary()`
- Shows recent steps (last 3) to avoid clutter
- Preserves final result summary display

**Display Example:**
```
⊙ Task[plan](Architecture Analysis) ⊙ Step 5
  ├─ thinking → Checking module structure → Analyzing dependencies
● Task[file_ops](Read Config) [Completed]
  ⎿ Read 256 bytes from .config/app.toml
```

#### Step 5.2: Optional - Add Detailed Subagent Steps View

For verbose mode or detailed view, show full step history:

```rust
pub fn render_detailed_subagent_steps(
    tool_call: &ActiveToolCall,
    max_lines: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    
    for step in tool_call.subagent_steps.iter().rev().take(max_lines) {
        let icon = match step.action_type.as_str() {
            "thinking" => "💭",
            "tool_starting" => "▶",
            "tool_completed" => "✓",
            "tool_result" => "📊",
            _ => "•",
        };
        
        lines.push(Line::from(vec![
            Span::raw(format!("  {} [{}] ", icon, step.step_number)),
            Span::styled(&step.description, Style::default().fg(Color::Gray)),
        ]));
    }
    
    lines
}
```

This would be used in a detailed/debug view if needed.

### Phase 6: Testing and Integration

#### Step 6.1: Update Tests

Modify existing tests to verify event flow:

```rust
#[tokio::test]
async fn test_subagent_progress_events() {
    // Test that TaskManager emits SubagentStepProgress events
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    
    let task_manager = TaskManager::new(...)
        .with_event_sender(event_tx)
        .with_tool_call_id("task-123".to_string());
    
    tokio::spawn(async move {
        let _ = task_manager.execute_task(task_def).await;
    });
    
    // Verify SubagentStepProgress events are emitted
    let mut seen_progress = false;
    while let Some(event) = event_rx.recv().await {
        if matches!(event, AgentEvent::SubagentStepProgress { .. }) {
            seen_progress = true;
            break;
        }
    }
    
    assert!(seen_progress, "Should emit SubagentStepProgress events");
}
```

#### Step 6.2: Integration Testing

Test full flow:
1. Main agent calls task tool
2. Task tool passes context to TaskManager
3. TaskManager emits SubagentStepProgress events
4. AppState receives and updates active_tool_calls
5. Component renders with progress
6. User sees real-time subagent steps in TUI

## Implementation Dependencies and Order

### Dependency Graph
```
1. Phase 1 (Event System) - MUST DO FIRST
   └─ Add AgentEvent variants (SubagentStepProgress, SubagentTaskComplete)
   └─ Add ActiveToolCall fields and helper methods
   └─ No dependencies on other code

2. Phase 2 (TaskManager Enhancement) - DEPENDS ON PHASE 1
   └─ TaskManager struct modifications
   └─ Event bridging logic
   └─ Requires AgentEvent variants to exist

3. Phase 3 (Tool Context) - INDEPENDENT
   └─ Add Tool trait extensions
   └─ Modify ToolExecutor
   └─ Can happen in parallel with Phase 2

4. Phase 4 (AppState Handlers) - DEPENDS ON PHASES 1 & 2
   └─ Event handler implementations
   └─ Requires new AgentEvent variants
   └─ Requires TaskManager changes to emit events

5. Phase 5 (Component Rendering) - DEPENDS ON PHASES 1 & 4
   └─ Update active_tool_calls.rs
   └─ Requires ActiveToolCall changes
   └─ Requires AppState event handling

6. Phase 6 (Testing) - DEPENDS ON ALL PHASES
   └─ Verify full integration
```

### Recommended Implementation Order
1. **Day 1**: Phase 1 (Event system) - ~2 hours
2. **Day 1**: Phase 3 (Tool context) - ~1.5 hours (can happen in parallel)
3. **Day 2**: Phase 2 (TaskManager) - ~2 hours
4. **Day 2**: Phase 4 (AppState handlers) - ~1 hour
5. **Day 2-3**: Phase 5 (Rendering) - ~2 hours
6. **Day 3**: Phase 6 (Testing & Integration) - ~2 hours

**Total Estimate**: 10.5 hours of development

## Key Design Decisions

### 1. **Tool Trait Extension vs. Middleware**
- ✅ **Chosen**: Trait extension with optional `execute_with_context()`
- Why: Backward compatible, opt-in, minimal code changes to ToolExecutor
- Alternative: Middleware layer (more complex, affects all tools)

### 2. **Event Bridging: Transform vs. Passthrough**
- ✅ **Chosen**: Transform subagent events to new `SubagentStepProgress` variants
- Why: Decouples subagent internals from parent agent display logic, cleaner UI state
- Alternative: Passthrough all events (clutters event system, requires filtering in UI)

### 3. **Storage: Append vs. Replace**
- ✅ **Chosen**: Append subagent steps to history list
- Why: Can show recent steps, preserves full history for debugging
- Alternative: Replace (only show current step) - loses context

### 4. **Running Summary: Last N vs. Full List**
- ✅ **Chosen**: Show last 3 steps in UI (via `get_running_summary()`)
- Why: Prevents UI clutter, still informative
- Full list available for detailed/debug views

### 5. **Progress Indication: Explicit Count vs. Spinner**
- ✅ **Chosen**: Show "Step N" count with status indicator
- Why: More informative than spinner alone
- Enhancement: Could add visual progress bar if total steps known

## Data Flow Example

### Scenario: User calls task tool to plan feature

```
1. User: "Create implementation plan for feature X"
                    ↓
2. Main Agent receives this, calls TaskTool with args
                    ↓
3. ToolExecutor:execute_tool_call()
   - Extracts tool_call_id = "call_abc123"
   - Creates ToolExecutionContext with tool_call_id and event_tx
   - Calls TaskTool.execute_with_context(args, context)
                    ↓
4. TaskTool.execute_impl()
   - Creates TaskManager
   - Calls .with_event_sender(event_tx)
   - Calls .with_tool_call_id("call_abc123")
   - Awaits execute_task()
                    ↓
5. TaskManager.execute_task()
   - Spawns subagent
   - Subagent emits AgentEvent::ToolExecutionStarted
   - Event collector receives: ToolExecutionStarted { tool_name: "read_file", .. }
   - Transforms to: SubagentStepProgress { 
       tool_call_id: "call_abc123",
       step_number: 1,
       action_type: "tool_starting",
       description: "Executing read_file",
       ..
     }
   - Sends to parent event_tx
                    ↓
6. Event Loop: process_agent_events()
   - Receives SubagentStepProgress
   - Calls app.handle_agent_event(SubagentStepProgress)
                    ↓
7. AppState.handle_agent_event()
   - Finds active_tool_call with id "call_abc123"
   - Creates SubagentStepSummary { step_number: 1, .. }
   - Calls tool_call.add_subagent_step(summary)
   - Updates subagent_steps list
                    ↓
8. Render Loop
   - Component checks tool_call.is_subagent_task
   - Calls tool_call.get_running_summary()
   - Renders: "⊙ Task[plan](Feature X planning) ⊙ Step 1"
   - Next line: "  ├─ [1] Executing read_file"
                    ↓
9. More subagent events flow through same pipeline
   - User sees updates in real-time
   
10. Subagent completes
    - TaskManager emits SubagentTaskComplete { tool_call_id: "call_abc123", .. }
    - AppState receives, marks completion
    - TaskManager returns TaskResult with success message
    - TaskTool returns final result to main agent
                    ↓
11. Final Response
    - Active tool call transitions to Completed
    - Result summary displayed
    - Tool call moved to message history
```

## Rollout Strategy

### Phase 1: Minimal Feature
- Implement Phase 1 only
- Add AgentEvent variants
- Add basic structure to ActiveToolCall
- **Result**: No visible change yet, foundation ready

### Phase 2: Event Pipeline
- Add Phase 2 (TaskManager) + Phase 3 (Tool context)
- Events emitted but not yet displayed
- **Result**: Data flows through system, not visible in UI

### Phase 3: Display
- Add Phase 4 (AppState handlers) + Phase 5 (Rendering)
- **Result**: Subagent steps visible in active_tool_calls component

### Phase 4: Polish
- Add Phase 6 (tests)
- Refine display format based on user feedback
- Add optional detailed view if needed
- **Result**: Production-ready feature

## Backward Compatibility

All changes maintain backward compatibility:
- ✅ Existing Tool implementations unaffected (new trait method optional)
- ✅ TaskManager API extended, not replaced (builder pattern)
- ✅ AppState event handler extended (match is exhaustive but default handling exists)
- ✅ ActiveToolCall struct extended with default values
- ✅ Component rendering preserves existing logic, adds conditional branches

## Potential Extensions (Future)

Once this is working, consider:

1. **Historical View**: Show all steps for a completed task in a popup
2. **Filtering**: Allow user to show only tool execution steps (hide thinking)
3. **Performance Stats**: Track timing per step, show cumulative duration
4. **Estimated Time**: Use historical data to estimate remaining time
5. **Step Replay**: Allow user to expand and see details of each step
6. **Conditional Formatting**: Color-code different action types differently

<function_calls>
<invoke name="bash">
<parameter name="command">find . -type f -name "tool_executor.rs" | xargs head -50