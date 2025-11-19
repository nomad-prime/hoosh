# Hoosh Context Management Fix Plan

**Date**: 2025-11-19
**Status**: Implementation Plan
**Goal**: Fix critical context management issues while maintaining Hoosh's architectural decisions

---

## Executive Summary

This plan addresses the critical gaps in Hoosh's context management system identified through comparison with Codex's approach. The implementation is structured in phases to fix the most critical issues first while minimizing disruption to the existing architecture.

### Key Problems Identified

1. **CRITICAL**: Token pressure calculation uses API response tokens, not actual conversation size
2. **CRITICAL**: Strategy execution order is backwards (truncate → window instead of window → truncate)
3. **MAJOR**: Last tool result is never truncated, preventing effective compression
4. **MAJOR**: Sliding window can exceed configured window_size when preserving messages
5. **MODERATE**: MessageSummarizer is implemented but never integrated
6. **MODERATE**: No strategy coordination mechanism

---

## Implementation Approach

### Guiding Principles

1. **Incremental, not revolutionary**: Fix problems one at a time, test thoroughly
2. **Preserve existing architecture**: Keep the strategy pattern, builder pattern, and module structure
3. **Maintain backward compatibility**: Add features, don't break existing configs
4. **Align with Hoosh's style**: Follow existing Rust idioms and patterns in the codebase
5. **Test-driven**: Each fix must include tests proving the issue is resolved

---

## Phase 1: Critical Fixes (P0)

These fixes address issues that make context management ineffective or misleading.

### Fix 1.1: Implement Proper Conversation Token Counting

**Problem**: `TokenAccountant` only tracks the last API response tokens, not the actual conversation size. This makes pressure calculations completely wrong.

**Current Behavior** (`token_accountant.rs:59-62`):
```rust
pub fn current_context_tokens(&self) -> usize {
    self.current_input_tokens.load(Ordering::Relaxed)
        + self.current_output_tokens.load(Ordering::Relaxed)
}
```

This returns the last API request's token count (e.g., 70K), not the full conversation (e.g., 120K).

#### Solution Design

**Approach**: Add a new method to estimate conversation tokens using the 4 bytes/token approximation (like Codex).

**File**: `src/context_management/token_accountant.rs`

**Changes**:

1. Add new public method `estimate_conversation_tokens()`:

```rust
/// Estimates token count for an entire conversation using 4 bytes/token approximation
pub fn estimate_conversation_tokens(conversation: &Conversation) -> usize {
    const APPROX_BYTES_PER_TOKEN: usize = 4;

    let total_bytes: usize = conversation
        .messages
        .iter()
        .map(|msg| Self::estimate_message_bytes(msg))
        .sum();

    // Round up: (bytes + 3) / 4
    total_bytes.saturating_add(APPROX_BYTES_PER_TOKEN.saturating_sub(1)) / APPROX_BYTES_PER_TOKEN
}

fn estimate_message_bytes(msg: &ConversationMessage) -> usize {
    let mut total = 0;

    // Content field
    if let Some(content) = &msg.content {
        total += content.len();
    }

    // Tool calls (including arguments JSON)
    if let Some(tool_calls) = &msg.tool_calls {
        for call in tool_calls {
            total += call.function.name.len();
            total += call.function.arguments.len();  // JSON can be large!
        }
    }

    // Role and other fields (small but count them)
    total += msg.role.len();

    if let Some(name) = &msg.name {
        total += name.len();
    }

    total
}
```

2. Update `ContextManager.get_token_pressure()` to use this new method:

**File**: `src/context_management/context_manager.rs:125-128`

**Before**:
```rust
pub fn get_token_pressure(&self) -> f32 {
    let current = self.token_accountant.current_context_tokens();
    current as f32 / self.config.max_tokens as f32
}
```

**After**:
```rust
pub fn get_token_pressure(&self, conversation: &Conversation) -> f32 {
    let current = TokenAccountant::estimate_conversation_tokens(conversation);
    current as f32 / self.config.max_tokens as f32
}
```

3. Update call sites in `core.rs:161-181`:

**File**: `src/agent/core.rs`

**Before**:
```rust
let current_pressure = context_manager.get_token_pressure();
```

**After**:
```rust
let current_pressure = context_manager.get_token_pressure(conversation);
```

4. Add unit tests:

