# Context Management Strategies Analysis

## Overview
The codebase implements context management through three main strategies:
1. **Tool Output Truncation Strategy** - Truncates long tool outputs
2. **Sliding Window Strategy** - Maintains a window of recent messages
3. **Token Accountant** - Tracks token usage and calculates token pressure

---

## Issues and Gaps Found

### 1. ⚠️ **CRITICAL: Strategies Applied in Wrong Order**

**Location:** `src/session.rs` lines 230-241

```rust
if let Some(truncation_config) = context_manager_config.tool_output_truncation {
    let truncation_strategy = ToolOutputTruncationStrategy::new(truncation_config);
    context_manager_builder =
        context_manager_builder.add_strategy(Box::new(truncation_strategy));
}

if let Some(sliding_window_config) = context_manager_config.sliding_window {
    let sliding_window_strategy = SlidingWindowStrategy::new(sliding_window_config);
    context_manager_builder =
        context_manager_builder.add_strategy(Box::new(sliding_window_strategy));
}
```

**Issue:** Tool output truncation is applied BEFORE sliding window.

**Why it's wrong:**
- ToolOutputTruncationStrategy should run FIRST (truncate individual outputs)
- SlidingWindowStrategy should run SECOND (reduce message count after truncation)
- Current order means we truncate after potentially already removing context via windowing
- This can lead to premature message removal instead of truncating content

**Recommended Fix:**
```rust
// Apply sliding window FIRST to reduce message count
if let Some(sliding_window_config) = context_manager_config.sliding_window {
    let sliding_window_strategy = SlidingWindowStrategy::new(sliding_window_config);
    context_manager_builder =
        context_manager_builder.add_strategy(Box::new(sliding_window_strategy));
}

// Apply truncation SECOND to reduce individual message sizes
if let Some(truncation_config) = context_manager_config.tool_output_truncation {
    let truncation_strategy = ToolOutputTruncationStrategy::new(truncation_config);
    context_manager_builder =
        context_manager_builder.add_strategy(Box::new(truncation_strategy));
}
```

---

### 2. ⚠️ **CRITICAL: Token Pressure Check Timing Issue**

**Location:** `src/agent/core.rs` lines 118-127

```rust
async fn apply_context_compression(
    &self,
    conversation: &mut Conversation,
    context_manager: &ContextManager,
) -> Result<()> {
    let current_pressure = context_manager.get_token_pressure();

    if context_manager.should_warn_about_pressure() {
        self.send_event(AgentEvent::TokenPressureWarning { ... });
    }

    context_manager.apply_strategies(conversation).await?;
    Ok(())
}
```

**Issue:** Token pressure is calculated BEFORE applying strategies.

**Why it's wrong:**
- The pressure calculation is based on CURRENT token usage (from TokenAccountant)
- But strategies haven't been applied yet, so the warning reflects pre-compression state
- User sees warning, but context hasn't actually been compressed yet in that calculation
- Better to recalculate after applying strategies to show actual pressure reduction

**Recommended Fix:**
```rust
async fn apply_context_compression(
    &self,
    conversation: &mut Conversation,
    context_manager: &ContextManager,
) -> Result<()> {
    let pressure_before = context_manager.get_token_pressure();
    
    context_manager.apply_strategies(conversation).await?;
    
    let pressure_after = context_manager.get_token_pressure();
    
    if pressure_after > context_manager.config.warning_threshold {
        self.send_event(AgentEvent::TokenPressureWarning {
            current_pressure: pressure_after,
            threshold: context_manager.config.warning_threshold,
        });
    }
    
    Ok(())
}
```

---

### 3. 🔴 **MAJOR: Sliding Window Doesn't Account for Token Count**

**Location:** `src/context_management/sliding_window_strategy.rs`

**Issue:** The sliding window strategy uses MESSAGE COUNT as the constraint, not TOKEN COUNT.

**Problems:**
- Config has `window_size: 40` (messages) but ContextManager has `max_tokens: 128_000`
- If recent messages are very large (e.g., long tool outputs), a 40-message window could exceed token limit
- The token-based truncation happens in a separate strategy, but:
  1. If truncation is applied first, window still removes messages
  2. If window is applied first, truncation doesn't help with message count pressure
