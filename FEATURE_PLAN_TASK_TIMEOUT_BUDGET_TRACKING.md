# Feature Plan: Task Tool Timeout Limit & Time/Step Budget Tracking

## Overview
Enhance the task tool system to support configurable, larger timeout limits and provide subagents with real-time visibility into their time and step budgets, allowing them to wrap up gracefully when running low on resources.

---

## Current State Analysis

### Timeout Implementation
- **Location**: `src/task_management/task_manager.rs`
- **Current Default**: 600 seconds (10 minutes) set in `TaskDefinition::new()`
- **Implementation**: Uses `tokio::time::timeout()` wrapper around `agent.handle_turn()`
- **Function Signature**: Takes hardcoded timeout in bash/other tools at 30 seconds

### Agent Loop
- **Location**: `src/agent/core.rs` - `handle_turn()` method
- **Loop Structure**: `for step in 0..self.max_steps` 
- **Max Steps**: Set per agent type (Plan: 50, Explore: 30)
- **Event Emission**: Uses `AgentEvent` channel to communicate progress

### Subagent Communication
- **Current**: Only system message with task prompt passed
- **Events**: Parent can see `SubagentStepProgress` and `SubagentTaskComplete` events
- **No Budget Info**: Subagent LLM has no awareness of time/step constraints

---

## Requirements

### R1: Configurable Timeout Limits
- Allow override of default timeout per task
- Support task tool parameter for custom timeout
- Add configuration file support for global defaults

### R2: Real-time Budget Awareness
- Track elapsed time since task start
- Track current step count vs max steps
- Calculate remaining time budget
- Calculate remaining steps

### R3: Subagent Awareness
- Pass budget information to LLM via system message updates
- Enable graceful wrap-up when low on resources
- Provide actionable information (not just warnings)
---

## Implementation Plan (Step-by-step)

### Phase 1: Budget Tracking Infrastructure

#### Step 1-1: Create Budget Tracking Struct
**File**: `src/task_management/mod.rs`
```
Add new struct:
- struct ExecutionBudget {
    start_time: std::time::Instant,
    max_duration: Duration,
    max_steps: usize,
  }
  
Methods:
- elapsed_seconds() -> u64
- remaining_seconds() -> u64
- time_percentage_used() -> f32
- steps_percentage_used(current: usize) -> f32
- should_wrap_up(current_step: usize) -> bool
```

#### Step 1-2: Add Budget to TaskDefinition
**File**: `src/task_management/mod.rs`
```
Extend TaskDefinition:
- Add field: budget: Option<ExecutionBudget>
- Add builder method: with_max_duration(secs: u64)
- Add method: initialize_budget()
```

#### Step 1-3: Add BudgetInfo to TaskResult
**File**: `src/task_management/mod.rs`
```
Extend TaskResult:
- Add field: budget_info: Option<BudgetInfo>
  where BudgetInfo {
    elapsed_seconds: u64,
    remaining_seconds: u64,
    steps_completed: usize,
    max_steps: usize,
  }
```

### Phase 2: Agent-Level Budget Awareness

#### Step 2-1: Add Budget to Agent Struct
**File**: `src/agent/core.rs`
```
Extend struct Agent:
- Add field: execution_budget: Option<Arc<ExecutionBudget>>
- Add builder method: with_execution_budget()
```

#### Step 2-2: Create BudgetContext
**File**: `src/agent/agent_events.rs`
```
Add new event variant:
- BudgetUpdate {
    elapsed_seconds: u64,
    remaining_seconds: u64,
    current_step: usize,
    max_steps: usize,
    time_pressure: f32,
    step_pressure: f32,
  }
```

#### Step 2-3: Update handle_turn Loop
**File**: `src/agent/core.rs` - `handle_turn()` method
```
In the for loop (for step in 0..self.max_steps):
1. At start of each iteration, check if budget exists
2. Calculate budget metrics
3. Emit BudgetUpdate event
4. Check should_wrap_up() and exit early if needed
5. Update system message with budget info
```

#### Step 2-4: Add Budget-Aware System Prompt
**File**: `src/agent/core.rs`
```
New helper method: get_budget_aware_system_prompt()
- Base system prompt
- Append budget info if available:
  "You have completed X/Y steps and used Z/T seconds.
   Remaining: Y-X steps, T-Z seconds.
   If under 20% budget remaining, prioritize wrapping up."
```

### Phase 3: TaskManager Integration

#### Step 3-1: Initialize Budget in TaskManager
**File**: `src/task_management/task_manager.rs` - `execute_task()` method
```
Before creating Agent:
1. Create ExecutionBudget from TaskDefinition
2. Initialize with start_time
3. Store in Arc for sharing
```

