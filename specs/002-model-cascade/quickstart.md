# Quickstart: Model Cascade Implementation

**Phase**: 1 | **Date**: 2025-12-10

---

## Overview

This guide walks through integrating the cascade module into Hoosh's task execution loop. The cascade system routes tasks to appropriate model tiers (Light/Medium/Heavy) based on multi-signal complexity analysis, with human approval required for escalations.

---

## Module Structure

```
src/cascades/
├── mod.rs                  # Public API exports
├── complexity_analyzer.rs  # Multi-signal analysis
├── router.rs               # Tier routing logic
├── context.rs              # CascadeContext lifecycle
├── approval_handler.rs     # HITL approval workflow
├── events.rs               # Event logging & queries
├── errors.rs               # Error types
└── tests.rs                # Integration tests
```

---

## Configuration (TOML)

Add to `.hoosh.toml`:

```toml
[cascades]
enabled = true
routing_policy = "multi-signal"
escalation_policy = "allow_all"
default_tier = "Medium"
escalation_needs_approval = true

[[cascades.model_tiers]]
tier = "Light"
backend = "anthropic"
models = ["claude-3-5-haiku-20241022"]
max_cost_per_request_cents = 2

[[cascades.model_tiers]]
tier = "Medium"
backend = "anthropic"
models = ["claude-3-5-sonnet-20241022"]
max_cost_per_request_cents = 5

[[cascades.model_tiers]]
tier = "Heavy"
backend = "anthropic"
models = ["claude-3-opus-20250219"]
max_cost_per_request_cents = 15
```

---

## Integration: Task Execution Loop

**File**: `src/agent/core.rs`

```rust
use cascades::{
    ComplexityAnalyzer, Router, CascadeContext, 
    EventLogger, ApprovalHandler
};

pub async fn execute_task(
    task: &TaskDefinition,
    config: &Config,
) -> Result<TaskResult> {
    // 1. Load cascade config (if enabled)
    let cascade_cfg = config.cascade_config();
    if !cascade_cfg.enabled {
        // Standard execution (no cascade)
        return execute_standard(task).await;
    }

    // 2. Analyze complexity
    let analyzer = ComplexityAnalyzer::new();
    let complexity = analyzer.analyze(&task.description)?;
    
    // 3. Route to initial tier
    let router = Router::new(&cascade_cfg);
    let initial_tier = router.route(&complexity);
    
    // 4. Create cascade context
    let mut cascade_ctx = CascadeContext::new(
        task.id.clone(),
        initial_tier.clone(),
        task.description.clone(),
    );

    // 5. Execute task with escalation support
    loop {
        let result = execute_on_tier(&mut cascade_ctx, &cascade_cfg).await;
        
        match result {
            Ok(output) => {
                log_event(
                    &cascade_ctx,
                    CascadeEventType::TaskCompleted,
                    &output,
                )?;
                return Ok(TaskResult::success(output));
            }
            Err(NeedsEscalation(reason)) => {
                // 6. Request escalation (with HITL approval)
                if cascade_cfg.escalation_needs_approval {
                    let approval = request_approval(
                        &cascade_ctx,
                        &reason,
                    ).await?;
                    
                    if !approval.approved {
                        log_event(
                            &cascade_ctx,
                            CascadeEventType::EscalationRejected,
                            &approval.rejection_reason,
                        )?;
                        return Err(approval.rejection_reason.into());
                    }
                    cascade_ctx.approve_escalation(&approval);
                }
                
                // 7. Move to next tier and retry
                let next_tier = cascade_ctx.escalate()?;
                log_event(
                    &cascade_ctx,
                    CascadeEventType::EscalationExecuted,
                    &format!("Escalated to {:?}", next_tier),
                )?;
            }
            Err(e) => {
                log_event(
                    &cascade_ctx,
                    CascadeEventType::TaskFailed,
                    &e.to_string(),
                )?;
                return Err(e);
            }
        }
    }
}
```

---

## Complexity Analyzer

**File**: `src/cascades/complexity_analyzer.rs`

```rust
pub struct ComplexityAnalyzer;

impl ComplexityAnalyzer {
    pub fn analyze(&self, text: &str) -> Result<TaskComplexity> {
        let depth_score = self.structural_depth(text);
        let action_score = self.action_density(text);
        let code_score = self.code_signals(text);
        
        let composite = (0.35 * depth_score) 
                      + (0.35 * action_score)
                      + (0.30 * code_score);
        
        let level = match composite {
            s if s < 0.35 => ComplexityLevel::Light,
            s if s > 0.65 => ComplexityLevel::Heavy,
            _ => ComplexityLevel::Medium,
        };
        
        // Confidence based on signal agreement
        let agreement = 1.0 - ((depth_score - action_score).abs() 
                            + (action_score - code_score).abs()) / 2.0;
        
        let confidence = if level == ComplexityLevel::Medium {
            agreement * 0.75  // Ambiguous tier
        } else if agreement > 0.8 {
            agreement * 0.95  // Strong consensus
        } else {
            agreement * 0.75  // Mixed signals
        };
        
        Ok(TaskComplexity {
            level,
            confidence: confidence.min(1.0),
            reasoning: format!(
                "Depth: {:.2}, Action: {:.2}, Code: {:.2} → {}",
                depth_score, action_score, code_score, level
            ),
            metrics: ComplexityMetrics { /* ... */ },
        })
    }
    
    fn structural_depth(&self, text: &str) -> f32 {
        // Count conditional depth
        let depth = count_nesting_levels(text);
        normalize(depth as f32, 1.0, 5.0)
    }
    
    fn action_density(&self, text: &str) -> f32 {
        let re = regex::Regex::new(
            r"\b(implement|design|analyze|refactor|...)\b"
        ).unwrap();
        let count = re.find_iter(&text.to_lowercase()).count();
        normalize(count as f32, 0.0, 6.0)
    }
    
    fn code_signals(&self, text: &str) -> f32 {
        let has_code = text.contains("```");
        if !has_code { return 0.0; }
        
        let cc = cyclomatic_complexity(text);
        match cc {
            c if c < 3 => 0.3,
            c if c < 5 => 0.6,
            _ => 1.0,
        }
    }
}
```

---

## Router

**File**: `src/cascades/router.rs`

```rust
pub struct Router<'a> {
    config: &'a CascadeConfig,
}

