# Phase 1 Data Model: Model Cascade System

**Status**: Design  
**Date**: 2025-12-10

## Core Entities

### 1. TaskComplexity

**Purpose**: Represents complexity level of incoming task with multi-signal analysis

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskComplexity {
    pub level: ComplexityLevel,           // Light, Medium, Heavy
    pub confidence: f32,                   // 0.0-1.0
    pub reasoning: String,                 // Why this level assigned
    pub metrics: ComplexityMetrics,        // Supporting data
    pub score: f32,                        // Normalized complexity score (0.0-1.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ComplexityLevel {
    Light,
    Medium,
    Heavy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityMetrics {
    // Raw signals
    pub message_length: usize,
    pub word_count: usize,
    pub line_count: usize,
    pub code_blocks: usize,
    pub has_multiple_questions: bool,
    pub conversation_depth: usize,        // Number of prior messages
    pub agent_type: Option<String>,       // "plan", "explore", "review"
    
    // Multi-signal metrics (NEW)
    pub structural_depth: usize,          // Nesting level of requirements (1-5)
    pub action_verb_count: usize,         // Count of action verbs (design, implement, etc)
    pub unique_concepts: usize,           // Unique domains mentioned
    pub code_cyclomatic_complexity: Option<usize>,  // CC of code blocks if present
    
    // Signal contributions to final score
    pub structural_depth_score: f32,      // 0.0-1.0 normalized
    pub action_density_score: f32,        // 0.0-1.0 normalized
    pub code_signal_score: f32,           // 0.0-1.0 normalized
}
```

**Validation Rules**:
- confidence must be 0.0-1.0
- score must be 0.0-1.0
- reasoning must be non-empty string
- metrics.structural_depth must be 1-5
- metrics.action_verb_count must be 0-20
- signal scores must sum approximately to score (within 0.05)

### 2. ModelTier

**Purpose**: Configuration of model capability tier

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelTier {
    pub tier_name: TierName,               // "light", "medium", "heavy"
    pub backend_name: String,              // "anthropic", "openai"
    pub model_id: String,                  // "claude-haiku-4.5"
    pub max_tokens: Option<usize>,         // Max context window for this tier
    pub priority: u8,                      // 1=light, 2=medium, 3=heavy
}

#[derive(Debug, Clone, Copy, PartialEq, Ord, PartialOrd, Eq, Serialize, Deserialize)]
pub enum TierName {
    Light = 1,
    Medium = 2,
    Heavy = 3,
}
```

**Validation Rules**:
- tier_name must have consistent priority value
- backend_name must be registered in config
- model_id must be valid for backend
- Each backend must have exactly 3 tiers (Light, Medium, Heavy)

### 3. CascadeContext

**Purpose**: Maintain state during escalation chain

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeContext {
    pub cascade_id: String,                // UUID for this escalation chain
    pub original_task: String,             // Initial task description
    pub started_at: u64,                   // Unix timestamp
    pub current_tier: TierName,            // Active tier
    pub escalation_path: Vec<EscalationStep>,  // History of escalations
    pub conversation_id: String,           // Preserved across escalations
    pub total_token_usage: TokenUsage,     // Cumulative across all tiers
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationStep {
    pub timestamp: u64,
    pub from_tier: TierName,
    pub to_tier: TierName,
    pub reason: String,
    pub model_name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
}
```

**Validation Rules**:
- cascade_id must be non-empty UUID string
- escalation_path must be monotonically increasing (Light → Medium or Medium → Heavy only)
- total_token_usage input/output must be non-negative
- Each step must have timestamp >= previous step

### 4. ComplexityAnalysisResult

**Purpose**: Decision result from analyzer

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityAnalysisResult {
    pub task_complexity: TaskComplexity,
    pub recommended_tier: TierName,        // Initial recommendation
    pub confidence: f32,
    pub analysis_time_ms: u64,
}
```

### 5. EscalateToolRequest

**Purpose**: Arguments for escalate tool

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalateToolRequest {
    pub reason: String,                    // Why escalation is needed
    pub context_summary: Option<String>,   // Optional context for higher tier
    pub preserve_history: bool,            // Always true in Phase 1
}
```

**Validation Rules**:
- reason must be non-empty
- reason must be < 1000 chars (prevent abuse)
- preserve_history must always be true

### 6. EscalationResult

**Purpose**: Result of escalation attempt

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationResult {
    pub success: bool,
    pub escalated_to: Option<TierName>,
    pub error: Option<String>,
    pub context_preserved: bool,
    pub message_count_transferred: usize,
}
```

## State Transitions

### Complexity → Tier Routing

```
Input: TaskComplexity
  ├─ Length 0-100 chars → Light (confidence 0.9)
  ├─ Length 100-1500 chars
  │  ├─ No code blocks → Medium (confidence 0.7)
  │  └─ 1+ code blocks → Medium (confidence 0.8)
  ├─ Length >1500 chars → Heavy (confidence 0.9)
  └─ Ambiguous → Medium (confidence 0.5) [DEFAULT]
Output: ModelTier
```

### Escalation State Machine

```
                 ┌─────────────┐
                 │   Light     │
                 │  (tier: 1)  │
                 └──────┬──────┘
                        │ escalate()
                        ▼
                 ┌─────────────┐
                 │   Medium    │
                 │  (tier: 2)  │
                 └──────┬──────┘
                        │ escalate()
                        ▼
                 ┌─────────────┐
                 │    Heavy    │
                 │  (tier: 3)  │
                 └─────────────┘
                        │
                  escalate() = Error
                  (max tier)
```

### Escalation Data Flow

```
1. Model detects issue → calls escalate() tool
2. Escalate validator checks: reason + tier constraints
3. CascadeContext updated with EscalationStep
4. Backend switched to next tier
5. Conversation history loaded (100% preserved)
6. Model continues with full context
7. Result attributed to escalated tier
```

## Validation Rules

### Cross-Entity Constraints

1. **Tier Uniqueness**: Only one tier per (backend, TierName) combination
2. **Escalation Linearity**: Never escalate down or laterally (Light → Heavy forbidden)
3. **Context Consistency**: CascadeContext.conversation_id must match active Conversation
4. **Token Tracking**: total_token_usage must equal sum of all EscalationSteps
5. **Reason Audit Trail**: Every escalation must have documented reason
6. **Conversation Integrity**: No messages lost during escalation (count verified)

### Phase 1 Specific Constraints

1. Single backend per cascade (no cross-backend escalation)
2. At most 3 escalations per task (Light → Medium → Heavy only)
3. Escalation must increase tier strictly (not decrease)
4. Maximum 3 active cascades per session
5. Cascade timeout = 30 minutes (no indefinite escalation chains)

## Storage & Persistence

### In-Memory
- Active CascadeContext in Arc<RwLock<>>
- ModelTier definitions cached from config
- ComplexityAnalysisResult transient (used once)

### On-Disk
- Model tier definitions: config.toml [cascade.{tier}] section
- Cascade history: ~/.hoosh/cascade_history.jsonl (one line per cascade)
- Conversation messages: existing ~/.hoosh/conversations/{id}/messages.jsonl (unchanged)

### Serialization Format

All entities implement Serialize/Deserialize (serde) for:
- JSON for API contracts
- TOML for config
- JSONL for audit logs