#### Step 3-2: Pass Budget to Agent
**File**: `src/task_management/task_manager.rs`
```
When creating Agent:
- Call agent.with_execution_budget(budget.clone())
```

#### Step 3-3: Enhance Event Transformation
**File**: `src/task_management/task_manager.rs` - `transform_to_subagent_event()`
```
Include budget info in SubagentStepProgress:
- Add fields: elapsed_seconds, remaining_seconds
- Include in step progress emitted to parent
```

#### Step 3-4: Capture Budget Info in Result
**File**: `src/task_management/task_manager.rs` - `execute_task()` method
```
Before returning TaskResult:
1. Extract final budget metrics
2. Create BudgetInfo struct
3. Attach to TaskResult.with_budget_info()
```

### Phase 4: TaskTool Parameter Support

#### Step 4-1: Extend TaskTool Parameters
**File**: `src/tools/task_tool.rs`
```
Extend TaskArgs struct:
- Add optional field: timeout_seconds: Option<u64>
- Add optional field: max_steps: Option<usize>
```

#### Step 4-2: Update Parameter Schema
**File**: `src/tools/task_tool.rs` - `parameter_schema()`
```
Add to JSON schema:
- "timeout_seconds": { type: integer, min: 30, max: 3600 }
- "max_steps": { type: integer, min: 5, max: 200 }
```

#### Step 4-3: Apply Parameters to TaskDefinition
**File**: `src/tools/task_tool.rs` - `execute_impl()` method
```
After creating TaskDefinition:
- If timeout_seconds provided, call .with_timeout()
- If max_steps provided, create modified agent_type or pass separately
```

### Phase 5: Configuration Support

#### Step 5-1: Add Config File Entries
**File**: `src/config/mod.rs` or new `src/config/task_tool.rs`
```
New config section:
[task_tool]
default_timeout_seconds = 600  # 10 minutes
default_plan_max_steps = 50
default_explore_max_steps = 30
```

#### Step 5-2: Load Config in TaskDefinition
**File**: `src/task_management/mod.rs`
```
In TaskDefinition::new():
- Load from config if available
- Allow parameter override
```

### Phase 6: Event System Updates

#### Step 6-1: Update AgentEvent Enum
**File**: `src/agent/agent_events.rs`
```
Add variants:
- BudgetUpdate { ... }
- LowBudgetWarning { remaining_seconds, steps_remaining }
```

#### Step 6-2: Update SubagentStepProgress
**File**: `src/agent/agent_events.rs`
```
Extend SubagentStepProgress:
- Add: elapsed_seconds: u64
- Add: remaining_seconds: u64
- Add: steps_remaining: usize
```

### Phase 7: Testing & Documentation

#### Step 7-1: Unit Tests for ExecutionBudget
**File**: `src/task_management/mod.rs` (tests section)
```
Tests:
- budget_elapsed_calculation
- budget_remaining_calculation
- budget_percentage_usage
- should_wrap_up_thresholds
- budget_serialization
```

#### Step 7-2: Integration Tests
**File**: `tests/` new file: `task_timeout_budget_integration_test.rs`
```
Tests:
- task_completes_within_timeout
- task_wraps_up_gracefully_at_70_percent_budget
- task_timeout_triggers_correctly
- parent_receives_budget_updates
- configuration_overrides_work
```

#### Step 7-3: TaskManager Tests Update
**File**: `src/task_management/task_manager.rs` (tests section)
```
Add/update tests:
- test_task_budget_tracking
- test_budget_info_in_result
- test_early_wrap_up_at_low_budget
```

#### Step 7-4: Documentation
**Files**:
- `TASK_TOOL_FEATURES.md` - Feature guide for users
- Update `src/tools/task_tool.rs` - Tool description with new parameters
- Code comments in core files

### Phase 8: CLI/UI Integration

#### Step 8-1: Task Tool Description Update
**File**: `src/tools/task_tool.rs` - `description()`
```
Update to mention:
- Available timeout parameters
- How subagents use budgets
- Examples
```

#### Step 8-2: Display Budget Info
**File**: `src/tui/components/` or `src/cli/`
```
If applicable, display in progress:
- Current step / max steps
- Elapsed / total time
- Budget pressure indicator
```

---

## Detailed Implementation Notes

### Budget Calculation Formula
```
time_percentage_used = (elapsed / total_duration) * 100
step_percentage_used = (current_step / max_steps) * 100
max_pressure = MAX(time_percentage_used, step_percentage_used)

should_wrap_up = max_pressure >= 70% OR
                 (elapsed >= total_duration * 0.8)
```

