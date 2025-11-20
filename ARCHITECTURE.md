# Resource Budget Awareness Architecture

## High-Level Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    Task Tool (task_tool.rs)                 │
│  • Accepts optional timeout_seconds parameter               │
│  • Creates TaskDefinition with budget info                  │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│              TaskManager (task_manager.rs)                  │
│  • Creates BudgetInfo from TaskDefinition                   │
│  • Initializes Agent with budget tracking                   │
│  • Passes budget info to system message generation          │
└────────────────────┬────────────────────────────────────────┘
                     │
     ┌───────────────┴───────────────┐
     │                               │
     ▼                               ▼
┌─────────────────────┐    ┌──────────────────────────┐
│   AgentType         │    │   BudgetInfo             │
│  (mod.rs)           │    │  (NEW struct)            │
│                     │    │                          │
│ • system_message() ◄─────┤  • start_time: Instant   │
│   + budget_info     │    │  • timeout_seconds: u64  │
└─────────────────────┘    │  • max_steps: usize      │
                           │  • current_step: usize   │
                           │                          │
                           │  Methods:                │
                           │  • elapsed_seconds()     │
                           │  • remaining_seconds()   │
                           │  • steps_remaining()     │
                           │  • is_time_critical()    │
                           │  • progress_percent()    │
                           └──────────────────────────┘
```

## Data Flow: Subagent Execution with Budget Awareness

```
┌──────────────────┐
│   Task Tool      │
│  (parent call)   │
└────────┬─────────┘
         │ execute_task(TaskDefinition {
         │   timeout_seconds: Some(600),
         │   agent_type: Plan,
         │   ...
         │ })
         │
         ▼
┌──────────────────────────────┐
│   TaskManager::execute_task  │
│                              │
│  1. Create BudgetInfo        │
│     start_time = Instant::now()
│     timeout_seconds = 600    │
│     max_steps = 50 (Plan)    │
│                              │
│  2. Format budget guidance   │
│     "You have ~600s, 50 steps"
│                              │
│  3. Create Agent + system msg
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│      Agent::handle_turn      │
│                              │
│  for step in 0..max_steps {  │
│                              │
│    • check_resource_budget() │
│      - Calculate elapsed     │
│      - Calculate remaining   │
│      - If < critical: break  │
│                              │
│    • If time < 30%:          │
│      - Send BudgetWarning    │
│      - Continue processing   │
│                              │
│    • LLM call with context   │
│    • Process response        │
│    • Handle tool calls       │
│  }                           │
│                              │
│  if budget_exceeded:         │
│    generate_graceful_        │
│    conclusion()              │
└────────┬─────────────────────┘
         │
         ▼
┌──────────────────────────────┐
│    Conversation object       │
│    (messages + metadata)     │
│                              │
│  • System message with:      │
│    - Agent instructions      │
│    - Budget constraints      │
│    - Time/step limits        │
│                              │
│  • User message (task)       │
│  • Tool calls/results        │
│  • Final response            │
└──────────────────────────────┘
```

## State Tracking: Agent Lifecycle

```
INITIALIZATION
├─ Task Tool receives call
├─ TaskManager creates BudgetInfo
│  └─ start_time = T0
│  └─ timeout_seconds = 600
│  └─ max_steps = 50
│  └─ current_step = 0
├─ Agent initialized
│  └─ budget_info field set
│  └─ start_time stored
└─ System message generated with budget text

EXECUTION LOOP
├─ Step N (0 to max_steps)
│  ├─ Check budget
│  │  ├─ elapsed = Instant::now() - start_time
│  │  ├─ remaining_time = timeout_seconds - elapsed.as_secs()
│  │  ├─ remaining_steps = max_steps - current_step
│  │  │
│  │  └─ if remaining_time < 10s OR remaining_steps < 2:
│  │     └─ Send AgentEvent::BudgetExceeded
│  │     └─ Break from loop
│  │     └─ Trigger graceful_conclusion()
│  │
│  ├─ if remaining_time < 30% AND not warned:
│  │  └─ Send AgentEvent::BudgetWarning
│  │  └─ Set warned flag
│  │
│  ├─ LLM call (sends conversation + tools)
│  ├─ Parse response (content or tool_calls)
│  ├─ Handle tools if needed
│  └─ current_step += 1

COMPLETION
├─ Loop exits naturally OR budget exceeded
├─ TaskManager receives agent completion
├─ TaskResult returned with:
│  └─ success: bool
│  └─ output: String
│  └─ events: Vec<TaskEvent> (includes budget events)
└─ Parent agent receives result via SubagentTaskComplete event
```

## Budget Information Communication

### Method 1: System Message (Primary)
**When**: During task initialization  
**Where**: Conversation system/user messages  
**Content**: Formatted budget constraints  
**Example**:
```
You have approximately 580 seconds (9.7 minutes) and 48 steps of 50 maximum.
Budget guidelines:
- If time runs low, prioritize wrapping up gracefully
- Provide best effort with findings so far
- When time < 10% remaining, prepare final summary
```

### Method 2: Events (Monitoring)
**When**: During execution  
**Where**: AgentEvent::BudgetWarning and BudgetExceeded  
**Content**: Real-time budget status  
**Recipients**: Parent agent, UI, logging  

### Method 3: Loop Checks (Internal)
**When**: Each iteration  
**Where**: Agent::handle_turn() loop  
**Content**: Direct budget calculations  
**Action**: Decision to continue or wrap up

## Key Structures

### BudgetInfo struct
```rust
pub struct BudgetInfo {
    pub start_time: Instant,
    pub timeout_seconds: u64,
    pub max_steps: usize,
    pub current_step: usize,
}

