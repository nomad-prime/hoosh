# Data Model: Model Cascade System

**Date**: 2025-12-10 | **Phase**: 1 Design

---

## Core Entities

### TaskComplexity

Represents complexity analysis result for incoming task.

```rust
pub struct TaskComplexity {
    pub level: ComplexityLevel,
    pub confidence: f32,  // 0.0-1.0
    pub reasoning: String,
    pub metrics: ComplexityMetrics,
}

pub enum ComplexityLevel {
    Light,
    Medium,
    Heavy,
}

pub struct ComplexityMetrics {
    pub structural_depth: u32,      // 1-5 scale
    pub structural_depth_score: f32, // 0.0-1.0
    pub action_density: usize,       // verb count
    pub action_density_score: f32,   // 0.0-1.0
    pub code_signals_score: f32,     // 0.0-1.0, based on presence + CC
    pub concept_count: usize,        // unique entities
}
```

**Lifecycle**: Created when task starts; used for initial tier routing; immutable.

---

### ModelTier

Represents capability tier and backend model mapping.

```rust
pub struct ModelTier {
    pub tier: ExecutionTier,
    pub backend: BackendName,
    pub models: Vec<String>,         // ["gpt-4o-mini", ...]
    pub max_cost_per_request_cents: u32,
}

pub enum ExecutionTier {
    Light,
    Medium,
    Heavy,
}

pub enum BackendName {
    Anthropic,
    OpenAI,
    TogetherAI,
    Ollama,
}
```

**Source**: Loaded from config `cascades` section at startup. Static per backend.

---

### CascadeContext

Maintains escalation state during task execution.

```rust
pub struct CascadeContext {
    pub task_id: String,
    pub current_tier: ExecutionTier,
    pub escalation_path: Vec<ExecutionTier>,
    pub original_task: String,
    pub conversation_history: Vec<ChatMessage>,
    pub time_started: SystemTime,
    pub approval_status: ApprovalStatus,
}

pub enum ApprovalStatus {
    NotRequested,
    Pending {
        escalation_request_id: String,
        requested_at: SystemTime,
    },
    Approved {
        approved_at: SystemTime,
        operator_notes: String,
    },
    Rejected {
        rejected_at: SystemTime,
        reason: String,
    },
}
```

**Lifecycle**: Created when task begins; in-memory only; destroyed on task completion. Not persisted.

**Constraints**:
- One CascadeContext per task (sequential execution only in Phase 1)
- Conversation history must fit in memory
- Escalation path length ≤ 3 (Light → Medium → Heavy max)

---

### CascadeConfig

Configuration for cascade activation and behavior.

```rust
pub struct CascadeConfig {
    pub enabled: bool,
    pub routing_policy: RoutingPolicy,
    pub escalation_policy: EscalationPolicy,
    pub default_tier: ExecutionTier,
    pub cost_limits: Option<CostLimits>,
    pub escalation_needs_approval: bool,
    pub model_tiers: Vec<ModelTier>,
}

pub enum RoutingPolicy {
    MultiSignal,      // Structural + Action + Code signals
    ThresholdBased,   // Simple cutoffs
}

pub enum EscalationPolicy {
    AllowAll,
    LightToMediumOnly,
    MediumToHeavyOnly,
}

pub struct CostLimits {
    pub light_tier_max_cents: u32,
    pub medium_tier_max_cents: u32,
    pub heavy_tier_max_cents: u32,
}
```

**Source**: From `cascades` section in config file. Optional. If absent, cascades disabled.

**Parsing**: TOML deserialization with defaults.

---

### CascadeEvent

Structured event for observability.

```rust
pub struct CascadeEvent {
    pub event_id: String,
    pub event_type: CascadeEventType,
    pub task_id: String,
    pub tier: ExecutionTier,
    pub timestamp: SystemTime,
    pub duration_ms: Option<u64>,
    pub reason: String,
    pub metrics: EventMetrics,
}

pub enum CascadeEventType {
    CascadeCreated,
    TaskRouted,
    EscalationRequested,
    EscalationApproved,
    EscalationRejected,
    EscalationExecuted,
    TaskCompleted,
    TaskFailed,
}

pub struct EventMetrics {
    pub success: bool,
    pub input_tokens: Option<usize>,
    pub output_tokens: Option<usize>,
    pub escalation_count: u32,
    pub retry_count: u32,
    pub latency_excluding_llm_ms: Option<u64>,
}
```