- Token pressure calculation in ContextManager looks at combined token count, not message count

**Recommendation:**
- Consider implementing a token-aware sliding window that removes messages until token target is met
- Or clarify that sliding window is a backup strategy when truncation isn't enough
- Document the interaction between window_size (messages) and max_tokens

---

### 4. 🟡 **MODERATE: Tool Output Truncation Preserves Last Result Unconditionally**

**Location:** `src/context_management/tool_output_truncation_strategy.rs` lines 88-91

```rust
let last_tool_result_index = conversation
    .messages
    .iter()
    .enumerate()
    .rev()
    .find(|(_, msg)| self.is_tool_result(msg))
    .map(|(i, _)| i);

for i in 0..message_count {
    if Some(i) == last_tool_result_index {
        continue;  // ← Always skips last tool result, no matter how large
    }
    // ... truncation logic
}
```

**Issue:** The last tool result is NEVER truncated, regardless of size.

**Why it matters:**
- If the last tool result is 10MB of output, it won't be truncated
- This could cause the token pressure to remain high
- The comment says "keeps_last_tool_result_full" which seems intentional, but undocumented
- Should have a size threshold or a flag to allow truncating even the last result if exceeding max

**Recommendation:**
```rust
// Add a configuration option:
pub struct ToolOutputTruncationConfig {
    pub max_length: usize,
    pub show_truncation_notice: bool,
    pub smart_truncate: bool,
    pub head_length: usize,
    pub tail_length: usize,
    pub preserve_last_result: bool,  // ← NEW
}

// Then use it:
if Some(i) == last_tool_result_index && self.config.preserve_last_result {
    continue;
}
```

---

### 5. 🟡 **MODERATE: No Summary-Based Compression Strategy**

**Location:** `src/context_management/summarizer.rs`

**Issue:** MessageSummarizer exists but is never used in the context management pipeline.

**Current State:**
- Summarizer is initialized in `session.rs` but passed to ConversationState
- Never called from ContextManager or core.rs
- No strategy implementation for summary-based compression

**Why it matters:**
- When token pressure is high, conversation could be summarized instead of just truncated
- More intelligent than windowing or truncation
- But currently unused

**Recommendation:**
- Create a `SummaryCompressionStrategy` that wraps MessageSummarizer
- Call it when token pressure exceeds a threshold (e.g., 85%)
- Replace old messages with their summaries

---

### 6. 🟡 **MODERATE: No Interaction Awareness Between Strategies**

**Location:** `src/context_management/context_manager.rs`

**Issue:** Strategies are applied sequentially without interaction or feedback.

**Current Implementation:**
```rust
pub async fn apply_strategies(&self, conversation: &mut Conversation) -> Result<()> {
    for strategy in &self.strategies {
        strategy.apply(conversation).await?;
    }
    Ok(())
}
```

**Problems:**
- Each strategy modifies the conversation independently
- No coordination between strategies
- No way for strategies to know results of previous strategies
- Can't stop early if target is reached

**Example scenario:**
- Sliding window removes to 40 messages (still 100K tokens)
- Then truncation runs on all 40 messages (reduces to 60K tokens)
- But we could have stopped earlier if strategies coordinated

**Recommendation:**
- Add a status return to strategies:
```rust
pub enum StrategyResult {
    Applied,
    TargetReached,
    NoChange,
}

#[async_trait]
pub trait ContextManagementStrategy: Send + Sync {
    async fn apply(&self, conversation: &mut Conversation) -> Result<StrategyResult>;
}

pub async fn apply_strategies(&self, conversation: &mut Conversation) -> Result<()> {
    for strategy in &self.strategies {
        let result = strategy.apply(conversation).await?;
        if matches!(result, StrategyResult::TargetReached) {
            break;
        }
    }
    Ok(())
}
```

---

### 7. 🟡 **MODERATE: Sliding Window Can Exceed Message Window Size**

**Location:** `src/context_management/sliding_window_strategy.rs` lines 64-72

```rust
if preserved_count >= total_to_keep {
    // Keep only preserved messages (maintaining order)
    conversation.messages = conversation
        .messages
        .drain(..)
        .enumerate()
        .filter_map(|(i, msg)| if keep_flags[i] { Some(msg) } else { None })
        .collect();

    return Ok(());
}
```