### System Prompt Update Strategy
**Location**: Before each LLM call in `handle_turn()`

```
If budget available:
- Calculate metrics every iteration
- Append to system message
- Update conversation with new budget awareness

Format:
"[EXECUTION BUDGET]
Current Progress: Step 15/30, Elapsed: 45/300 seconds
Time Used: 15%, Steps Used: 50%
Remaining: 285 seconds, 15 steps
Status: NOMINAL (Continue normally)

If >= 70% budget used:
Status: LOW BUDGET (Prioritize wrapping up. Provide final answer soon.)"
```

### Graceful Wrap-Up Behavior
```
When should_wrap_up() == true:
1. Add low-budget message to conversation
2. Don't exit immediately - let LLM decide
3. LLM typically will provide final answer
4. If still looping at 90%, force exit with accumulated result
```

### Timeout Handling
```
Current: Task timeout -> returns "Task timed out" error
Updated: Task timeout -> 
  1. Emit TimeoutWarning event
  2. Force agent.handle_turn() to exit
  3. Collect partial results from conversation
  4. Return partial result with timeout indicator
```

---

## Configuration File Example

**File**: `hoosh.toml` (or equivalent)

```toml
[task_tool]
# Default timeout for task tool execution (seconds)
default_timeout_seconds = 600

# Agent type specific limits
[task_tool.agents.plan]
max_steps = 50
default_timeout_seconds = 600

[task_tool.agents.explore]
max_steps = 30
default_timeout_seconds = 300

# Budget thresholds for early wrap-up
[task_tool.budget]
wrap_up_threshold_percent = 70
timeout_force_exit_percent = 90
warn_threshold_percent = 80
```

---

## API Changes Summary

### New Structs
- `ExecutionBudget`
- `BudgetInfo`

### New Methods
- `Agent::with_execution_budget()`
- `Agent::get_budget_aware_system_prompt()`
- `ExecutionBudget::elapsed_seconds()`
- `ExecutionBudget::remaining_seconds()`
- `ExecutionBudget::should_wrap_up()`
- `TaskDefinition::with_max_duration()`
- `TaskResult::with_budget_info()`

### Updated Parameters
- `TaskTool` - adds `timeout_seconds`, `max_steps` optional parameters
- `TaskArgs` - adds timeout and step limit fields
- `SubagentStepProgress` event - adds budget fields

### Backward Compatibility
✅ All changes are additive
✅ Existing code works unchanged
✅ Optional parameters throughout
✅ Sensible defaults from configuration

---

## Testing Strategy

### Unit Tests
- ExecutionBudget calculation methods
- Budget-aware system prompt generation
- Configuration loading

### Integration Tests
- End-to-end task execution with budget
- Event emission during task execution
- Parent process receives budget updates
- Task wraps up gracefully at budget threshold

### Stress Tests
- Very short timeout (5 seconds)
- Very short step limit (2 steps)
- Rapid budget consumption
- Timeout during tool execution

---

## Rollout Plan

1. **Phase 1-2**: Core infrastructure (non-breaking)
2. **Phase 3-4**: Agent & TaskTool integration (testable independently)
3. **Phase 5-6**: Configuration & events (observable)
4. **Phase 7**: Comprehensive testing
5. **Phase 8**: UI/CLI improvements
6. **Final**: Documentation & release

Each phase can be merged independently and tested before proceeding to next.

---

## Success Criteria

✅ Task tools support configurable timeout > 30 seconds (e.g., 600 seconds by default)
✅ Subagents receive budget information in system context
✅ Subagents gracefully wrap up when approaching budget limits
✅ Parent processes can monitor subagent budget consumption via events
✅ Configuration file allows customization of timeouts
✅ All new features are backward compatible
✅ Comprehensive test coverage (>90%)
✅ Feature documented in guides and code comments

---

## Risk Assessment & Mitigation

| Risk | Severity | Mitigation |
|------|----------|-----------|
| Budget calc overhead | Low | Use Instant, minimal CPU impact |
| LLM confusion from budget context | Medium | Test with various models, make format clear |
| Breaking existing tasks | Low | All changes optional, defaults safe |
| Timeout during critical tool call | Medium | Allow timeout extension for in-flight calls |
| Configuration loading issues | Low | Validate at startup, sensible defaults |

---

## Future Enhancements

- Adaptive timeout based on task complexity
- Per-tool timeout overrides
- Budget-aware tool selection (skip heavy tools if low budget)
- Budget consumption prediction
- Historical budget analytics