**File**: `src/context_management/token_accountant.rs` (add to tests section)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::conversation::{Conversation, ConversationMessage};

    #[test]
    fn test_estimate_conversation_tokens_empty() {
        let conv = Conversation::new("test".to_string());
        assert_eq!(TokenAccountant::estimate_conversation_tokens(&conv), 0);
    }

    #[test]
    fn test_estimate_conversation_tokens_simple_messages() {
        let mut conv = Conversation::new("test".to_string());

        // "user" (4) + "Hello" (5) = 9 bytes / 4 = 2.25 → 3 tokens
        conv.messages.push(ConversationMessage {
            role: "user".to_string(),
            content: Some("Hello".to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        // "assistant" (9) + "Hi there" (8) = 17 bytes / 4 = 4.25 → 5 tokens
        conv.messages.push(ConversationMessage {
            role: "assistant".to_string(),
            content: Some("Hi there".to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        // Total: 9 + 17 = 26 bytes / 4 = 6.5 → 7 tokens
        assert_eq!(TokenAccountant::estimate_conversation_tokens(&conv), 7);
    }

    #[test]
    fn test_estimate_conversation_tokens_with_tool_calls() {
        let mut conv = Conversation::new("test".to_string());

        conv.messages.push(ConversationMessage {
            role: "assistant".to_string(),
            content: None,
            tool_calls: Some(vec![ToolCall {
                id: "call_123".to_string(),
                r#type: "function".to_string(),
                function: ToolFunction {
                    name: "read_file".to_string(),  // 9 bytes
                    arguments: r#"{"path": "/foo/bar.txt"}"#.to_string(),  // 25 bytes
                },
            }]),
            tool_call_id: None,
            name: None,
        });

        // "assistant" (9) + "read_file" (9) + arguments (25) = 43 bytes / 4 = 10.75 → 11 tokens
        assert_eq!(TokenAccountant::estimate_conversation_tokens(&conv), 11);
    }

    #[test]
    fn test_estimate_conversation_tokens_large_tool_output() {
        let mut conv = Conversation::new("test".to_string());

        // Simulate a 10KB tool output
        let large_output = "x".repeat(10_000);

        conv.messages.push(ConversationMessage {
            role: "tool".to_string(),
            content: Some(large_output),
            tool_calls: None,
            tool_call_id: Some("call_123".to_string()),
            name: Some("read_file".to_string()),
        });

        // "tool" (4) + "read_file" (9) + content (10000) = 10013 bytes / 4 = 2503.25 → 2504 tokens
        assert_eq!(TokenAccountant::estimate_conversation_tokens(&conv), 2504);
    }
}
```

**Impact**: This fixes the root cause of inaccurate pressure calculations. Token pressure will now reflect the actual conversation size.

**Testing**:
- Run existing tests: `cargo test token_accountant`
- Add integration test showing pressure now changes as conversation grows
- Verify warning threshold triggers at correct conversation size

**Files Modified**:
- `src/context_management/token_accountant.rs` (add method + tests)
- `src/context_management/context_manager.rs` (update get_token_pressure signature)
- `src/agent/core.rs` (pass conversation to get_token_pressure)

**Estimated Effort**: 2-3 hours

---

### Fix 1.2: Reverse Strategy Execution Order

**Problem**: Strategies run in the wrong order (truncate first, then window), wasting computation and reducing effectiveness.

**Current Order** (`session.rs:353-362`):
1. ToolOutputTruncationStrategy (reduces output size)
2. SlidingWindowStrategy (removes old messages)

**Should Be**:
1. SlidingWindowStrategy (remove old messages first)
2. ToolOutputTruncationStrategy (truncate remaining outputs)

#### Solution Design

**File**: `src/session.rs`

**Change**: Simply reverse the order of strategy registration.

**Before** (lines 353-362):
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

**After**:
```rust
// Apply sliding window FIRST to remove old messages
if let Some(sliding_window_config) = context_manager_config.sliding_window {
    let sliding_window_strategy = SlidingWindowStrategy::new(sliding_window_config);
    context_manager_builder =
        context_manager_builder.add_strategy(Box::new(sliding_window_strategy));
}

// Apply truncation SECOND to reduce size of remaining messages
if let Some(truncation_config) = context_manager_config.tool_output_truncation {
    let truncation_strategy = ToolOutputTruncationStrategy::new(truncation_config);
    context_manager_builder =
        context_manager_builder.add_strategy(Box::new(truncation_strategy));
}
```

**Impact**: More efficient compression. If windowing removes 30 messages, truncation doesn't waste time processing them.

**Testing**:
- Create integration test with both strategies enabled
- Verify window runs before truncation (check debug logs or add strategy execution tracking)
- Compare final token count with old order vs new order (should be same or better)

**Files Modified**:
- `src/session.rs` (lines 353-362)

**Estimated Effort**: 30 minutes

---

### Fix 1.3: Recalculate Token Pressure After Compression

**Problem**: Token pressure is calculated BEFORE compression, then shown to user. Misleading.

**Current Flow** (`core.rs:161-181`):
```rust
let current_pressure = context_manager.get_token_pressure();  // BEFORE compression

if context_manager.should_warn_about_pressure() {
    self.send_event(AgentEvent::TokenPressureWarning { ... });
}

context_manager.apply_strategies(conversation).await?;  // AFTER warning sent
```

#### Solution Design

**Approach**: Calculate pressure, apply compression, THEN recalculate and warn if still high.

**File**: `src/agent/core.rs`

**Before** (lines 161-181):
```rust
async fn apply_context_compression(
    &self,
    conversation: &mut Conversation,
    context_manager: &ContextManager,
) -> Result<()> {
    let current_pressure = context_manager.get_token_pressure(conversation);

    if context_manager.should_warn_about_pressure() {
        self.send_event(AgentEvent::TokenPressureWarning {
            current_pressure,
            threshold: context_manager.config.warning_threshold,
        });
    }

    context_manager
        .apply_strategies(conversation)
        .await
        .expect("error applying context management");

    Ok(())
}
```

**After**:
```rust
async fn apply_context_compression(
    &self,
    conversation: &mut Conversation,
    context_manager: &ContextManager,
) -> Result<()> {
    // Calculate pressure BEFORE compression
    let pressure_before = context_manager.get_token_pressure(conversation);

    // Apply compression strategies
    context_manager
        .apply_strategies(conversation)
        .await
        .expect("error applying context management");

    // Recalculate pressure AFTER compression
    let pressure_after = context_manager.get_token_pressure(conversation);

    // Warn if STILL above threshold after compression
    if context_manager.should_warn_about_pressure_value(pressure_after) {
        self.send_event(AgentEvent::TokenPressureWarning {
            current_pressure: pressure_after,  // Show POST-compression pressure
            threshold: context_manager.config.warning_threshold,
        });
    }

    Ok(())
}
```

**Additional Change**: Add helper method to `ContextManager`:

**File**: `src/context_management/context_manager.rs`

```rust
/// Check if a specific pressure value exceeds the warning threshold
pub fn should_warn_about_pressure_value(&self, pressure: f32) -> bool {
    pressure >= self.config.warning_threshold
}
```

**Impact**: Users see accurate post-compression pressure. If compression successfully reduces pressure below threshold, no warning is shown.

**Testing**:
- Create test conversation with 100K tokens
- Set warning_threshold = 0.70, max_tokens = 128K
- Apply compression that reduces to 80K tokens (62% pressure)
- Verify NO warning is sent (because 62% < 70%)

**Files Modified**:
- `src/agent/core.rs` (update apply_context_compression)
- `src/context_management/context_manager.rs` (add should_warn_about_pressure_value)

**Estimated Effort**: 1 hour

---

## Phase 2: Major Improvements (P1)

These fixes improve the effectiveness and flexibility of context management.

### Fix 2.1: Make Last Tool Result Preservation Configurable

**Problem**: Last tool result is ALWAYS kept at full size, no matter how large. This is hardcoded.

**Current Behavior** (`tool_output_truncation_strategy.rs:134-145`):
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
        continue;  // Skip truncation unconditionally
    }
    // ...
}
```

#### Solution Design

**Approach**: Add configuration option to control this behavior.

**File**: `src/context_management/tool_output_truncation_strategy.rs`

1. Update `ToolOutputTruncationConfig` structure (around line 54):

**Before**:
```rust
pub struct ToolOutputTruncationConfig {
    pub max_length: usize,
    pub show_truncation_notice: bool,
    pub smart_truncate: bool,
    pub head_length: usize,
    pub tail_length: usize,
}
```

**After**:
```rust
pub struct ToolOutputTruncationConfig {
    pub max_length: usize,
    pub show_truncation_notice: bool,
    pub smart_truncate: bool,
    pub head_length: usize,
    pub tail_length: usize,
    pub preserve_last_tool_result: bool,  // NEW: Default true for backward compatibility
}
```

2. Update `Default` implementation (around line 60):

```rust
impl Default for ToolOutputTruncationConfig {
    fn default() -> Self {
        Self {
            max_length: 4000,
            show_truncation_notice: true,
            smart_truncate: false,
            head_length: 3000,
            tail_length: 1000,
            preserve_last_tool_result: true,  // Default to current behavior
        }
    }
}
```

3. Update `apply()` method to check config (around line 134):

**Before**:
```rust
for i in 0..message_count {
    if Some(i) == last_tool_result_index {
        continue;
    }
    // ...
}
```

**After**:
```rust
for i in 0..message_count {
    // Only skip if preserve_last_tool_result is enabled
    if self.config.preserve_last_tool_result && Some(i) == last_tool_result_index {
        continue;
    }
    // ...
}
```

4. Update `example_config.toml` to document the option:

**File**: `example_config.toml`

```toml
[context_manager.tool_output_truncation]
max_length = 4000
show_truncation_notice = true
smart_truncate = false
head_length = 3000
tail_length = 1000
preserve_last_tool_result = true  # If false, even the last tool result will be truncated
```

**Impact**: Users can disable preservation if they hit context limits due to large final results.

**Testing**:
- Add test with `preserve_last_tool_result = false` showing last result IS truncated
- Verify default behavior unchanged (preserve = true)

**Files Modified**:
- `src/context_management/tool_output_truncation_strategy.rs` (config + logic)
- `example_config.toml` (documentation)

**Estimated Effort**: 1-2 hours

---

### Fix 2.2: Add Strategy Coordination Mechanism

**Problem**: Strategies always run to completion, even if target is reached. No feedback between strategies.

**Current Behavior** (`context_manager.rs:143-148`):
```rust
pub async fn apply_strategies(&self, conversation: &mut Conversation) -> Result<()> {
    for strategy in &self.strategies {
        strategy.apply(conversation).await?;  // All run unconditionally
    }
    Ok(())
}
```

#### Solution Design

**Approach**: Introduce `StrategyResult` enum to allow strategies to signal their status.

**Files Modified**:
1. `src/context_management/mod.rs` (add StrategyResult enum)
2. `src/context_management/context_manager.rs` (update apply_strategies)
3. `src/context_management/tool_output_truncation_strategy.rs` (return StrategyResult)
4. `src/context_management/sliding_window_strategy.rs` (return StrategyResult)

**Step 1: Define StrategyResult enum**

**File**: `src/context_management/mod.rs`

```rust
/// Result of applying a context management strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrategyResult {
    /// Strategy was applied and modified the conversation
    Applied,

    /// Strategy was applied but made no changes (already within target)
    NoChange,

    /// Strategy reached the token target, stop further processing
    TargetReached,
}
```

**Step 2: Update trait definition**

**File**: `src/context_management/context_manager.rs` (around line 28)

**Before**:
```rust
#[async_trait]
pub trait ContextManagementStrategy: Send + Sync {
    async fn apply(&self, conversation: &mut Conversation) -> Result<()>;
}
```

**After**:
```rust
#[async_trait]
pub trait ContextManagementStrategy: Send + Sync {
    async fn apply(&self, conversation: &mut Conversation) -> Result<StrategyResult>;
}
```

**Step 3: Update apply_strategies to respect TargetReached**

**File**: `src/context_management/context_manager.rs`

**Before**:
```rust
pub async fn apply_strategies(&self, conversation: &mut Conversation) -> Result<()> {
    for strategy in &self.strategies {
        strategy.apply(conversation).await?;
    }
    Ok(())
}
```

**After**:
```rust
pub async fn apply_strategies(&self, conversation: &mut Conversation) -> Result<()> {
    for strategy in &self.strategies {
        let result = strategy.apply(conversation).await?;

        // If strategy reports target reached, stop processing further strategies
        if result == StrategyResult::TargetReached {
            break;
        }
    }
    Ok(())
}
```

**Step 4: Update SlidingWindowStrategy to return StrategyResult**

**File**: `src/context_management/sliding_window_strategy.rs`

**Before** (line 39):
```rust
async fn apply(&self, conversation: &mut Conversation) -> Result<()> {
    // ... logic ...
    Ok(())
}
```

**After**:
```rust
async fn apply(&self, conversation: &mut Conversation) -> Result<StrategyResult> {
    let initial_count = conversation.messages.len();

    // ... existing logic ...

    let final_count = conversation.messages.len();

    if final_count < initial_count {
        Ok(StrategyResult::Applied)
    } else {
        Ok(StrategyResult::NoChange)
    }
}
```

**Step 5: Update ToolOutputTruncationStrategy to return StrategyResult**

**File**: `src/context_management/tool_output_truncation_strategy.rs`

**Before** (line 127):
```rust
async fn apply(&self, conversation: &mut Conversation) -> Result<()> {
    // ... logic ...
    Ok(())
}
```

**After**:
```rust
async fn apply(&self, conversation: &mut Conversation) -> Result<StrategyResult> {
    let mut any_truncated = false;

    // ... existing truncation logic, set any_truncated = true when truncating ...

    if any_truncated {
        Ok(StrategyResult::Applied)
    } else {
        Ok(StrategyResult::NoChange)
    }
}
```

**Future Enhancement**: Strategies could check token pressure and return `TargetReached` if below compression_threshold.

**Impact**: Enables future optimizations where strategies can stop early if target is reached.

**Testing**:
- Add test showing `TargetReached` stops further strategies
- Verify existing tests still pass with StrategyResult changes

**Files Modified**:
- `src/context_management/mod.rs` (add enum)
- `src/context_management/context_manager.rs` (trait + apply_strategies)
- `src/context_management/sliding_window_strategy.rs` (return StrategyResult)
- `src/context_management/tool_output_truncation_strategy.rs` (return StrategyResult)

**Estimated Effort**: 2-3 hours

---

### Fix 2.3: Fix Sliding Window to Respect window_size Constraint

**Problem**: When `preserve_system = true`, the window can exceed `window_size` if there are many system messages.

**Current Behavior** (`sliding_window_strategy.rs:71-81`):
```rust
if preserved_count >= total_to_keep {
    // Keep only preserved messages (maintaining order)
    conversation.messages = conversation
        .messages
        .drain(..)
        .enumerate()
        .filter_map(|(i, msg)| if keep_flags[i] { Some(msg) } else { None })
        .collect();

    return Ok(StrategyResult::Applied);  // Can return 15 messages when window_size=10!
}
```

#### Solution Design

**Approach**: Add a config option to control behavior: strict window size vs preservation priority.

**File**: `src/context_management/sliding_window_strategy.rs`

1. Update config structure (around line 8):

**Before**:
```rust
pub struct SlidingWindowConfig {
    pub window_size: usize,
    pub preserve_system: bool,
    pub min_messages_before_windowing: usize,
    pub preserve_initial_task: bool,
}
```

**After**:
```rust
pub struct SlidingWindowConfig {
    pub window_size: usize,
    pub preserve_system: bool,
    pub min_messages_before_windowing: usize,
    pub preserve_initial_task: bool,
    pub strict_window_size: bool,  // NEW: If true, enforce window_size as hard limit
}
```

2. Update Default (around line 14):

```rust
impl Default for SlidingWindowConfig {
    fn default() -> Self {
        Self {
            window_size: 40,
            preserve_system: true,
            min_messages_before_windowing: 50,
            preserve_initial_task: true,
            strict_window_size: false,  // Default: allow exceeding for preserved messages
        }
    }
}
```

3. Update apply() logic (around line 71):

**Before**:
```rust
if preserved_count >= total_to_keep {
    conversation.messages = conversation
        .messages
        .drain(..)
        .enumerate()
        .filter_map(|(i, msg)| if keep_flags[i] { Some(msg) } else { None })
        .collect();

    return Ok(StrategyResult::Applied);
}
```

**After**:
```rust
if preserved_count >= total_to_keep {
    if self.config.strict_window_size {
        // Strict mode: enforce window_size even for preserved messages
        // Keep the MOST RECENT window_size preserved messages
        let mut preserved_indices: Vec<usize> = keep_flags
            .iter()
            .enumerate()
            .filter(|(_, &k)| k)
            .map(|(i, _)| i)
            .collect();

        // Keep only the last window_size indices
        if preserved_indices.len() > total_to_keep {
            preserved_indices = preserved_indices
                .into_iter()
                .rev()
                .take(total_to_keep)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
        }

        conversation.messages = conversation
            .messages
            .drain(..)
            .enumerate()
            .filter_map(|(i, msg)| {
                if preserved_indices.contains(&i) {
                    Some(msg)
                } else {
                    None
                }
            })
            .collect();
    } else {
        // Legacy mode: keep all preserved messages even if exceeds window_size
        conversation.messages = conversation
            .messages
            .drain(..)
            .enumerate()
            .filter_map(|(i, msg)| if keep_flags[i] { Some(msg) } else { None })
            .collect();
    }

    return Ok(StrategyResult::Applied);
}
```

**Impact**: Users can enforce strict window size if needed, while maintaining backward compatibility.

**Testing**:
- Test with `strict_window_size = true` and 15 system messages, window_size=10 → exactly 10 kept
- Test with `strict_window_size = false` → all 15 kept (current behavior)

**Files Modified**:
- `src/context_management/sliding_window_strategy.rs` (config + logic)

**Estimated Effort**: 2 hours

---

## Phase 3: Feature Integration (P2)

These enhancements add new capabilities to the context management system.

### Fix 3.1: Integrate MessageSummarizer into Compression Pipeline

**Problem**: `MessageSummarizer` is fully implemented but never used.

**Goal**: Create a `SummaryCompactionStrategy` that uses the summarizer when pressure is very high.

#### Solution Design

**Approach**: Create a new strategy that triggers summarization when other strategies can't reduce enough.

**File**: `src/context_management/summary_compaction_strategy.rs` (NEW)

```rust
use super::{ContextManagementStrategy, StrategyResult};
use crate::agent::conversation::Conversation;
use crate::agent::message_summarizer::MessageSummarizer;
use crate::context_management::token_accountant::TokenAccountant;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryCompactionConfig {
    /// Trigger summarization when pressure exceeds this threshold (default: 0.90)
    pub trigger_threshold: f32,

    /// Target token count after summarization (default: 50% of max)
    pub target_token_ratio: f32,

    /// Minimum messages required before summarization (default: 20)
    pub min_messages_for_summary: usize,

    /// Number of recent messages to keep unsummarized (default: 10)
    pub preserve_recent_count: usize,
}

impl Default for SummaryCompactionConfig {
    fn default() -> Self {
        Self {
            trigger_threshold: 0.90,
            target_token_ratio: 0.50,
            min_messages_for_summary: 20,
            preserve_recent_count: 10,
        }
    }
}

pub struct SummaryCompactionStrategy {
    config: SummaryCompactionConfig,
    summarizer: MessageSummarizer,
    max_tokens: usize,
}

impl SummaryCompactionStrategy {
    pub fn new(config: SummaryCompressionConfig, summarizer: MessageSummarizer, max_tokens: usize) -> Self {
        Self {
            config,
            summarizer,
            max_tokens,
        }
    }
}

#[async_trait]
impl ContextManagementStrategy for SummaryCompactionStrategy {
    async fn apply(&self, conversation: &mut Conversation) -> Result<StrategyResult> {
        // Check if we should trigger summarization
        let current_tokens = TokenAccountant::estimate_conversation_tokens(conversation);
        let current_pressure = current_tokens as f32 / self.max_tokens as f32;

        if current_pressure < self.config.trigger_threshold {
            return Ok(StrategyResult::NoChange);
        }

        if conversation.messages.len() < self.config.min_messages_for_summary {
            return Ok(StrategyResult::NoChange);
        }

        // Split messages into "to summarize" and "to keep"
        let total_messages = conversation.messages.len();
        let preserve_count = self.config.preserve_recent_count.min(total_messages);
        let summarize_count = total_messages.saturating_sub(preserve_count);

        if summarize_count == 0 {
            return Ok(StrategyResult::NoChange);
        }

        // Extract messages to summarize
        let messages_to_summarize: Vec<_> = conversation
            .messages
            .drain(..summarize_count)
            .collect();

        // Summarize them
        let summary = self
            .summarizer
            .summarize(&messages_to_summarize, None)
            .await?;

        // Create summary message
        use crate::agent::conversation::ConversationMessage;
        let summary_message = ConversationMessage {
            role: "system".to_string(),
            content: Some(format!(
                "## Conversation Summary (Auto-Generated)\n\n{}",
                summary
            )),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };

        // Insert summary at the beginning
        conversation.messages.insert(0, summary_message);

        Ok(StrategyResult::Applied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: Add tests for summary compaction strategy
}
```

**Step 2: Register in session.rs**

**File**: `src/session.rs` (around line 366)

```rust
// Apply sliding window FIRST
if let Some(sliding_window_config) = context_manager_config.sliding_window {
    let sliding_window_strategy = SlidingWindowStrategy::new(sliding_window_config);
    context_manager_builder =
        context_manager_builder.add_strategy(Box::new(sliding_window_strategy));
}

// Apply truncation SECOND
if let Some(truncation_config) = context_manager_config.tool_output_truncation {
    let truncation_strategy = ToolOutputTruncationStrategy::new(truncation_config);
    context_manager_builder =
        context_manager_builder.add_strategy(Box::new(truncation_strategy));
}

// Apply summarization LAST (only if pressure still very high)
if let Some(summary_config) = context_manager_config.summary_compression {
    let summary_strategy = SummaryCompressionStrategy::new(
        summary_config,
        self.conversation_state.summarizer.clone(),
        context_manager_config.max_tokens,
    );
    context_manager_builder =
        context_manager_builder.add_strategy(Box::new(summary_strategy));
}
```

**Step 3: Add config option**

**File**: `src/context_management/context_manager.rs`

```rust
pub struct ContextManagerConfig {
    pub max_tokens: usize,
    pub compression_threshold: f32,
    pub preserve_recent_percentage: f32,
    pub warning_threshold: f32,
    pub tool_output_truncation: Option<ToolOutputTruncationConfig>,
    pub sliding_window: Option<SlidingWindowConfig>,
    pub summary_compression: Option<SummaryCompressionConfig>,  // NEW
}
```

**Impact**: Enables automatic summarization when conversation gets very large, inspired by Codex's compaction.

**Testing**:
- Create conversation with 100 messages, pressure > 90%
- Verify summarization triggers
- Check that recent 10 messages are preserved
- Verify summary is inserted as system message

**Files Created/Modified**:
- `src/context_management/summary_compression_strategy.rs` (NEW)
- `src/context_management/mod.rs` (export new strategy)
- `src/context_management/context_manager.rs` (add config)
- `src/session.rs` (register strategy)

**Estimated Effort**: 4-5 hours

---

### Fix 3.2: Add Token-Aware Sliding Window

**Problem**: Sliding window uses message COUNT, not token COUNT. A window of 40 messages could be 10K tokens or 100K tokens.

#### Solution Design

**Approach**: Add option to use token budget instead of message count.

**File**: `src/context_management/sliding_window_strategy.rs`

1. Update config:

```rust
pub struct SlidingWindowConfig {
    pub window_size: usize,
    pub preserve_system: bool,
    pub min_messages_before_windowing: usize,
    pub preserve_initial_task: bool,
    pub strict_window_size: bool,
    pub use_token_window: bool,           // NEW: Use token budget instead of message count
    pub token_window_budget: usize,       // NEW: Token budget if use_token_window=true
}
```

2. Update apply() to support token-based windowing:

```rust
async fn apply(&self, conversation: &mut Conversation) -> Result<StrategyResult> {
    if self.config.use_token_window {
        self.apply_token_window(conversation).await
    } else {
        self.apply_message_window(conversation).await
    }
}

async fn apply_token_window(&self, conversation: &mut Conversation) -> Result<StrategyResult> {
    // Calculate current token usage
    let current_tokens = TokenAccountant::estimate_conversation_tokens(conversation);

    if current_tokens <= self.config.token_window_budget {
        return Ok(StrategyResult::NoChange);
    }

    let mut keep_flags = vec![false; conversation.messages.len()];
    let mut preserved_tokens = 0usize;

    // Mark system messages and first user message for preservation
    for (i, msg) in conversation.messages.iter().enumerate() {
        if (self.config.preserve_system && msg.role == "system")
            || (self.config.preserve_initial_task && i == self.find_first_user_message())
        {
            keep_flags[i] = true;
            preserved_tokens += TokenAccountant::estimate_message_bytes(msg) / 4;
        }
    }

    // Fill remaining budget with most recent messages
    let remaining_budget = self.config.token_window_budget.saturating_sub(preserved_tokens);
    let mut used_budget = 0usize;

    for (i, msg) in conversation.messages.iter().enumerate().rev() {
        if keep_flags[i] {
            continue;  // Already marked
        }

        let msg_tokens = TokenAccountant::estimate_message_bytes(msg) / 4;

        if used_budget + msg_tokens <= remaining_budget {
            keep_flags[i] = true;
            used_budget += msg_tokens;
        }
    }

    // Filter messages
    conversation.messages = conversation
        .messages
        .drain(..)
        .enumerate()
        .filter_map(|(i, msg)| if keep_flags[i] { Some(msg) } else { None })
        .collect();

    Ok(StrategyResult::Applied)
}

async fn apply_message_window(&self, conversation: &mut Conversation) -> Result<StrategyResult> {
    // Existing message-count-based logic
    // ...
}
```

**Impact**: More precise control over context size using tokens instead of message count.

**Testing**:
- Create conversation with 100 small messages (total 10K tokens)
- Set token_window_budget = 50K
- Verify all messages kept
- Create conversation with 10 large messages (total 100K tokens)
- Set token_window_budget = 50K
- Verify only ~5 recent messages kept

**Files Modified**:
- `src/context_management/sliding_window_strategy.rs` (add token-aware windowing)

**Estimated Effort**: 3-4 hours

---

## Phase 4: Polish & Optimization (P3)

### Fix 4.1: Add Recursion Depth Limit to JSON Truncation

**Problem**: Unbounded recursion in `truncate_json_strings` could cause stack overflow on malicious deeply nested JSON.

**File**: `src/context_management/tool_output_truncation_strategy.rs`

**Solution**: Add depth parameter and limit.

```rust
const MAX_JSON_RECURSION_DEPTH: usize = 100;

fn truncate_json_strings(&self, value: &mut serde_json::Value) -> bool {
    self.truncate_json_strings_with_depth(value, 0)
}

fn truncate_json_strings_with_depth(&self, value: &mut serde_json::Value, depth: usize) -> bool {
    if depth > MAX_JSON_RECURSION_DEPTH {
        return false;  // Stop recursion
    }

    let mut modified = false;
    match value {
        serde_json::Value::String(s) => {
            if s.len() > self.config.max_length {
                *s = self.truncate_content(s);
                modified = true;
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                if self.truncate_json_strings_with_depth(item, depth + 1) {
                    modified = true;
                }
            }
        }
        serde_json::Value::Object(map) => {
            for (_key, val) in map.iter_mut() {
                if self.truncate_json_strings_with_depth(val, depth + 1) {
                    modified = true;
                }
            }
        }
        _ => {}
    }
    modified
}
```

**Impact**: Prevents stack overflow on pathological JSON inputs.

**Estimated Effort**: 1 hour

---

## Implementation Timeline

### Week 1: Phase 1 (Critical Fixes)
- **Day 1**: Fix 1.1 - Implement conversation token counting (2-3 hours)
- **Day 2**: Fix 1.2 - Reverse strategy order (30 min) + Fix 1.3 - Recalculate pressure (1 hour)
- **Day 3**: Testing and validation of Phase 1 fixes
- **Day 4-5**: Buffer for issues and additional testing

### Week 2: Phase 2 (Major Improvements)
- **Day 1**: Fix 2.1 - Configurable last result preservation (1-2 hours)
- **Day 2**: Fix 2.2 - Strategy coordination (2-3 hours)
- **Day 3**: Fix 2.3 - Strict window size (2 hours)
- **Day 4-5**: Testing and integration

### Week 3: Phase 3 (Feature Integration)
- **Day 1-2**: Fix 3.1 - Integrate summarizer (4-5 hours)
- **Day 3-4**: Fix 3.2 - Token-aware windowing (3-4 hours)
- **Day 5**: Integration testing

### Week 4: Phase 4 (Polish)
- **Day 1**: Fix 4.1 - Recursion depth limit (1 hour)
- **Day 2-5**: Documentation, final testing, bug fixes

**Total Estimated Effort**: 20-30 hours over 4 weeks

---

## Testing Strategy

### Unit Tests
- Add tests for each new function/method
- Update existing tests that break due to signature changes
- Aim for 100% coverage of new code

### Integration Tests
- Create end-to-end tests simulating real agent conversations
- Test with various configuration combinations
- Verify pressure calculations are accurate

### Regression Tests
- Run full existing test suite after each phase
- Ensure no behavioral changes unless intended
- Document any breaking changes

### Performance Tests
- Measure token estimation performance on large conversations
- Verify strategy execution time is acceptable
- Profile memory usage with summarization

---

## Migration Guide for Existing Configs

### Backward Compatibility

All changes maintain backward compatibility by:
1. Adding new optional config fields with sensible defaults
2. Preserving existing behavior when new options are not set
3. Not breaking existing TOML configurations

### Recommended Config Updates

**For Users Hitting Context Limits**:
```toml
[context_manager.tool_output_truncation]
preserve_last_tool_result = false  # Allow truncating even the last result

[context_manager.sliding_window]
strict_window_size = true  # Enforce window size strictly
```

**For Users Wanting Automatic Summarization**:
```toml
[context_manager.summary_compression]
trigger_threshold = 0.90
target_token_ratio = 0.50
preserve_recent_count = 10
```

**For Token-Aware Windowing**:
```toml
[context_manager.sliding_window]
use_token_window = true
token_window_budget = 100000  # Keep last 100K tokens
```

---

## Success Metrics

### Metrics to Track

1. **Accuracy**: Token pressure calculation accuracy (estimate vs actual API tokens)
2. **Effectiveness**: Average token reduction per compression cycle
3. **Performance**: Time to apply strategies on large conversations
4. **Reliability**: Number of context overflow errors before/after fixes

### Expected Improvements

- **Token pressure accuracy**: 95%+ correlation with actual API token counts
- **Context overflow reduction**: 80%+ reduction in overflow errors
- **Compression effectiveness**: 30-50% token reduction when triggered
- **User warnings**: Only show warnings when truly needed (reduce false positives)

---

### Documentation Updates

- Update `README.md` with new configuration options
- Add `CONTEXT_MANAGEMENT.md` explaining the system architecture
- Create migration guide for users on older versions
- Update `example_config.toml` with all new options

---

## Future Enhancements (Post-Implementation)

1. **Adaptive compression**: Automatically adjust thresholds based on usage patterns
2. **Compression analytics**: Track and report compression effectiveness metrics
3. **Custom strategies**: Allow users to register custom compression strategies
4. **Persistent summaries**: Store summaries across sessions for long-running conversations
5. **Multi-model support**: Different configs for different LLM providers

---

## Conclusion

This implementation plan provides a structured approach to fixing Hoosh's context management issues while:
- Preserving the existing architecture and design patterns
- Maintaining backward compatibility
- Adding flexibility through configuration
- Improving effectiveness and accuracy
- Enabling future enhancements

The phased approach allows for incremental progress with validation at each step, reducing risk and ensuring quality throughout the implementation.