impl BudgetInfo {
    pub fn elapsed_seconds(&self) -> u64 { ... }
    pub fn remaining_seconds(&self) -> u64 { ... }
    pub fn steps_remaining(&self) -> usize { ... }
    pub fn progress_percentage(&self) -> f32 { ... }
    pub fn is_time_critical(&self) -> bool { ... }
}
```

### Agent struct changes
```rust
pub struct Agent {
    // ... existing fields ...
    budget_info: Option<BudgetInfo>,
    start_time: Instant,
    budget_warning_sent: bool,
}
```

### New AgentEvents
```rust
pub enum AgentEvent {
    // ... existing variants ...
    BudgetWarning {
        remaining_time: u64,      // seconds
        remaining_steps: usize,
        time_percent: f32,        // 0.0-100.0
    },
    BudgetExceeded {
        reason: BudgetExceededReason,  // Time or Steps
        elapsed_time: u64,
        steps_taken: usize,
    },
}

pub enum BudgetExceededReason {
    TimeLimit,
    StepLimit,
}
```

## Integration Points

### 1. Task Tool → TaskManager
```
TaskArgs {
    timeout_seconds: Option<u64>  // NEW
}
│
▼
TaskDefinition {
    timeout_seconds: Option<u64>  // EXISTING, used here
}
```

### 2. TaskManager → Agent
```
BudgetInfo created from:
- timeout_seconds (TaskDefinition)
- max_steps (AgentType)

Passed to:
- Agent::with_budget_info()
- AgentType::system_message()
```

### 3. Agent → Conversation
```
Budget info in system message:
- Time constraints
- Step limits
- Guidance on graceful shutdown

Influences:
- LLM understanding of constraints
- Tool selection
- Response brevity
```

### 4. Agent → Events
```
Budget checks emit:
- BudgetWarning (periodic)
- BudgetExceeded (terminal)

Listeners:
- Parent agent
- UI/TUI components
- Logging system
```

## Configuration Constants

```rust
// src/task_management/mod.rs
pub const PLAN_AGENT_TIMEOUT_SECONDS: u64 = 600;      // 10 minutes
pub const EXPLORE_AGENT_TIMEOUT_SECONDS: u64 = 300;   // 5 minutes
pub const BASH_TOOL_TIMEOUT_SECONDS: u64 = 30;        // 30 seconds

// Thresholds
pub const BUDGET_WARNING_THRESHOLD: f32 = 0.30;       // 30% remaining
pub const BUDGET_CRITICAL_THRESHOLD: u64 = 10;        // 10 seconds
pub const BUDGET_CRITICAL_STEPS: usize = 2;           // 2 steps
```

## Execution Timeline Example

```
Task: "Analyze codebase and create plan"
Timeout: 600 seconds
Max Steps: 50

Timeline:
T=0s        Agent starts, receives budget message
T=10s       Step 1: Explore file structure
T=30s       Step 2: Analyze main.rs
T=60s       Step 3: Identify patterns (now at 60s / 600s = 10%)
T=180s      Step 10: Start drafting plan
T=420s      Step 35: BudgetWarning sent (30% = 180s remaining)
T=480s      Step 45: Approaching limits
T=550s      Step 49: Critical threshold (50s = 8.3% remaining)
T=580s      BudgetExceeded (20s < 10s threshold)
            → Generate graceful conclusion
            → Return final plan with best effort results
T=590s      Return to parent agent with summary
```

## Error Handling

```
Budget Exceeded Scenarios:

1. Time Limit (most common)
   ├─ LLM call takes longer than remaining time
   ├─ Agent interrupted
   ├─ Event: BudgetExceeded(TimeLimit)
   └─ Result: Best effort with apology message

2. Step Limit
   ├─ Max 50 steps (Plan) or 30 steps (Explore) reached
   ├─ Loop naturally exits
   ├─ Event: MaxStepsReached (existing)
   └─ Result: Normal completion

3. Graceful Degradation
   ├─ Budget critical but not yet exceeded
   ├─ Agent still functions, receives BudgetWarning
   ├─ Can adjust behavior (shorter responses, skip optimization)
   └─ Completes with current best state
```

## Backward Compatibility

All changes are backward compatible:
- `BudgetInfo` is optional in Agent
- Budget checks don't affect behavior if not set
- Existing agents work without budget tracking
- Old TaskDefinition usage still valid
- Tests updated to work with/without budget info

---

**Diagram Legend:**
- `┌─────┐` = Module/Component
- `──►` = Data flow
- `◄──` = Configuration
- `▼` = Execution flow
- `├─` = Hierarchical relationship