impl<'a> Router<'a> {
    pub fn route(&self, complexity: &TaskComplexity) -> ExecutionTier {
        if complexity.confidence < 0.7
            || (complexity.confidence < 0.8 
                && complexity.level == ComplexityLevel::Medium) {
            // Conservative default
            return self.config.default_tier.clone();
        }
        
        match complexity.level {
            ComplexityLevel::Light => ExecutionTier::Light,
            ComplexityLevel::Medium => ExecutionTier::Medium,
            ComplexityLevel::Heavy => ExecutionTier::Heavy,
        }
    }
}
```

---

## Approval Handler (HITL)

**File**: `src/cascades/approval_handler.rs`

```rust
pub async fn request_approval(
    cascade_ctx: &CascadeContext,
    reason: &str,
) -> Result<ApprovalDecision> {
    // Show TUI dialog to operator
    let dialog = EscalationApprovalDialog::new(
        &cascade_ctx.original_task,
        &cascade_ctx.current_tier,
        &cascade_ctx.escalation_path.last(),
        reason,
        &cascade_ctx.conversation_history,
    );
    
    match dialog.show_with_timeout(Duration::from_secs(300)).await {
        Ok(decision) => {
            log_audit(&cascade_ctx, &decision)?;
            Ok(decision)
        }
        Err(TimeoutError) => {
            Err("Operator approval timeout".into())
        }
    }
}
```

---

## Event Logging

**File**: `src/cascades/events.rs`

```rust
pub struct EventLogger {
    events: Arc<DashMap<String, Vec<CascadeEvent>>>,
    // indices for queryability
}

impl EventLogger {
    pub fn emit(&self, event: CascadeEvent) {
        self.events
            .entry(event.task_id.clone())
            .or_insert_with(Vec::new)
            .push(event);
    }
    
    pub fn query_by_task(&self, task_id: &str) -> Vec<CascadeEvent> {
        self.events.get(task_id)
            .map(|e| e.clone())
            .unwrap_or_default()
    }
}
```

---

## Testing Integration

**File**: `src/cascades/tests.rs`

```rust
#[tokio::test]
async fn routes_simple_task_to_light_tier() {
    let analyzer = ComplexityAnalyzer::new();
    let complexity = analyzer.analyze("What is 2+2?").unwrap();
    
    assert_eq!(complexity.level, ComplexityLevel::Light);
    assert!(complexity.confidence > 0.8);
}

#[tokio::test]
async fn preserves_conversation_during_escalation() {
    let mut ctx = CascadeContext::new(
        "task1".to_string(),
        ExecutionTier::Light,
        "complex task".to_string(),
    );
    
    ctx.conversation_history.push(ChatMessage { /* ... */ });
    ctx.escalate().unwrap();
    
    assert_eq!(ctx.conversation_history.len(), 1);
    assert_eq!(ctx.escalation_path, vec![Light, Medium]);
}
```

---

## Module Exports

**File**: `src/cascades/mod.rs`

```rust
pub mod complexity_analyzer;
pub mod router;
pub mod context;
pub mod approval_handler;
pub mod events;
pub mod errors;

pub use complexity_analyzer::ComplexityAnalyzer;
pub use router::Router;
pub use context::CascadeContext;
pub use approval_handler::request_approval;
pub use events::{EventLogger, CascadeEvent};
pub use errors::{CascadeError, NeedsEscalation};
```

---

## Checklist

- [ ] Add `cascades/` module to `src/`
- [ ] Update `src/lib.rs` to export cascades module
- [ ] Modify `agent/core.rs` to integrate execute_task loop
- [ ] Update config parser to handle `[cascades]` section
- [ ] Register `escalate` tool in `tools/mod.rs`
- [ ] Extend TUI approval dialog in `src/tui/`
- [ ] Add tests in `src/cascades/tests.rs`
- [ ] Update `Cargo.toml` with `dashmap`, `uuid` dependencies
- [ ] Run `cargo test` and `cargo clippy`

---

**Next**: Run `/speckit.tasks` to generate task breakdown