**Issue:** If there are more preserved messages than `window_size`, ALL are kept.

**Example:**
- window_size = 10
- preserve_system = true
- 15 system messages
- Result: All 15 kept, window_size limit exceeded

**Test Evidence:**
`test_no_user_messages_only_system` shows this behavior but doesn't document as issue

**Recommendation:**
- Either limit preserved messages too, or
- Document this clearly as "window_size is minimum when preservation rules apply"
- Add a separate `max_preserved_messages` config

---

### 8. 🟢 **MINOR: Token Accountant Doesn't Track Conversation Context Tokens**

**Location:** `src/context_management/token_accountant.rs`

**Issue:** TokenAccountant only tracks tokens from LLM responses (input/output from API calls).

**Missing:**
- Actual conversation context token count
- Estimated tokens in conversation messages
- Could use a tokenizer to estimate

**Why it matters:**
- ContextManager.get_token_pressure() divides `current_context_tokens()` by `max_tokens`
- But `current_context_tokens()` = last input + last output from API
- NOT the actual tokens in the conversation history

**Example problem:**
- Send 50K token message to API
- Get 30K token response
- Pressure = 80K / 128K = 62%
- But actual conversation might have 120K tokens total
- Pressure calculation is misleading

**Recommendation:**
- Add method to estimate conversation tokens
- Use a tokenizer library (e.g., `tiktoken-rs`)
- Make pressure calculation more accurate

---

### 9. 🟢 **MINOR: No Maximum Recursion Check in JSON Truncation**

**Location:** `src/context_management/tool_output_truncation_strategy.rs` lines 76-86

```rust
fn truncate_json_strings(&self, value: &mut serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(s) => { ... }
        serde_json::Value::Object(map) => {
            let mut modified = false;
            for (_key, val) in map.iter_mut() {
                if self.truncate_json_strings(val) {  // ← Recursive
                    modified = true;
                }
            }
            modified
        }
        serde_json::Value::Array(arr) => {
            let mut modified = false;
            for item in arr.iter_mut() {
                if self.truncate_json_strings(item) {  // ← Recursive
                    modified = true;
                }
            }
            modified
        }
        _ => false,
    }
}
```

**Issue:** No protection against deeply nested JSON causing stack overflow.

**Risk:** Low (unlikely, but possible with malicious input)

**Recommendation:**
```rust
fn truncate_json_strings(&self, value: &mut serde_json::Value, depth: usize) -> bool {
    const MAX_DEPTH: usize = 100;
    if depth > MAX_DEPTH {
        return false;
    }
    
    match value {
        // ... 
        serde_json::Value::Object(map) => {
            for (_key, val) in map.iter_mut() {
                let _ = self.truncate_json_strings(val, depth + 1);
            }
            // ...
        }
        // ...
    }
}
```

---

## Summary Table

| Issue | Severity | Type | Status |
|-------|----------|------|--------|
| Strategies applied in wrong order | 🔴 CRITICAL | Logic | Needs Fix |
| Token pressure check timing | 🔴 CRITICAL | Logic | Needs Fix |
| Sliding window ignores token count | 🔴 CRITICAL | Design | Needs Review |
| Tool truncation preserves last result | 🟡 MODERATE | Design | Document/Fix |
| No summary strategy integration | 🟡 MODERATE | Missing Feature | Enhancement |
| No strategy coordination | 🟡 MODERATE | Design | Improvement |
| Window size can be exceeded by preservation | 🟡 MODERATE | Logic | Edge Case |
| Token accountant inaccuracy | 🟢 MINOR | Accuracy | Enhancement |
| JSON recursion depth | 🟢 MINOR | Safety | Enhancement |

---

## Recommendations Priority

### P0 (Critical - Fix Immediately)
1. Reverse strategy application order
2. Fix token pressure calculation timing
3. Review sliding window vs token count interaction

### P1 (High - Fix Soon)
4. Document and handle "preserve last result" behavior
5. Add strategy coordination mechanism
6. Fix window size preservation edge case

### P2 (Medium - Nice to Have)
7. Integrate summary compression strategy
8. Implement token counting with tokenizer
9. Add recursion depth limit to JSON truncation
