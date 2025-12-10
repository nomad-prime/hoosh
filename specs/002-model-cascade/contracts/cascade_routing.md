# Contract: Cascade Routing System

## Overview

The cascade routing system is invoked at task initialization to automatically select appropriate model tier based on task complexity.

## Input Contract: Analyze Task

**When**: Before agent initialization  
**Who calls**: TaskManager::execute_task()  
**What**: Analyzes incoming task prompt for complexity signals

### Request

```rust
pub async fn analyze_task_complexity(
    task_prompt: &str,
    conversation_depth: usize,
    agent_type: Option<&str>,
) -> Result<TaskComplexity>
```

### Parameters

| Parameter | Type | Required | Constraints |
|-----------|------|----------|-------------|
| task_prompt | &str | Yes | Non-empty, < 50,000 chars |
| conversation_depth | usize | Yes | 0-1000 (message count in prior conversation) |
| agent_type | Option<&str> | No | "plan", "explore", or "review" if provided |

### Response (Success)

```json
{
  "level": "medium",
  "confidence": 0.75,
  "reasoning": "Moderate length prompt (850 chars) with 1 code block suggests analysis task",
  "metrics": {
    "message_length": 850,
    "word_count": 140,
    "line_count": 8,
    "code_blocks": 1,
    "has_multiple_questions": true,
    "conversation_depth": 3,
    "agent_type": "plan"
  }
}
```

### Response (Error)

```json
{
  "error": "Task prompt empty",
  "code": "INVALID_INPUT"
}
```

---

## Output Contract: Tier Selection

**When**: After complexity analysis  
**Who receives**: Agent initialization  
**What**: Returns tier recommendation and corresponding backend config

### Request

```rust
pub fn select_tier_for_complexity(
    complexity: &TaskComplexity,
    config: &AppConfig,
) -> Result<(TierName, Arc<dyn LlmBackend>)>
```

### Parameters

| Parameter | Type | Constraints |
|-----------|------|------------|
| complexity | TaskComplexity | All fields populated, confidence > 0.0 |
| config | AppConfig | Must have [cascade.*] sections for all 3 tiers |

### Response (Success)

```json
{
  "tier": "medium",
  "backend_name": "anthropic",
  "model_id": "claude-sonnet-4.5",
  "max_tokens": 200000,
  "priority": 2
}
```

### Response (Error)

```json
{
  "error": "Backend 'anthropic' not configured",
  "code": "CONFIG_ERROR",
  "resolution": "Add [backends.anthropic] to config.toml"
}
```

---

## Routing Decision Table (Phase 1) - Multi-Signal

### Primary Decision Logic

```
complexity_score = (0.35 * structural_depth) + (0.35 * action_density) + (0.30 * code_signals)

if score < 0.35 AND confidence > 0.80:
    tier = Light
elif score > 0.65 AND confidence > 0.75:
    tier = Heavy
else:
    tier = Medium  # Conservative default
```

### Example Routing Decisions

| Task | Depth | Verbs | Code | Score | Confidence | Routing | Notes |
|------|-------|-------|------|-------|------------|---------|-------|
| "What is Docker?" | 1 | 0 | No | 0.15 | 0.95 | **Light** | Clear simple query |
| "Fix typo on line 5" | 1 | 1 | No | 0.20 | 0.90 | **Light** | Simple change |
| "Add OAuth to auth flow" | 2 | 1 | Maybe | 0.40 | 0.65 | **Medium** | Moderate, ambiguous |
| "Design, implement, test cache" | 2 | 3 | Yes | 0.50 | 0.70 | **Medium** | Multiple actions |
| "Build distributed consensus with Byzantine tolerance" | 3+ | 2+ | Yes | 0.75 | 0.85 | **Heavy** | High complexity |
| Ambiguous mid-range | 2 | 2 | No | 0.45 | 0.55 | **Medium** | Default for doubt |

**Confidence Thresholds**:
- Route confidently only at extremes (score 0.2 or 0.8+)
- Default to Medium for score 0.35-0.65 (ambiguous zone)
- Escalate tool catches routing errors automatically

---

## Integration Points

### TaskManager

```rust
// BEFORE: No routing
let agent = Agent::new(backend, tools, executor);

// AFTER: With routing
let complexity = analyze_task_complexity(&task.prompt)?;
let (tier, tier_backend) = select_tier_for_complexity(&complexity)?;
let agent = Agent::new(tier_backend, tools, executor);
```

### Configuration Extension

```toml
# config.toml additions for Phase 1

[cascade.light]
backend = "anthropic"
model = "claude-haiku-4.5"
max_tokens = 100000

[cascade.medium]
backend = "anthropic"
model = "claude-sonnet-4.5"
max_tokens = 200000

[cascade.heavy]
backend = "anthropic"
model = "claude-opus-4"
max_tokens = 200000
```

---

## Error Handling

| Error | Cause | Action |
|-------|-------|--------|
| `INVALID_INPUT` | Prompt empty or > 50KB | Reject task with message |
| `CONFIG_ERROR` | Tier not configured | Fall back to default backend |
| `ROUTING_FAILED` | Analysis panicked | Default to Medium tier |
| `BACKEND_UNAVAILABLE` | Backend unreachable | Error and ask user to check config |

---

## Observability

### Events Emitted

```rust
pub enum AgentEvent {
    // NEW: Routing decision
    RoutingDecision {
        complexity_level: ComplexityLevel,
        selected_tier: TierName,
        confidence: f32,
    },
    // Existing events still emit
    StepStarted { step },
    // ...
}
```

### Metrics Tracked

- Routing decision time (ms)
- Complexity distribution (Light/Medium/Heavy %)
- Tier utilization per session
- Escalation rate by initial tier
- Task success rate by tier
