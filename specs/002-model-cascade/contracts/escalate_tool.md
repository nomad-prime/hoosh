# Contract: Escalate Tool

## Overview

The escalate tool allows models to request upgrade to a higher-tier model when the current tier is insufficient. Available to all executing models (Light, Medium, Heavy) with fallback behavior at maximum tier.

## Tool Definition

### Metadata

| Field | Value |
|-------|-------|
| Tool Name | `escalate` |
| Availability | All agent types (Plan, Explore, Review) |
| Availability | All model tiers (Light, Medium, Heavy) |
| Maximum Calls | 2 per task (prevent escalation loops) |
| Requires Approval | No (trusted operation) |
| Requires Permission | Yes (cascading task execution) |

### Tool Signature

```rust
pub async fn escalate(request: EscalateToolRequest) -> Result<EscalateToolResponse>
```

---

## Input Contract

### Request Schema

```json
{
  "type": "object",
  "properties": {
    "reason": {
      "type": "string",
      "description": "Explanation of why escalation is needed (e.g., 'Task requires deeper analysis than current tier supports', 'Encountered complex edge case')",
      "minLength": 10,
      "maxLength": 1000
    },
    "context_summary": {
      "type": "string",
      "description": "Optional: Brief summary of work completed so far to guide the escalated model",
      "maxLength": 500
    },
    "preserve_history": {
      "type": "boolean",
      "description": "Always true. Indicates full conversation history should be preserved (Phase 1 requirement)"
    }
  },
  "required": ["reason"],
  "additionalProperties": false
}
```

### Example Request

```json
{
  "reason": "Task involves implementing a distributed consensus algorithm which requires deep reasoning about edge cases and formal verification concepts. Current tier unable to reason about Byzantine fault tolerance guarantees.",
  "context_summary": "Analyzed 3 existing implementations, identified 2 critical issues in concurrent state management",
  "preserve_history": true
}
```

### Validation Rules

1. `reason` must be non-empty and at least 10 characters
2. `reason` must be <= 1000 characters (prevent token waste)
3. `context_summary` if provided must be <= 500 characters
4. `preserve_history` must be true (no option to discard history)

---

## Output Contract

### Success Response

```json
{
  "success": true,
  "escalated_to": "heavy",
  "escalated_from": "medium",
  "model_name": "claude-opus-4",
  "context_preserved": true,
  "message_count_transferred": 42,
  "note": "Your conversation history has been preserved. Continue with the escalated model."
}
```

### Fields

| Field | Type | Meaning |
|-------|------|---------|
| success | bool | true if escalation succeeded |
| escalated_to | string | Tier moved to (light/medium/heavy) |
| escalated_from | string | Tier moved from |
| model_name | string | Model ID of escalated backend |
| context_preserved | bool | Whether full history was kept (always true Phase 1) |
| message_count_transferred | usize | Number of messages preserved |
| note | string | Human-readable status |

### Error Response - Already at Maximum Tier

```json
{
  "success": false,
  "error": "Cannot escalate: already at maximum tier (heavy)",
  "code": "AT_MAXIMUM_TIER",
  "suggestion": "Consider rephrasing the problem or breaking into smaller tasks"
}
```

### Error Response - Too Many Escalations

```json
{
  "success": false,
  "error": "Escalation limit reached (max 2 per task)",
  "code": "ESCALATION_LIMIT_EXCEEDED",
  "suggestion": "This problem may need to be decomposed into smaller tasks"
}
```

### Error Response - Invalid Reason

```json
{
  "success": false,
  "error": "Reason too short (minimum 10 chars)",
  "code": "INVALID_REASON",
  "suggestion": "Please provide a detailed explanation of why escalation is needed"
}
```

---

## Execution Flow

