# Quickstart: Model Cascade Implementation

## Overview

Implementing model cascade system for hoosh involves 5 main components:

1. **Complexity Analyzer** - Measure task complexity from prompt
2. **Tier Router** - Map complexity to model tier
3. **Backend Factory** - Create tier-specific backends
4. **Escalate Tool** - Allow runtime tier escalation
5. **Integration Points** - Wire into existing task execution

## Phase 1 Implementation Path

### Step 1: Configuration Extension

Add to `config.toml`:

```toml
[cascade.light]
backend = "anthropic"
model = "claude-haiku-4.5"

[cascade.medium]
backend = "anthropic"
model = "claude-sonnet-4.5"

[cascade.heavy]
backend = "anthropic"
model = "claude-opus-4"
```

**Files to modify:**
- `src/config/mod.rs` - Add `CascadeConfig` struct
- `config.rs tests` - Validate tier loading

### Step 2: Core Data Structures

**File**: `src/model_cascade/mod.rs` (new)

```rust
pub mod complexity;        // TaskComplexity, ComplexityMetrics
pub mod tier;              // ModelTier, TierName enum
pub mod context;           // CascadeContext, EscalationStep
pub mod analyzer;          // analyze_task_complexity() function

pub use complexity::*;
pub use tier::*;
pub use context::*;
pub use analyzer::*;
```

### Step 3: Complexity Analyzer

**File**: `src/model_cascade/analyzer.rs`

Implement `analyze_task_complexity(prompt, depth, agent_type)`:

```rust
pub async fn analyze_task_complexity(
    task_prompt: &str,
    conversation_depth: usize,
    agent_type: Option<&str>,
) -> Result<TaskComplexity>
```

**Decision logic** (multi-signal heuristic):

```rust
// Collect signals
let structural_depth = parse_requirement_nesting(prompt);
let action_verbs = count_action_verbs(prompt);  // "design", "implement", "debug", etc.
let code_signal = analyze_code_blocks(prompt);

// Normalize to 0.0-1.0
let depth_score = normalize(structural_depth, 1.0, 5.0);
let action_score = normalize(action_verbs as f32, 0.0, 6.0);
let code_score = if has_code { estimate_cyclomatic_complexity(code) / 10.0 } else { 0.0 };

// Weighted combination
let complexity_score = (0.35 * depth_score) + (0.35 * action_score) + (0.30 * code_score);

// Route conservatively
if complexity_score < 0.35 && confidence > 0.80 {
    tier = Light
} else if complexity_score > 0.65 && confidence > 0.75 {
    tier = Heavy
} else {
    tier = Medium  // Safe default
}
```

**Metrics to collect**:
- Basic: `message_length`, `word_count`, `line_count`
- Signals: `code_blocks`, `has_multiple_questions`, `conversation_depth`
- Context: `agent_type`
- **New Multi-Signal**: `structural_depth`, `action_verb_count`, `unique_concepts`, `code_cyclomatic_complexity`

### Step 4: Backend Factory for Tiers

**File**: `src/backends/cascade_factory.rs` (new)

```rust
pub struct CascadeBackendFactory;

impl CascadeBackendFactory {
    pub async fn create_backend_for_tier(
        tier: &ModelTier,
        config: &AppConfig,
    ) -> Result<Arc<dyn LlmBackend>> {
        // Get base backend config
        let base = config.get_backend_config(&tier.backend_name)?;
        
        // Override model with tier-specific model
        let mut tier_config = base.clone();
        tier_config.model = Some(tier.model_id.clone());
        
        // Create backend using existing factory
        let backend = backends::create_backend(&tier.backend_name, &tier_config)?;
        Ok(Arc::new(backend))
    }
}
```

### Step 5: Escalate Tool

**File**: `src/tools/escalate_tool.rs` (new)

```rust
pub struct EscalateTool {
    cascade_context: Arc<RwLock<Option<CascadeContext>>>,
    backend_factory: Arc<CascadeBackendFactory>,
    config: Arc<AppConfig>,
}

#[async_trait]
impl Tool for EscalateTool {
    async fn execute(
        &self,
        args: &Value,
        context: &ToolExecutionContext,
    ) -> ToolResult<String> {
        // Parse request
        let request: EscalateToolRequest = serde_json::from_value(args.clone())?;
        
        // Validate request
        validate_escalate_request(&request)?;
        
        // Get current cascade context or create new
        let mut cascade = self.get_or_create_cascade_context();
        
        // Check escalation constraints
        if cascade.current_tier == TierName::Heavy {
            return Err("Already at maximum tier".into());
        }
        if cascade.escalation_path.len() >= 2 {
            return Err("Escalation limit reached".into());
        }
        
        // Determine next tier
        let next_tier = match cascade.current_tier {
            TierName::Light => TierName::Medium,
            TierName::Medium => TierName::Heavy,
            TierName::Heavy => unreachable!(),
        };
        
        // Record escalation step
        cascade.escalation_path.push(EscalationStep {
            timestamp: now(),
            from_tier: cascade.current_tier,
            to_tier: next_tier,
            reason: request.reason.clone(),
            model_name: get_model_for_tier(next_tier),
        });
        
        // Signal Agent to switch backend
        // (Agent will call create_backend_for_tier with next_tier)
        
        Ok(format!("Escalating to {:?} tier...", next_tier))
    }
    
    // Tool metadata
    fn name(&self) -> &'static str { "escalate" }
    fn description(&self) -> &'static str { 
        "Request escalation to a higher-tier model when current tier is insufficient"
    }
}
```