**Serialization**: serde_json for JSONL persistence.

---

## Entity Relationships

```
CascadeConfig
    ├─→ [ModelTier] (1..* per backend)
    └─→ CostLimits (optional)

Task Input
    ├─→ TaskComplexity (1:1 analysis)
    ├─→ CascadeContext (1:1 per task)
    │   ├─→ ExecutionTier (current)
    │   ├─→ [ChatMessage] (conversation history)
    │   └─→ ApprovalStatus (escalation state)
    └─→ [CascadeEvent] (*.* lifecycle events)

ModelTier
    ├─→ ExecutionTier (Light/Medium/Heavy)
    ├─→ BackendName (Anthropic/OpenAI/...)
    └─→ models: Vec<String> (concrete model IDs)
```

---

## State Transitions

### CascadeContext State Machine

```
Created
  ├─ Initial routing: TaskComplexity → ExecutionTier
  │
  ├─ Execution on tier
  │  ├─ Success → TaskCompleted
  │  └─ Needs escalation → EscalationRequested
  │
  ├─ Escalation Request (if EscalationNeedsApproval=true)
  │  ├─ Approved → EscalationExecuted → [re-execute on new tier]
  │  └─ Rejected → TaskFailed (with rejection reason)
  │
  └─ Task Completion
     ├─ TaskCompleted (success)
     └─ TaskFailed (error)

// Context destroyed after completion (garbage collected)
```

### ApprovalStatus Lifecycle

```
NotRequested
  → Pending (on escalate tool invocation)
    ├─ → Approved (operator accepts)
    │   └─ → execute on new tier
    └─ → Rejected (operator denies)
        └─ → fail task
```

---

## Validation Rules

### TaskComplexity
- `confidence` ∈ [0.0, 1.0]
- `level` must match score thresholds (see research.md)
- `reasoning` non-empty string

### CascadeContext
- `escalation_path.len()` ≤ 3
- `escalation_path` is monotonically increasing (Light → Medium → Heavy)
- `conversation_history` non-empty after first message
- `current_tier` ∈ escalation_path

### CascadeConfig
- If `enabled=false`, all other fields ignored
- `default_tier` must exist in `model_tiers`
- All `model_tiers` must have non-empty `models` list
- Cost limits must be positive if specified

### CascadeEvent
- `timestamp` ≤ `now()`
- `duration_ms` ≥ 0 if present
- `event_type` determines required fields (e.g., EscalationRequested requires reason)

---

## Data Volume & Constraints

| Entity | Cardinality | Size | Notes |
|--------|---|---|---|
| TaskComplexity | 1 per task | ~500B | Immutable |
| CascadeContext | 1 per active task | 10-50KB | In-memory only |
| Conversation history | ~5-50 messages | 5-50KB | Preserved during escalation |
| CascadeEvent | 5-20 per task | ~1KB each | Appended to JSONL |
| CascadeConfig | 1 per process | ~2KB | Loaded at startup |
| ModelTier | 3-12 per config | ~200B each | Static |

**Phase 1 Assumptions**:
- Max 100 concurrent in-memory tasks (sequential in Phase 1, so 1)
- Event log persisted every 30s → max 1000 events/min
- Single backend (all Anthropic or all OpenAI)

---

## Integration Points

### agent/core.rs
- Create CascadeContext at task start
- Route through ComplexityAnalyzer
- Check ApprovalStatus before escalation

### backends/strategy.rs
- Load ModelTier from CascadeConfig
- Map tier to concrete model ID

### tools/mod.rs
- Register `escalate` tool
- Tool invocation creates EscalationRequest
- Trigger HITL approval workflow

### config/mod.rs
- Parse `cascades` section → CascadeConfig
- Provide accessor: `config.cascade_config()`

---

**Status**: ✅ Ready for contracts generation