```
1. Model calls escalate tool with reason
   ↓
2. ToolExecutor validates request schema
   ├─ Invalid → Return error (don't escalate)
   └─ Valid → Continue
   ↓
3. Check escalation constraints
   ├─ Already at Heavy → Return "AT_MAXIMUM_TIER" error
   ├─ Escalations > 2 → Return "ESCALATION_LIMIT_EXCEEDED" error
   └─ Valid escalation → Continue
   ↓
4. Load current CascadeContext
   ├─ Not found → Create new context
   └─ Found → Update existing
   ↓
5. Select next tier (Light→Medium or Medium→Heavy)
   ↓
6. Record EscalationStep
   ├─ from_tier: current tier
   ├─ to_tier: next tier
   ├─ reason: from request
   ├─ timestamp: now
   └─ model_name: next tier's model
   ↓
7. Serialize and preserve conversation
   ├─ Get all messages from current Conversation
   ├─ Verify message count
   └─ No messages dropped
   ↓
8. Create new backend instance for escalated tier
   ├─ Use config tier definition
   ├─ Same API key / auth as parent
   └─ Prepare for model switch
   ↓
9. Return success response
   ├─ Include messages transferred count
   ├─ Include new model name
   └─ Trigger Agent to switch backend
   ↓
10. Agent::handle_turn() continues with new backend
    └─ Next LLM call uses escalated model
    └─ All prior messages in conversation
```

---

## Conversation History Preservation

### Guarantee

**100% of messages are preserved during escalation.**

- No content loss
- No message reordering
- All tool calls and results retained
- Full token count transferred with messages

### Mechanism

```rust
// Before escalation
Conversation {
  messages: vec![
    {"role": "user", "content": "..."},
    {"role": "assistant", "content": "..."},
    {"role": "tool", "tool_call_id": "...", "content": "..."},
    // ... more messages
  ]
}

// After escalation - IDENTICAL Conversation
// Escalated model sees all prior context
// Continues from current state
// Can reference prior reasoning
```

### Token Counting Across Tiers

```
Light tier result:
  - Input tokens: 500
  - Output tokens: 200

Escalation to Medium tier:
  - Previous messages (Light): 700 tokens
  - Escalation request: 50 tokens
  - Total context sent to Medium: 750 tokens
  - Medium response: 300 tokens

Total CascadeContext.total_token_usage:
  - input: 500 + 750 = 1250
  - output: 200 + 300 = 500
```

---

## Constraints & Limits

| Constraint | Value | Rationale |
|-----------|-------|-----------|
| Max escalations per task | 2 | Prevent infinite loops; use Phase 2 decomposition for harder tasks |
| Reason min length | 10 chars | Prevent vague escalations; force thoughtful decisions |
| Reason max length | 1000 chars | Prevent token waste on explanation |
| Context summary max | 500 chars | Concise summary for escalated model |
| Escalation rate limit | 1 per 30 seconds | Prevent rapid re-escalation |
| Task timeout | 30 minutes | Hard limit on cascade execution time |

---

## Monitoring & Logging

### Events Emitted

```rust
pub enum AgentEvent {
    EscalationRequested {
        reason: String,
        from_tier: TierName,
        to_tier: TierName,
    },
    EscalationApproved {
        cascade_id: String,
        model_name: String,
        messages_preserved: usize,
    },
    EscalationDenied {
        reason: String,
        code: String,
    },
}
```

### Audit Trail

All escalations logged to: `~/.hoosh/cascade_history.jsonl`

```json
{
  "cascade_id": "550e8400-e29b-41d4-a716-446655440000",
  "timestamp": 1702220400,
  "from_tier": "light",
  "to_tier": "medium",
  "reason": "Task requires deeper analysis...",
  "initial_task_length": 1200,
  "escalation_step": 1,
  "model_from": "claude-haiku-4.5",
  "model_to": "claude-sonnet-4.5",
  "messages_preserved": 42,
  "session_id": "user-session-123"
}
```

---

## Error Recovery

### Model Escalates While Already Executing at Maximum Tier

**Current**: Return error with suggestion  
**Result**: Model receives error, can handle or wrap up  
**Phase 2**: May support task decomposition or external retry

### Escalation During Tool Call

**Sequence**:
1. Model makes tool call (e.g., bash)
2. Tool completes
3. Model receives result, decides to escalate
4. Escalation executes (tool result preserved in history)
5. Escalated model sees tool result in context

### Network Failure During Escalation

**Behavior**:
- If backend switch fails → Return error (don't escalate)
- Current context remains with current tier
- Model can retry escalate or continue with current tier
- No partial escalation state