### Step 6: Integration with TaskManager

**File**: `src/task_management/task_manager.rs` (modify)

```rust
pub async fn execute_task(&self, task_def: TaskDefinition) -> Result<TaskResult> {
    // NEW: Analyze task complexity
    let complexity = model_cascade::analyze_task_complexity(
        &task_def.prompt,
        0,  // Initial task, no prior conversation
        None,
    ).await?;
    
    // NEW: Select tier
    let (tier, tier_backend) = self.select_tier_for_complexity(&complexity)?;
    
    // Use tier-specific backend instead of default
    let backend = tier_backend;
    
    // Rest of execution continues as before
    let agent = Agent::new(backend, self.tool_registry.clone(), executor);
    // ...
}

fn select_tier_for_complexity(
    &self,
    complexity: &TaskComplexity,
) -> Result<(TierName, Arc<dyn LlmBackend>)> {
    // Map complexity to tier
    let tier_name = match complexity.level {
        ComplexityLevel::Light => TierName::Light,
        ComplexityLevel::Medium => TierName::Medium,
        ComplexityLevel::Heavy => TierName::Heavy,
    };
    
    // Create tier-specific backend
    let tier_config = self.config.get_cascade_tier(&tier_name)?;
    let backend = CascadeBackendFactory::create_backend_for_tier(
        &tier_config,
        &self.config,
    ).await?;
    
    Ok((tier_name, backend))
}
```

### Step 7: Agent Backend Switching

**File**: `src/agent/core.rs` (modify handle_turn)

When escalate tool is executed:

```rust
// In Agent::handle_turn(), after tool execution:
if tool_name == "escalate" && tool_result.success {
    // Extract new tier from result
    let new_tier = extract_tier_from_result(&tool_result)?;
    
    // Create new backend
    let new_backend = CascadeBackendFactory::create_backend_for_tier(
        &new_tier,
        &self.config,
    ).await?;
    
    // Switch backend for next LLM call
    self.backend = new_backend;
    
    // Emit event
    self.emit_event(AgentEvent::EscalationApproved { ... });
}
```

### Step 8: Tool Registration

**File**: `src/tools/mod.rs` (modify)

```rust
pub fn create_tool_provider() -> ToolProvider {
    let mut provider = DefaultToolProvider::new();
    
    // Register escalate tool
    provider.register(Arc::new(EscalateTool::new(
        Arc::new(RwLock::new(None)),
        Arc::new(CascadeBackendFactory),
        config.clone(),
    )));
    
    provider
}
```

## Testing Strategy

### Unit Tests

**File**: `src/model_cascade/tests.rs`

```rust
#[test]
fn simple_task_selects_light_tier() {
    let prompt = "What is 2+2?";
    let complexity = analyze_task_complexity(prompt, 0, None);
    assert_eq!(complexity.level, ComplexityLevel::Light);
}

#[test]
fn complex_task_selects_heavy_tier() {
    let prompt = "Design a distributed consensus algorithm..."; // 2000+ chars
    let complexity = analyze_task_complexity(prompt, 0, None);
    assert_eq!(complexity.level, ComplexityLevel::Heavy);
}

#[test]
fn ambiguous_defaults_to_medium() {
    let prompt = "Moderate task"; // ~100-1500 chars
    let complexity = analyze_task_complexity(prompt, 0, None);
    assert_eq!(complexity.level, ComplexityLevel::Medium);
}
```

### Integration Tests

**File**: `tests/cascade_integration.rs`

```rust
#[tokio::test]
async fn escalation_preserves_conversation() {
    // Start with Light tier
    // Execute some steps
    // Call escalate tool
    // Verify all messages transferred to Medium tier
    // Verify model can reference prior messages
}

#[tokio::test]
async fn cannot_escalate_beyond_heavy() {
    // Start with Heavy tier
    // Call escalate tool
    // Verify error returned
}
```

## Validation Checklist

- [ ] Configuration loads tier definitions correctly
- [ ] Complexity analyzer produces reasonable classifications
- [ ] Backend factory creates tier-specific backends
- [ ] Escalate tool validates input parameters
- [ ] Escalate tool tracks escalation history
- [ ] Agent switches backend on escalation
- [ ] Conversation history 100% preserved
- [ ] Token counts accurate across escalations
- [ ] Error cases handled gracefully
- [ ] All tests pass
- [ ] No performance regression

## Debugging Tips

### Check Tier Selection

```rust
// Add to TaskManager::execute_task
eprintln!("Task complexity: {:?}", complexity);
eprintln!("Selected tier: {:?}", tier_name);
```

### Monitor Escalations

```
tail -f ~/.hoosh/cascade_history.jsonl
```

### Backend Verification

```toml
# Verify in config.toml
[cascade.light]
model = "claude-haiku-4.5"  # ✓ Should be fast/cheap

[cascade.medium]
model = "claude-sonnet-4.5" # ✓ Should be balanced

[cascade.heavy]
model = "claude-opus-4"     # ✓ Should be most capable
```

## Phase 1 vs Phase 2

**Phase 1 (Current)**:
- ✅ Single backend per cascade
- ✅ Heuristic-based routing
- ✅ Manual tier selection via config
- ✅ Escalate tool only
- ✅ Conservative Medium default

**Phase 2 (Future)**:
- Cross-backend escalation
- ML-based complexity analysis
- Automatic de-escalation
- Cost tracking and optimization
- Telemetry-driven learning
