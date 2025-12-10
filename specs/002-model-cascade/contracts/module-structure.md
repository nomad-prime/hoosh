# Module Structure Contract

Defines internal module organization and data flow for the cascade system.

## Module Layout

```
src/cascades/
├── mod.rs                      # Public API re-exports
├── errors.rs                   # CascadeError type & Result<T>
├── types.rs                    # Data structures (TaskComplexity, etc.)
├── complexity.rs               # ComplexityAnalyzer implementation
├── router.rs                   # CascadeRouter implementation
├── context.rs                  # CascadeContext lifecycle management
├── approval.rs                 # ApprovalHandler (HITL TUI integration)
├── events.rs                   # EventLogger (in-memory + JSONL)
└── tests/
    ├── complexity_tests.rs     # Analyzer behavior tests
    ├── router_tests.rs         # Routing logic tests
    ├── integration_tests.rs    # End-to-end cascade flow
    └── fixtures/               # Test data & mock responses
```

## Data Flow

### Task Reception → Escalation → Completion

```
agent/core.rs:execute_task()
    ↓
[1] complexity::analyze(task_description)
    └─→ TaskComplexity { level, metrics, confidence }
    ↓
[2] router::route(complexity)
    └─→ ExecutionTier (Light | Medium | Heavy)
    ↓
[3] context::create(task_id, tier, description)
    └─→ CascadeContext { current_tier, escalation_path, history }
    ↓
[4] LLM execution on initial tier
    ├─→ Success: TaskCompleted event → DONE
    ├─→ NeedsEscalation: [go to 5]
    └─→ Error: TaskFailed event → DONE
    ↓
[5] approval::request_approval(reason, current_tier)
    └─→ ApprovalDecision (approved/rejected)
    ↓
[6] if approved:
        context::escalate() → next_tier
        events::emit(EscalationExecuted)
        goto [4]
    else:
        events::emit(EscalationRejected)
        DONE (failure)
```

## Trait Implementation Responsibility

### New Cascade Traits (Phase 1)

| Trait | Module | Dependencies | Async | Notes |
|-------|--------|--------------|-------|-------|
| `ComplexityAnalyzer` | `complexity.rs` | None (pure) | No | Domain-specific task analyzer |
| `CascadeRouter` | `router.rs` | `CascadeConfig` | No | Deterministic tier assignment |
| `CascadeEventLogger` | `events.rs` | `dashmap`, JSONL writer | Yes | Observability sink |

### Reused Existing Traits

| Trait | Location | Usage | Notes |
|-------|----------|-------|-------|
| `Tool` | `src/tools/mod.rs` | Implemented for `escalate` tool | Extends existing tool ecosystem |
| `ApprovalResponse` | `src/agent/core.rs` | Escalation approval response | Reused unchanged |
| `ApprovalHandler` TUI | `src/tui/handlers/approval_handler.rs` | HITL approval workflow | Reused unchanged |

## Public API Surface (mod.rs)

```rust
// Data types
pub use types::{
    ExecutionTier, TaskComplexity, ComplexityLevel, 
    ComplexityMetrics, CascadeEvent, EventFilters,
};
pub use errors::CascadeError;

// Trait interfaces
pub use crate::tools::Tool;  // Reused for escalate tool
pub use crate::agent::ApprovalResponse;  // Reused for escalation approval

// New traits
pub trait ComplexityAnalyzer { ... }
pub trait CascadeRouter { ... }
pub trait CascadeEventLogger { ... }

// Builder for cascade initialization
pub struct CascadeSystemBuilder { ... }

impl CascadeSystemBuilder {
    pub fn new() -> Self { ... }
    pub fn with_complexity_analyzer(self, analyzer: Arc<dyn ComplexityAnalyzer>) -> Self { ... }
    pub fn with_router(self, router: Arc<dyn CascadeRouter>) -> Self { ... }
    pub fn with_event_logger(self, logger: Arc<dyn CascadeEventLogger>) -> Self { ... }
    pub fn build(self) -> Result<CascadeSystem> { ... }
}

// Cascade query interface
pub struct CascadeQueryService { ... }
impl CascadeQueryService {
    pub async fn get_cascade_history(&self, task_id: &str) -> Result<Vec<CascadeEvent>> { ... }
}
```

## Configuration & Initialization

### Cascade Config (in .hoosh.toml)

```toml
[cascades]
enabled = true
routing_policy = "multi-signal"
default_tier = "Medium"
escalation_needs_approval = true

[[cascades.model_tiers]]
tier = "Light"
models = ["claude-3-5-haiku-20241022"]

[[cascades.model_tiers]]
tier = "Medium"
models = ["claude-3-5-sonnet-20241022"]

[[cascades.model_tiers]]
tier = "Heavy"
models = ["claude-3-opus-20250219"]
```

### Bootstrap in Session/Agent Init

```rust
// In session.rs or agent initialization
if let Some(cascade_cfg) = config.cascade_config() {
    let analyzer = ComplexityAnalyzerImpl::new(cascade_cfg.routing_weights);
    let router = RouterImpl::new(&cascade_cfg.model_tiers);
    let logger = EventLoggerImpl::new("./cascade-events.jsonl")?;
    
    let cascade_system = CascadeSystemBuilder::new()
        .with_complexity_analyzer(Arc::new(analyzer))
        .with_router(Arc::new(router))
        .with_event_logger(Arc::new(logger))
        .build()?;
    
    // Register escalate tool with tool registry
    let escalate_tool = EscalateTool::new(cascade_system.clone());
    tool_registry.register(escalate_tool)?;
}
```

## Error Handling Contract

- `complexity::analyze()` → `anyhow::Result<TaskComplexity>`
- `router::route()` → `ExecutionTier` (infallible; always returns a tier)
- `approval::request_approval()` → `Result<ApprovalDecision>` (timeout or I/O error)
- `logger::emit()` → `Result<()>` (non-blocking; errors logged to stderr)

## Testing Strategy

1. **Unit Tests** (module-private): 
   - `complexity_tests.rs`: Test heuristics with synthetic inputs
   - `router_tests.rs`: Verify tier selection logic

2. **Integration Tests** (public API):
   - `integration_tests.rs`: Full cascade flow with mock LLM
   - Approval timeout scenarios
   - Escalation path validation

3. **Fixtures**:
   - `fixtures/tasks.json`: Sample tasks with expected complexity
   - `fixtures/config.toml`: Test configuration
