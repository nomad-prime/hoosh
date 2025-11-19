# Context Management Implementation Guide for Hoosh

**Based on Codex's Context Management System**

This document provides a comprehensive guide to implementing Codex's context management system for handling context length in agentic workflows. This is a complete reference for replicating the architecture in your coding agent "Hoosh".

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Core Components](#core-components)
3. [Token Tracking System](#token-tracking-system)
4. [Truncation System](#truncation-system)
5. [Auto-Compaction Strategy](#auto-compaction-strategy)
6. [History Normalization](#history-normalization)
7. [Configuration & Model Families](#configuration--model-families)
8. [Implementation Checklist](#implementation-checklist)
9. [Code Examples](#code-examples)

---

## Architecture Overview

### High-Level Design

Codex uses a **layered context management approach** with four primary strategies:

1. **Token Tracking** - Monitor usage in real-time
2. **Truncation** - Limit individual tool outputs to prevent bloat
3. **Pruning** - Remove oldest history items when approaching limits
4. **Compaction** - Summarize conversation when context fills up

```
┌─────────────────────────────────────────────────┐
│           Conversation Turn (User Input)        │
└───────────────────┬─────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────┐
│         1. Truncate Tool Outputs                │
│    (Apply truncation policy to new items)       │
└───────────────────┬─────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────┐
│         2. Record to Context Manager            │
│      (Add items to history with tracking)       │
└───────────────────┬─────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────┐
│         3. Normalize History                    │
│  (Ensure call/output pairs are intact)          │
└───────────────────┬─────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────┐
│         4. Estimate Token Count                 │
│    (4 bytes/token approximation)                │
└───────────────────┬─────────────────────────────┘
                    │
                    ▼
           ┌────────┴────────┐
           │ Within Limit?   │
           └────────┬────────┘
                    │
         ┌──────────┴──────────┐
         │                     │
        YES                   NO
         │                     │
         ▼                     ▼
    ┌────────┐          ┌─────────────┐
    │Continue│          │Auto-Compact │
    └────────┘          └─────────────┘
                              │
                              ▼
                     ┌─────────────────┐
                     │Remove Oldest    │
                     │Items if Needed  │
                     └────────┬────────┘
                              │
                              ▼
                     ┌─────────────────┐
                     │Summarize History│
                     └────────┬────────┘
                              │
                              ▼
                     ┌─────────────────┐
                     │Rebuild with     │
                     │Summary Message  │
                     └─────────────────┘
```

---

## Core Components

### 1. Context Manager (History Management)


The `ContextManager` is the central data structure that maintains conversation state.

#### Data Structure

```rust
pub struct ContextManager {
    /// History items ordered from oldest to newest
    conversation: Arc<Conversation>,

    /// Cumulative token usage tracking
    token_info: Option<TokenUsageInfo>,
}
```

**b. Token Estimation**

```rust
pub fn estimate_token_count(&self, turn_context: &TurnContext) -> Option<i64> {
    let tokenizer = Tokenizer::for_model(model.as_str()).ok()?;

    Some(
        self.items
            .iter()
            .map(|item| {
                serde_json::to_string(&item)
                    .map(|item| tokenizer.count(&item))
                    .unwrap_or_default()
            })
            .sum::<i64>()
            + tokenizer.count(base_instructions)
    )
}
```

**c. History Pruning**

```rust
pub fn remove_first_item(&mut self) {
    if !self.items.is_empty() {
        // Remove oldest item (FIFO)
        let removed = self.items.remove(0);

        // Also remove corresponding call/output pair
        normalize::remove_corresponding_for(&mut self.items, &removed);
    }
}
```

**d. Getting History for Prompt**

```rust
pub fn get_history_for_prompt(&mut self) -> Vec<ResponseItem> {
    let mut history = self.get_history();
    Self::remove_ghost_snapshots(&mut history);
    history
}
```

## Token Tracking System

### Token Usage Info Structure


```rust
pub struct TokenUsageInfo {
    /// Cumulative usage across all turns
    pub total_token_usage: TokenUsage,

    /// Usage from the most recent turn
    pub last_token_usage: TokenUsage,

    /// Model's context window size
    pub model_context_window: Option<i64>,
}

pub struct TokenUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cached_input_tokens: i64,
}
```

### Token Counting Methods

**a. Approximate Token Count (Fast)**

**File Reference:** `codex-rs/core/src/truncate.rs:358-361`

```rust
const APPROX_BYTES_PER_TOKEN: usize = 4;

pub fn approx_token_count(text: &str) -> usize {
    let len = text.len();
    len.saturating_add(APPROX_BYTES_PER_TOKEN.saturating_sub(1)) / APPROX_BYTES_PER_TOKEN
}
```

**Why 4 bytes per token?**
- Most tokenizers (GPT, Claude) average 3-5 bytes per token
- 4 is a safe middle ground that avoids expensive full tokenization
- Used for quick estimates during truncation decisions

### Updating Token Usage

**File Reference:** `codex-rs/core/src/context_manager/history.rs:115-125`

```rust
pub fn update_token_info(
    &mut self,
    usage: &TokenUsage,
    model_context_window: Option<i64>,
) {
    self.token_info = TokenUsageInfo::new_or_append(
        &self.token_info,
        &Some(usage.clone()),
        model_context_window,
    );
}
```

---

## Truncation System


### Truncation Strategies

#### 1. Middle Truncation (Preserve Beginning and End)


```rust
fn truncate_with_byte_estimate(s: &str, max_bytes: usize, source: TruncationSource) -> String {
    if s.is_empty() || max_bytes == 0 || s.len() <= max_bytes {
        return handle_edge_cases(s, max_bytes, source);
    }

    // Calculate how much to remove
    let total_bytes = s.len();
    let removed_bytes = total_bytes.saturating_sub(max_bytes);
    let marker = format_truncation_marker(source, removed_units_for_source(source, removed_bytes));
    let marker_len = marker.len();

    if marker_len >= max_bytes {
        return truncate_on_boundary(&marker, max_bytes).to_string();
    }

    // Budget for content (excluding marker)
    let keep_budget = max_bytes - marker_len;
    let (left_budget, right_budget) = split_budget(keep_budget);

    // Find prefix/suffix boundaries (prefer newlines)
    let prefix_end = pick_prefix_end(s, left_budget);
    let mut suffix_start = pick_suffix_start(s, right_budget);

    if suffix_start < prefix_end {
        suffix_start = prefix_end;
    }

    // Assemble: prefix + marker + suffix
    let mut out = assemble_truncated_output(&s[..prefix_end], &s[suffix_start..], &marker);

    // Ensure we're still within budget
    if out.len() > max_bytes {
        let boundary = truncate_on_boundary(&out, max_bytes);
        out.truncate(boundary.len());
    }

    out
}

fn split_budget(budget: usize) -> (usize, usize) {
    let left = budget / 2;
    (left, budget - left)
}

fn pick_prefix_end(s: &str, left_budget: usize) -> usize {
    // Try to find a newline boundary
    if let Some(head) = s.get(..left_budget)
        && let Some(i) = head.rfind('\n')
    {
        return i + 1;
    }
    truncate_on_boundary(s, left_budget).len()
}

fn pick_suffix_start(s: &str, right_budget: usize) -> usize {
    let start_tail = s.len().saturating_sub(right_budget);

    // Try to find a newline boundary
    if let Some(tail) = s.get(start_tail..)
        && let Some(i) = tail.find('\n')
    {
        return start_tail + i + 1;
    }

    // Find valid UTF-8 boundary
    let mut idx = start_tail.min(s.len());
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}
```

#### 2. Line/Byte Truncation


```rust
fn truncate_formatted_exec_output(
    content: &str,
    total_lines: usize,
    limit_bytes: usize,
    limit_lines: usize,
) -> String {
    let head_lines: usize = limit_lines / 2;
    let tail_lines: usize = limit_lines - head_lines;
    let head_bytes: usize = limit_bytes / 2;

    let segments: Vec<&str> = content.split_inclusive('\n').collect();
    let head_take = head_lines.min(segments.len());
    let tail_take = tail_lines.min(segments.len().saturating_sub(head_take));
    let omitted = segments.len().saturating_sub(head_take + tail_take);

    // Calculate byte positions for head and tail
    let head_slice_end: usize = segments
        .iter()
        .take(head_take)
        .map(|segment| segment.len())
        .sum();

    let tail_slice_start: usize = if tail_take == 0 {
        content.len()
    } else {
        content.len()
            - segments
                .iter()
                .rev()
                .take(tail_take)
                .map(|segment| segment.len())
                .sum::<usize>()
    };

    let head_slice = &content[..head_slice_end];
    let tail_slice = &content[tail_slice_start..];

    // Create appropriate marker
    let marker = if omitted > 0 {
        Some(format!("\n[... omitted {omitted} of {total_lines} lines ...]\n\n"))
    } else if content.len() > limit_bytes {
        let removed_bytes = content.len().saturating_sub(limit_bytes);
        Some(format!("\n[... removed {removed_bytes} bytes to fit {limit_bytes} byte limit ...]\n\n"))
    } else {
        None
    };

    // Assemble result
    let marker_len = marker.as_ref().map_or(0, String::len);
    let head_budget = head_bytes.min(limit_bytes.saturating_sub(marker_len));
    let head_part = take_bytes_at_char_boundary(head_slice, head_budget);

    let mut result = String::with_capacity(limit_bytes.min(content.len()));
    result.push_str(head_part);

    if let Some(marker_text) = marker.as_ref() {
        result.push_str(marker_text);
    }

    let remaining = limit_bytes.saturating_sub(result.len());
    if remaining > 0 {
        let tail_part = take_last_bytes_at_char_boundary(tail_slice, remaining);
        result.push_str(tail_part);
    }

    result
}
```

#### 3. Function Output Items Truncation


For multimodal outputs (text + images):

```rust
pub fn truncate_function_output_items_with_policy(
    items: &[FunctionCallOutputContentItem],
    policy: TruncationPolicy,
) -> Vec<FunctionCallOutputContentItem> {
    let mut out: Vec<FunctionCallOutputContentItem> = Vec::with_capacity(items.len());
    let mut remaining_budget = match policy {
        TruncationPolicy::Bytes(_) => policy.byte_budget(),
        TruncationPolicy::Tokens(_) => policy.token_budget(),
    };
    let mut omitted_text_items = 0usize;

    for it in items {
        match it {
            FunctionCallOutputContentItem::InputText { text } => {
                if remaining_budget == 0 {
                    omitted_text_items += 1;
                    continue;
                }

                let cost = match policy {
                    TruncationPolicy::Bytes(_) => text.len(),
                    TruncationPolicy::Tokens(_) => approx_token_count(text),
                };

                if cost <= remaining_budget {
                    out.push(FunctionCallOutputContentItem::InputText { text: text.clone() });
                    remaining_budget = remaining_budget.saturating_sub(cost);
                } else {
                    // Truncate to fit remaining budget
                    let snippet_policy = match policy {
                        TruncationPolicy::Bytes(_) => TruncationPolicy::Bytes(remaining_budget),
                        TruncationPolicy::Tokens(_) => TruncationPolicy::Tokens(remaining_budget),
                    };
                    let snippet = truncate_text(text, snippet_policy);
                    if !snippet.is_empty() {
                        out.push(FunctionCallOutputContentItem::InputText { text: snippet });
                    } else {
                        omitted_text_items += 1;
                    }
                    remaining_budget = 0;
                }
            }
            FunctionCallOutputContentItem::InputImage { image_url } => {
                // Images always pass through without counting against budget
                out.push(FunctionCallOutputContentItem::InputImage {
                    image_url: image_url.clone(),
                });
            }
        }
    }

    if omitted_text_items > 0 {
        out.push(FunctionCallOutputContentItem::InputText {
            text: format!("[omitted {omitted_text_items} text items ...]"),
        });
    }

    out
}
```

### Truncation Markers


```rust
fn format_truncation_marker(source: TruncationSource, removed_count: u64) -> String {
    match source {
        TruncationSource::Policy(TruncationPolicy::Tokens(_)) => {
            format!("[…{removed_count} tokens truncated…]")
        }
        TruncationSource::Policy(TruncationPolicy::Bytes(_)) => {
            format!("[…{removed_count} bytes truncated…]")
        }
        TruncationSource::LineOmission { total_lines } => {
            format!("[... omitted {removed_count} of {total_lines} lines ...]")
        }
        TruncationSource::ByteLimit { limit_bytes } => {
            format!("[... removed {removed_count} bytes to fit {limit_bytes} byte limit ...]")
        }
    }
}
```

---

## Auto-Compaction Strategy


### When Compaction Triggers

Compaction is triggered when:

1. Token usage exceeds `model_auto_compact_token_limit` (configured threshold)
2. API returns `context_length_exceeded` error

### Compaction Process

```rust
const COMPACT_USER_MESSAGE_MAX_TOKENS: usize = 20_000;

pub async fn run_inline_auto_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
) {
    let prompt = turn_context.compact_prompt().to_string();
    let input = vec![UserInput::Text { text: prompt }];
    run_compact_task_inner(sess, turn_context, input).await;
}
```

#### Step-by-Step Algorithm



```rust
async fn run_compact_task_inner(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    input: Vec<UserInput>,
) {
    // 1. Record the compaction request as a user input
    let initial_input_for_turn: ResponseInputItem = ResponseInputItem::from(input);

    let mut history = sess.clone_history().await;
    history.record_items(
        &[initial_input_for_turn.into()],
        turn_context.truncation_policy,
    );

    let mut truncated_count = 0usize;
    let max_retries = turn_context.client.get_provider().stream_max_retries();
    let mut retries = 0;

    loop {
        let turn_input = history.get_history_for_prompt();
        let prompt = Prompt {
            input: turn_input.clone(),
            ..Default::default()
        };

        // 2. Attempt to run compaction
        let attempt_result = drain_to_completed(&sess, turn_context.as_ref(), &prompt).await;

        match attempt_result {
            Ok(()) => {
                // Success - notify if items were pruned
                if truncated_count > 0 {
                    sess.notify_background_event(
                        turn_context.as_ref(),
                        format!(
                            "Trimmed {truncated_count} older conversation item(s) before compacting so the prompt fits the model context window."
                        ),
                    ).await;
                }
                break;
            }
            Err(CodexErr::Interrupted) => {
                return;
            }
            Err(e @ CodexErr::ContextWindowExceeded) => {
                // 3. If still exceeds context, remove oldest item iteratively
                if turn_input.len() > 1 {
                    error!(
                        "Context window exceeded while compacting; removing oldest history item. Error: {e}"
                    );
                    history.remove_first_item();
                    truncated_count += 1;
                    retries = 0;
                    continue;
                }

                // Can't compact further
                sess.set_total_tokens_full(turn_context.as_ref()).await;
                let event = EventMsg::Error(ErrorEvent {
                    message: e.to_string(),
                });
                sess.send_event(&turn_context, event).await;
                return;
            }
            Err(e) => {
                // Retry with backoff
                if retries < max_retries {
                    retries += 1;
                    let delay = backoff(retries);
                    sess.notify_stream_error(
                        turn_context.as_ref(),
                        format!("Reconnecting... {retries}/{max_retries}"),
                    ).await;
                    tokio::time::sleep(delay).await;
                    continue;
                } else {
                    let event = EventMsg::Error(ErrorEvent {
                        message: e.to_string(),
                    });
                    sess.send_event(&turn_context, event).await;
                    return;
                }
            }
        }
    }

    // 4. Extract summary from the model's response
    let history_snapshot = sess.clone_history().await.get_history();
    let summary_suffix =
        get_last_assistant_message_from_turn(&history_snapshot).unwrap_or_default();
    let summary_text = format!("{SUMMARY_PREFIX}\n{summary_suffix}");
    let user_messages = collect_user_messages(&history_snapshot);

    // 5. Build new compacted history
    let initial_context = sess.build_initial_context(turn_context.as_ref());
    let mut new_history = build_compacted_history(initial_context, &user_messages, &summary_text);

    // 6. Preserve ghost snapshots
    let ghost_snapshots: Vec<ResponseItem> = history_snapshot
        .iter()
        .filter(|item| matches!(item, ResponseItem::GhostSnapshot { .. }))
        .cloned()
        .collect();
    new_history.extend(ghost_snapshots);

    // 7. Replace history with compacted version
    sess.replace_history(new_history).await;

    // 8. Update token estimates
    if let Some(estimated_tokens) = sess
        .clone_history()
        .await
        .estimate_token_count(&turn_context)
    {
        sess.override_last_token_usage_estimate(&turn_context, estimated_tokens)
            .await;
    }

    // 9. Persist compaction event
    let rollout_item = RolloutItem::Compacted(CompactedItem {
        message: summary_text.clone(),
        replacement_history: None,
    });
    sess.persist_rollout_items(&[rollout_item]).await;

    // 10. Notify user
    let event = EventMsg::AgentMessage(AgentMessageEvent {
        message: "Compact task completed".to_string(),
    });
    sess.send_event(&turn_context, event).await;

    let warning = EventMsg::Warning(WarningEvent {
        message: "Heads up: Long conversations and multiple compactions can cause the model to be less accurate. Start a new conversation when possible to keep conversations small and targeted.".to_string(),
    });
    sess.send_event(&turn_context, warning).await;
}
```

### Building Compacted History


```rust
fn build_compacted_history_with_limit(
    mut history: Vec<ResponseItem>,
    user_messages: &[String],
    summary_text: &str,
    max_tokens: usize,
) -> Vec<ResponseItem> {
    let mut selected_messages: Vec<String> = Vec::new();

    if max_tokens > 0 {
        let mut remaining = max_tokens;

        // Take user messages from most recent backwards
        for message in user_messages.iter().rev() {
            if remaining == 0 {
                break;
            }

            let tokens = approx_token_count(message);

            if tokens <= remaining {
                selected_messages.push(message.clone());
                remaining = remaining.saturating_sub(tokens);
            } else {
                // Truncate last message to fit
                let truncated = truncate_text(message, TruncationPolicy::Tokens(remaining));
                selected_messages.push(truncated);
                break;
            }
        }

        selected_messages.reverse();
    }

    // Add selected user messages
    for message in &selected_messages {
        history.push(ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: message.clone(),
            }],
        });
    }

    // Add summary as final user message
    let summary_text = if summary_text.is_empty() {
        "(no summary available)".to_string()
    } else {
        summary_text.to_string()
    };

    history.push(ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText { text: summary_text }],
    });

    history
}
```

### Compaction Prompt Template


```markdown
The conversation history has grown too long. Please write a comprehensive summary of the conversation so far.

Your summary should:
1. Capture all important context, decisions, and information discussed
2. Preserve technical details, file paths, and specific implementation choices
3. Note any outstanding issues, bugs, or tasks that remain
4. Be detailed enough to allow continuing the conversation naturally

Write your summary now:
```


```markdown
## Conversation Summary (Auto-Generated)
```

---

## History Normalization


History normalization ensures **conversation invariants** are maintained:

1. Every tool call has a corresponding output
2. Every output has a corresponding tool call

### Ensuring Call Outputs Are Present

```rust
pub fn ensure_call_outputs_present(items: &mut Vec<ResponseItem>) {
    let mut missing_outputs_to_insert: Vec<(usize, ResponseItem)> = Vec::new();

    for (idx, item) in items.iter().enumerate() {
        match item {
            ResponseItem::FunctionCall { call_id, .. } => {
                let has_output = items.iter().any(|i| match i {
                    ResponseItem::FunctionCallOutput {
                        call_id: existing, ..
                    } => existing == call_id,
                    _ => false,
                });

                if !has_output {
                    error_or_panic(format!(
                        "Function call output is missing for call id: {call_id}"
                    ));

                    // Insert synthetic "aborted" output
                    missing_outputs_to_insert.push((
                        idx,
                        ResponseItem::FunctionCallOutput {
                            call_id: call_id.clone(),
                            output: FunctionCallOutputPayload {
                                content: "aborted".to_string(),
                                ..Default::default()
                            },
                        },
                    ));
                }
            }
            ResponseItem::CustomToolCall { call_id, .. } => {
                let has_output = items.iter().any(|i| match i {
                    ResponseItem::CustomToolCallOutput {
                        call_id: existing, ..
                    } => existing == call_id,
                    _ => false,
                });

                if !has_output {
                    error_or_panic(format!(
                        "Custom tool call output is missing for call id: {call_id}"
                    ));

                    missing_outputs_to_insert.push((
                        idx,
                        ResponseItem::CustomToolCallOutput {
                            call_id: call_id.clone(),
                            output: "aborted".to_string(),
                        },
                    ));
                }
            }
            ResponseItem::LocalShellCall { call_id, .. } => {
                if let Some(call_id) = call_id.as_ref() {
                    let has_output = items.iter().any(|i| match i {
                        ResponseItem::FunctionCallOutput {
                            call_id: existing, ..
                        } => existing == call_id,
                        _ => false,
                    });

                    if !has_output {
                        error_or_panic(format!(
                            "Local shell call output is missing for call id: {call_id}"
                        ));

                        missing_outputs_to_insert.push((
                            idx,
                            ResponseItem::FunctionCallOutput {
                                call_id: call_id.clone(),
                                output: FunctionCallOutputPayload {
                                    content: "aborted".to_string(),
                                    ..Default::default()
                                },
                            },
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    // Insert in reverse order to avoid re-indexing
    for (idx, output_item) in missing_outputs_to_insert.into_iter().rev() {
        items.insert(idx + 1, output_item);
    }
}
```

### Removing Orphan Outputs

```rust
pub fn remove_orphan_outputs(items: &mut Vec<ResponseItem>) {
    // Collect all valid call IDs
    let function_call_ids: HashSet<String> = items
        .iter()
        .filter_map(|i| match i {
            ResponseItem::FunctionCall { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect();

    let local_shell_call_ids: HashSet<String> = items
        .iter()
        .filter_map(|i| match i {
            ResponseItem::LocalShellCall {
                call_id: Some(call_id),
                ..
            } => Some(call_id.clone()),
            _ => None,
        })
        .collect();

    let custom_tool_call_ids: HashSet<String> = items
        .iter()
        .filter_map(|i| match i {
            ResponseItem::CustomToolCall { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect();

    // Remove outputs without matching calls
    items.retain(|item| match item {
        ResponseItem::FunctionCallOutput { call_id, .. } => {
            let has_match =
                function_call_ids.contains(call_id) || local_shell_call_ids.contains(call_id);
            if !has_match {
                error_or_panic(format!(
                    "Orphan function call output for call id: {call_id}"
                ));
            }
            has_match
        }
        ResponseItem::CustomToolCallOutput { call_id, .. } => {
            let has_match = custom_tool_call_ids.contains(call_id);
            if !has_match {
                error_or_panic(format!(
                    "Orphan custom tool call output for call id: {call_id}"
                ));
            }
            has_match
        }
        _ => true,
    });
}
```

### Removing Corresponding Pairs

When removing an item, also remove its counterpart:

```rust
pub fn remove_corresponding_for(items: &mut Vec<ResponseItem>, item: &ResponseItem) {
    match item {
        ResponseItem::FunctionCall { call_id, .. } => {
            // Remove corresponding output
            remove_first_matching(items, |i| {
                matches!(
                    i,
                    ResponseItem::FunctionCallOutput {
                        call_id: existing, ..
                    } if existing == call_id
                )
            });
        }
        ResponseItem::FunctionCallOutput { call_id, .. } => {
            // Remove corresponding call
            if let Some(pos) = items.iter().position(|i| {
                matches!(i, ResponseItem::FunctionCall { call_id: existing, .. } if existing == call_id)
            }) {
                items.remove(pos);
            } else if let Some(pos) = items.iter().position(|i| {
                matches!(i, ResponseItem::LocalShellCall { call_id: Some(existing), .. } if existing == call_id)
            }) {
                items.remove(pos);
            }
        }
        ResponseItem::CustomToolCall { call_id, .. } => {
            remove_first_matching(items, |i| {
                matches!(
                    i,
                    ResponseItem::CustomToolCallOutput {
                        call_id: existing, ..
                    } if existing == call_id
                )
            });
        }
        ResponseItem::CustomToolCallOutput { call_id, .. } => {
            remove_first_matching(
                items,
                |i| matches!(i, ResponseItem::CustomToolCall { call_id: existing, .. } if existing == call_id),
            );
        }
        ResponseItem::LocalShellCall {
            call_id: Some(call_id),
            ..
        } => {
            remove_first_matching(items, |i| {
                matches!(
                    i,
                    ResponseItem::FunctionCallOutput {
                        call_id: existing, ..
                    } if existing == call_id
                )
            });
        }
        _ => {}
    }
}
```

---

## Configuration & Model Families


### Model Family Structure

```rust
pub struct ModelFamily {
    /// Full model slug (e.g., "gpt-4.1-2025-04-14")
    pub slug: String,

    /// Family name (e.g., "gpt-4.1")
    pub family: String,

    /// Percentage of context window usable for inputs (default 95%)
    pub effective_context_window_percent: i64,

    /// Truncation policy for this model family
    pub truncation_policy: TruncationPolicy,

    /// Base instructions for the model
    pub base_instructions: String,

    // ... other model-specific settings
}
```

### Default Truncation Policies by Model


```rust
// GPT-4 family
truncation_policy: TruncationPolicy::Bytes(10_000)

// GPT-5 family
truncation_policy: TruncationPolicy::Bytes(10_000)

// GPT-5.1
truncation_policy: TruncationPolicy::Bytes(10_000)

// Codex/experimental models
truncation_policy: TruncationPolicy::Tokens(10_000)

// Test models
truncation_policy: TruncationPolicy::Tokens(10_000)
```

### Configuration Parameters


```rust
pub struct Config {
    /// Size of the context window for the model, in tokens
    pub model_context_window: Option<i64>,

    /// Maximum number of output tokens
    pub model_max_output_tokens: Option<i64>,

    /// Token usage threshold triggering auto-compaction
    pub model_auto_compact_token_limit: Option<i64>,

    /// Token budget for tool/function outputs
    pub tool_output_token_limit: Option<usize>,

    // ... other config fields
}
```

### Effective Context Window Calculation

```rust
fn calculate_effective_context_window(
    model_context_window: i64,
    effective_percent: i64,
) -> i64 {
    (model_context_window * effective_percent) / 100
}

// Example: GPT-4 with 128k context window
// Effective: (128_000 * 95) / 100 = 121_600 tokens
```

---

## Implementation Checklist

### Phase 1: Core Data Structures

- [ ] **Define ResponseItem enum** wi# Context Management Implementation Guide for Hoosh

**Based on Codex's Context Management System**

This document provides a comprehensive guide to implementing Codex's context management system for handling context length in agentic workflows. This is a complete reference for replicating the architecture in your coding agent "Hoosh".

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Core Components](#core-components)
3. [Token Tracking System](#token-tracking-system)
4. [Truncation System](#truncation-system)
5. [Auto-Compaction Strategy](#auto-compaction-strategy)
6. [History Normalization](#history-normalization)
7. [Configuration & Model Families](#configuration--model-families)
8. [Implementation Checklist](#implementation-checklist)
9. [Code Examples](#code-examples)

---

## Architecture Overview

### High-Level Design

Codex uses a **layered context management approach** with four primary strategies:

1. **Token Tracking** - Monitor usage in real-time
2. **Truncation** - Limit individual tool outputs to prevent bloat
3. **Pruning** - Remove oldest history items when approaching limits
4. **Compaction** - Summarize conversation when context fills up

```
┌─────────────────────────────────────────────────┐
│           Conversation Turn (User Input)        │
└───────────────────┬─────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────┐
│         1. Truncate Tool Outputs                │
│    (Apply truncation policy to new items)       │
└───────────────────┬─────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────┐
│         2. Record to Context Manager            │
│      (Add items to history with tracking)       │
└───────────────────┬─────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────┐
│         3. Normalize History                    │
│  (Ensure call/output pairs are intact)          │
└───────────────────┬─────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────┐
│         4. Estimate Token Count                 │
│    (4 bytes/token approximation)                │
└───────────────────┬─────────────────────────────┘
                    │
                    ▼
           ┌────────┴────────┐
           │ Within Limit?   │
           └────────┬────────┘
                    │
         ┌──────────┴──────────┐
         │                     │
        YES                   NO
         │                     │
         ▼                     ▼
    ┌────────┐          ┌─────────────┐
    │Continue│          │Auto-Compact │
    └────────┘          └─────────────┘
                              │
                              ▼
                     ┌─────────────────┐
                     │Remove Oldest    │
                     │Items if Needed  │
                     └────────┬────────┘
                              │
                              ▼
                     ┌─────────────────┐
                     │Summarize History│
                     └────────┬────────┘
                              │
                              ▼
                     ┌─────────────────┐
                     │Rebuild with     │
                     │Summary Message  │
                     └─────────────────┘
```

---

## Core Components

### 1. Context Manager (History Management)


The `ContextManager` is the central data structure that maintains conversation state.

#### Data Structure

```rust
pub struct ContextManager {
    /// History items ordered from oldest to newest
    conversation: Arc<Conversation>,

    /// Cumulative token usage tracking
    token_info: Option<TokenUsageInfo>,
}
```

**b. Token Estimation**

```rust
pub fn estimate_token_count(&self, turn_context: &TurnContext) -> Option<i64> {
    let tokenizer = Tokenizer::for_model(model.as_str()).ok()?;

    Some(
        self.items
            .iter()
            .map(|item| {
                serde_json::to_string(&item)
                    .map(|item| tokenizer.count(&item))
                    .unwrap_or_default()
            })
            .sum::<i64>()
            + tokenizer.count(base_instructions)
    )
}
```

**c. History Pruning**

```rust
pub fn remove_first_item(&mut self) {
    if !self.items.is_empty() {
        // Remove oldest item (FIFO)
        let removed = self.items.remove(0);

        // Also remove corresponding call/output pair
        normalize::remove_corresponding_for(&mut self.items, &removed);
    }
}
```

**d. Getting History for Prompt**

```rust
pub fn get_history_for_prompt(&mut self) -> Vec<ResponseItem> {
    let mut history = self.get_history();
    Self::remove_ghost_snapshots(&mut history);
    history
}
```

## Token Tracking System

### Token Usage Info Structure


```rust
pub struct TokenUsageInfo {
    /// Cumulative usage across all turns
    pub total_token_usage: TokenUsage,

    /// Usage from the most recent turn
    pub last_token_usage: TokenUsage,

    /// Model's context window size
    pub model_context_window: Option<i64>,
}

pub struct TokenUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cached_input_tokens: i64,
}
```

### Token Counting Methods

**a. Approximate Token Count (Fast)**

**File Reference:** `codex-rs/core/src/truncate.rs:358-361`

```rust
const APPROX_BYTES_PER_TOKEN: usize = 4;

pub fn approx_token_count(text: &str) -> usize {
    let len = text.len();
    len.saturating_add(APPROX_BYTES_PER_TOKEN.saturating_sub(1)) / APPROX_BYTES_PER_TOKEN
}
```

**Why 4 bytes per token?**
- Most tokenizers (GPT, Claude) average 3-5 bytes per token
- 4 is a safe middle ground that avoids expensive full tokenization
- Used for quick estimates during truncation decisions

### Updating Token Usage

**File Reference:** `codex-rs/core/src/context_manager/history.rs:115-125`

```rust
pub fn update_token_info(
    &mut self,
    usage: &TokenUsage,
    model_context_window: Option<i64>,
) {
    self.token_info = TokenUsageInfo::new_or_append(
        &self.token_info,
        &Some(usage.clone()),
        model_context_window,
    );
}
```

---

## Truncation System


### Truncation Strategies

#### 1. Middle Truncation (Preserve Beginning and End)


```rust
fn truncate_with_byte_estimate(s: &str, max_bytes: usize, source: TruncationSource) -> String {
    if s.is_empty() || max_bytes == 0 || s.len() <= max_bytes {
        return handle_edge_cases(s, max_bytes, source);
    }

    // Calculate how much to remove
    let total_bytes = s.len();
    let removed_bytes = total_bytes.saturating_sub(max_bytes);
    let marker = format_truncation_marker(source, removed_units_for_source(source, removed_bytes));
    let marker_len = marker.len();

    if marker_len >= max_bytes {
        return truncate_on_boundary(&marker, max_bytes).to_string();
    }

    // Budget for content (excluding marker)
    let keep_budget = max_bytes - marker_len;
    let (left_budget, right_budget) = split_budget(keep_budget);

    // Find prefix/suffix boundaries (prefer newlines)
    let prefix_end = pick_prefix_end(s, left_budget);
    let mut suffix_start = pick_suffix_start(s, right_budget);

    if suffix_start < prefix_end {
        suffix_start = prefix_end;
    }

    // Assemble: prefix + marker + suffix
    let mut out = assemble_truncated_output(&s[..prefix_end], &s[suffix_start..], &marker);

    // Ensure we're still within budget
    if out.len() > max_bytes {
        let boundary = truncate_on_boundary(&out, max_bytes);
        out.truncate(boundary.len());
    }

    out
}

fn split_budget(budget: usize) -> (usize, usize) {
    let left = budget / 2;
    (left, budget - left)
}

fn pick_prefix_end(s: &str, left_budget: usize) -> usize {
    // Try to find a newline boundary
    if let Some(head) = s.get(..left_budget)
        && let Some(i) = head.rfind('\n')
    {
        return i + 1;
    }
    truncate_on_boundary(s, left_budget).len()
}

fn pick_suffix_start(s: &str, right_budget: usize) -> usize {
    let start_tail = s.len().saturating_sub(right_budget);

    // Try to find a newline boundary
    if let Some(tail) = s.get(start_tail..)
        && let Some(i) = tail.find('\n')
    {
        return start_tail + i + 1;
    }

    // Find valid UTF-8 boundary
    let mut idx = start_tail.min(s.len());
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}
```

#### 2. Line/Byte Truncation


```rust
fn truncate_formatted_exec_output(
    content: &str,
    total_lines: usize,
    limit_bytes: usize,
    limit_lines: usize,
) -> String {
    let head_lines: usize = limit_lines / 2;
    let tail_lines: usize = limit_lines - head_lines;
    let head_bytes: usize = limit_bytes / 2;

    let segments: Vec<&str> = content.split_inclusive('\n').collect();
    let head_take = head_lines.min(segments.len());
    let tail_take = tail_lines.min(segments.len().saturating_sub(head_take));
    let omitted = segments.len().saturating_sub(head_take + tail_take);

    // Calculate byte positions for head and tail
    let head_slice_end: usize = segments
        .iter()
        .take(head_take)
        .map(|segment| segment.len())
        .sum();

    let tail_slice_start: usize = if tail_take == 0 {
        content.len()
    } else {
        content.len()
            - segments
                .iter()
                .rev()
                .take(tail_take)
                .map(|segment| segment.len())
                .sum::<usize>()
    };

    let head_slice = &content[..head_slice_end];
    let tail_slice = &content[tail_slice_start..];

    // Create appropriate marker
    let marker = if omitted > 0 {
        Some(format!("\n[... omitted {omitted} of {total_lines} lines ...]\n\n"))
    } else if content.len() > limit_bytes {
        let removed_bytes = content.len().saturating_sub(limit_bytes);
        Some(format!("\n[... removed {removed_bytes} bytes to fit {limit_bytes} byte limit ...]\n\n"))
    } else {
        None
    };

    // Assemble result
    let marker_len = marker.as_ref().map_or(0, String::len);
    let head_budget = head_bytes.min(limit_bytes.saturating_sub(marker_len));
    let head_part = take_bytes_at_char_boundary(head_slice, head_budget);

    let mut result = String::with_capacity(limit_bytes.min(content.len()));
    result.push_str(head_part);

    if let Some(marker_text) = marker.as_ref() {
        result.push_str(marker_text);
    }

    let remaining = limit_bytes.saturating_sub(result.len());
    if remaining > 0 {
        let tail_part = take_last_bytes_at_char_boundary(tail_slice, remaining);
        result.push_str(tail_part);
    }

    result
}
```

#### 3. Function Output Items Truncation


For multimodal outputs (text + images):

```rust
pub fn truncate_function_output_items_with_policy(
    items: &[FunctionCallOutputContentItem],
    policy: TruncationPolicy,
) -> Vec<FunctionCallOutputContentItem> {
    let mut out: Vec<FunctionCallOutputContentItem> = Vec::with_capacity(items.len());
    let mut remaining_budget = match policy {
        TruncationPolicy::Bytes(_) => policy.byte_budget(),
        TruncationPolicy::Tokens(_) => policy.token_budget(),
    };
    let mut omitted_text_items = 0usize;

    for it in items {
        match it {
            FunctionCallOutputContentItem::InputText { text } => {
                if remaining_budget == 0 {
                    omitted_text_items += 1;
                    continue;
                }

                let cost = match policy {
                    TruncationPolicy::Bytes(_) => text.len(),
                    TruncationPolicy::Tokens(_) => approx_token_count(text),
                };

                if cost <= remaining_budget {
                    out.push(FunctionCallOutputContentItem::InputText { text: text.clone() });
                    remaining_budget = remaining_budget.saturating_sub(cost);
                } else {
                    // Truncate to fit remaining budget
                    let snippet_policy = match policy {
                        TruncationPolicy::Bytes(_) => TruncationPolicy::Bytes(remaining_budget),
                        TruncationPolicy::Tokens(_) => TruncationPolicy::Tokens(remaining_budget),
                    };
                    let snippet = truncate_text(text, snippet_policy);
                    if !snippet.is_empty() {
                        out.push(FunctionCallOutputContentItem::InputText { text: snippet });
                    } else {
                        omitted_text_items += 1;
                    }
                    remaining_budget = 0;
                }
            }
            FunctionCallOutputContentItem::InputImage { image_url } => {
                // Images always pass through without counting against budget
                out.push(FunctionCallOutputContentItem::InputImage {
                    image_url: image_url.clone(),
                });
            }
        }
    }

    if omitted_text_items > 0 {
        out.push(FunctionCallOutputContentItem::InputText {
            text: format!("[omitted {omitted_text_items} text items ...]"),
        });
    }

    out
}
```

### Truncation Markers


```rust
fn format_truncation_marker(source: TruncationSource, removed_count: u64) -> String {
    match source {
        TruncationSource::Policy(TruncationPolicy::Tokens(_)) => {
            format!("[…{removed_count} tokens truncated…]")
        }
        TruncationSource::Policy(TruncationPolicy::Bytes(_)) => {
            format!("[…{removed_count} bytes truncated…]")
        }
        TruncationSource::LineOmission { total_lines } => {
            format!("[... omitted {removed_count} of {total_lines} lines ...]")
        }
        TruncationSource::ByteLimit { limit_bytes } => {
            format!("[... removed {removed_count} bytes to fit {limit_bytes} byte limit ...]")
        }
    }
}
```

---

## Auto-Compaction Strategy


### When Compaction Triggers

Compaction is triggered when:

1. Token usage exceeds `model_auto_compact_token_limit` (configured threshold)
2. API returns `context_length_exceeded` error

### Compaction Process

```rust
const COMPACT_USER_MESSAGE_MAX_TOKENS: usize = 20_000;

pub async fn run_inline_auto_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
) {
    let prompt = turn_context.compact_prompt().to_string();
    let input = vec![UserInput::Text { text: prompt }];
    run_compact_task_inner(sess, turn_context, input).await;
}
```

#### Step-by-Step Algorithm



```rust
async fn run_compact_task_inner(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    input: Vec<UserInput>,
) {
    // 1. Record the compaction request as a user input
    let initial_input_for_turn: ResponseInputItem = ResponseInputItem::from(input);

    let mut history = sess.clone_history().await;
    history.record_items(
        &[initial_input_for_turn.into()],
        turn_context.truncation_policy,
    );

    let mut truncated_count = 0usize;
    let max_retries = turn_context.client.get_provider().stream_max_retries();
    let mut retries = 0;

    loop {
        let turn_input = history.get_history_for_prompt();
        let prompt = Prompt {
            input: turn_input.clone(),
            ..Default::default()
        };

        // 2. Attempt to run compaction
        let attempt_result = drain_to_completed(&sess, turn_context.as_ref(), &prompt).await;

        match attempt_result {
            Ok(()) => {
                // Success - notify if items were pruned
                if truncated_count > 0 {
                    sess.notify_background_event(
                        turn_context.as_ref(),
                        format!(
                            "Trimmed {truncated_count} older conversation item(s) before compacting so the prompt fits the model context window."
                        ),
                    ).await;
                }
                break;
            }
            Err(CodexErr::Interrupted) => {
                return;
            }
            Err(e @ CodexErr::ContextWindowExceeded) => {
                // 3. If still exceeds context, remove oldest item iteratively
                if turn_input.len() > 1 {
                    error!(
                        "Context window exceeded while compacting; removing oldest history item. Error: {e}"
                    );
                    history.remove_first_item();
                    truncated_count += 1;
                    retries = 0;
                    continue;
                }

                // Can't compact further
                sess.set_total_tokens_full(turn_context.as_ref()).await;
                let event = EventMsg::Error(ErrorEvent {
                    message: e.to_string(),
                });
                sess.send_event(&turn_context, event).await;
                return;
            }
            Err(e) => {
                // Retry with backoff
                if retries < max_retries {
                    retries += 1;
                    let delay = backoff(retries);
                    sess.notify_stream_error(
                        turn_context.as_ref(),
                        format!("Reconnecting... {retries}/{max_retries}"),
                    ).await;
                    tokio::time::sleep(delay).await;
                    continue;
                } else {
                    let event = EventMsg::Error(ErrorEvent {
                        message: e.to_string(),
                    });
                    sess.send_event(&turn_context, event).await;
                    return;
                }
            }
        }
    }

    // 4. Extract summary from the model's response
    let history_snapshot = sess.clone_history().await.get_history();
    let summary_suffix =
        get_last_assistant_message_from_turn(&history_snapshot).unwrap_or_default();
    let summary_text = format!("{SUMMARY_PREFIX}\n{summary_suffix}");
    let user_messages = collect_user_messages(&history_snapshot);

    // 5. Build new compacted history
    let initial_context = sess.build_initial_context(turn_context.as_ref());
    let mut new_history = build_compacted_history(initial_context, &user_messages, &summary_text);

    // 6. Preserve ghost snapshots
    let ghost_snapshots: Vec<ResponseItem> = history_snapshot
        .iter()
        .filter(|item| matches!(item, ResponseItem::GhostSnapshot { .. }))
        .cloned()
        .collect();
    new_history.extend(ghost_snapshots);

    // 7. Replace history with compacted version
    sess.replace_history(new_history).await;

    // 8. Update token estimates
    if let Some(estimated_tokens) = sess
        .clone_history()
        .await
        .estimate_token_count(&turn_context)
    {
        sess.override_last_token_usage_estimate(&turn_context, estimated_tokens)
            .await;
    }

    // 9. Persist compaction event
    let rollout_item = RolloutItem::Compacted(CompactedItem {
        message: summary_text.clone(),
        replacement_history: None,
    });
    sess.persist_rollout_items(&[rollout_item]).await;

    // 10. Notify user
    let event = EventMsg::AgentMessage(AgentMessageEvent {
        message: "Compact task completed".to_string(),
    });
    sess.send_event(&turn_context, event).await;

    let warning = EventMsg::Warning(WarningEvent {
        message: "Heads up: Long conversations and multiple compactions can cause the model to be less accurate. Start a new conversation when possible to keep conversations small and targeted.".to_string(),
    });
    sess.send_event(&turn_context, warning).await;
}
```

### Building Compacted History


```rust
fn build_compacted_history_with_limit(
    mut history: Vec<ResponseItem>,
    user_messages: &[String],
    summary_text: &str,
    max_tokens: usize,
) -> Vec<ResponseItem> {
    let mut selected_messages: Vec<String> = Vec::new();

    if max_tokens > 0 {
        let mut remaining = max_tokens;

        // Take user messages from most recent backwards
        for message in user_messages.iter().rev() {
            if remaining == 0 {
                break;
            }

            let tokens = approx_token_count(message);

            if tokens <= remaining {
                selected_messages.push(message.clone());
                remaining = remaining.saturating_sub(tokens);
            } else {
                // Truncate last message to fit
                let truncated = truncate_text(message, TruncationPolicy::Tokens(remaining));
                selected_messages.push(truncated);
                break;
            }
        }

        selected_messages.reverse();
    }

    // Add selected user messages
    for message in &selected_messages {
        history.push(ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: message.clone(),
            }],
        });
    }

    // Add summary as final user message
    let summary_text = if summary_text.is_empty() {
        "(no summary available)".to_string()
    } else {
        summary_text.to_string()
    };

    history.push(ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText { text: summary_text }],
    });

    history
}
```

### Compaction Prompt Template


```markdown
The conversation history has grown too long. Please write a comprehensive summary of the conversation so far.

Your summary should:
1. Capture all important context, decisions, and information discussed
2. Preserve technical details, file paths, and specific implementation choices
3. Note any outstanding issues, bugs, or tasks that remain
4. Be detailed enough to allow continuing the conversation naturally

Write your summary now:
```


```markdown
## Conversation Summary (Auto-Generated)
```

---

## History Normalization


History normalization ensures **conversation invariants** are maintained:

1. Every tool call has a corresponding output
2. Every output has a corresponding tool call

### Ensuring Call Outputs Are Present

```rust
pub fn ensure_call_outputs_present(items: &mut Vec<ResponseItem>) {
    let mut missing_outputs_to_insert: Vec<(usize, ResponseItem)> = Vec::new();

    for (idx, item) in items.iter().enumerate() {
        match item {
            ResponseItem::FunctionCall { call_id, .. } => {
                let has_output = items.iter().any(|i| match i {
                    ResponseItem::FunctionCallOutput {
                        call_id: existing, ..
                    } => existing == call_id,
                    _ => false,
                });

                if !has_output {
                    error_or_panic(format!(
                        "Function call output is missing for call id: {call_id}"
                    ));

                    // Insert synthetic "aborted" output
                    missing_outputs_to_insert.push((
                        idx,
                        ResponseItem::FunctionCallOutput {
                            call_id: call_id.clone(),
                            output: FunctionCallOutputPayload {
                                content: "aborted".to_string(),
                                ..Default::default()
                            },
                        },
                    ));
                }
            }
            ResponseItem::CustomToolCall { call_id, .. } => {
                let has_output = items.iter().any(|i| match i {
                    ResponseItem::CustomToolCallOutput {
                        call_id: existing, ..
                    } => existing == call_id,
                    _ => false,
                });

                if !has_output {
                    error_or_panic(format!(
                        "Custom tool call output is missing for call id: {call_id}"
                    ));

                    missing_outputs_to_insert.push((
                        idx,
                        ResponseItem::CustomToolCallOutput {
                            call_id: call_id.clone(),
                            output: "aborted".to_string(),
                        },
                    ));
                }
            }
            ResponseItem::LocalShellCall { call_id, .. } => {
                if let Some(call_id) = call_id.as_ref() {
                    let has_output = items.iter().any(|i| match i {
                        ResponseItem::FunctionCallOutput {
                            call_id: existing, ..
                        } => existing == call_id,
                        _ => false,
                    });

                    if !has_output {
                        error_or_panic(format!(
                            "Local shell call output is missing for call id: {call_id}"
                        ));

                        missing_outputs_to_insert.push((
                            idx,
                            ResponseItem::FunctionCallOutput {
                                call_id: call_id.clone(),
                                output: FunctionCallOutputPayload {
                                    content: "aborted".to_string(),
                                    ..Default::default()
                                },
                            },
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    // Insert in reverse order to avoid re-indexing
    for (idx, output_item) in missing_outputs_to_insert.into_iter().rev() {
        items.insert(idx + 1, output_item);
    }
}
```

### Removing Orphan Outputs

```rust
pub fn remove_orphan_outputs(items: &mut Vec<ResponseItem>) {
    // Collect all valid call IDs
    let function_call_ids: HashSet<String> = items
        .iter()
        .filter_map(|i| match i {
            ResponseItem::FunctionCall { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect();

    let local_shell_call_ids: HashSet<String> = items
        .iter()
        .filter_map(|i| match i {
            ResponseItem::LocalShellCall {
                call_id: Some(call_id),
                ..
            } => Some(call_id.clone()),
            _ => None,
        })
        .collect();

    let custom_tool_call_ids: HashSet<String> = items
        .iter()
        .filter_map(|i| match i {
            ResponseItem::CustomToolCall { call_id, .. } => Some(call_id.clone()),
            _ => None,
        })
        .collect();

    // Remove outputs without matching calls
    items.retain(|item| match item {
        ResponseItem::FunctionCallOutput { call_id, .. } => {
            let has_match =
                function_call_ids.contains(call_id) || local_shell_call_ids.contains(call_id);
            if !has_match {
                error_or_panic(format!(
                    "Orphan function call output for call id: {call_id}"
                ));
            }
            has_match
        }
        ResponseItem::CustomToolCallOutput { call_id, .. } => {
            let has_match = custom_tool_call_ids.contains(call_id);
            if !has_match {
                error_or_panic(format!(
                    "Orphan custom tool call output for call id: {call_id}"
                ));
            }
            has_match
        }
        _ => true,
    });
}
```

### Removing Corresponding Pairs

When removing an item, also remove its counterpart:

```rust
pub fn remove_corresponding_for(items: &mut Vec<ResponseItem>, item: &ResponseItem) {
    match item {
        ResponseItem::FunctionCall { call_id, .. } => {
            // Remove corresponding output
            remove_first_matching(items, |i| {
                matches!(
                    i,
                    ResponseItem::FunctionCallOutput {
                        call_id: existing, ..
                    } if existing == call_id
                )
            });
        }
        ResponseItem::FunctionCallOutput { call_id, .. } => {
            // Remove corresponding call
            if let Some(pos) = items.iter().position(|i| {
                matches!(i, ResponseItem::FunctionCall { call_id: existing, .. } if existing == call_id)
            }) {
                items.remove(pos);
            } else if let Some(pos) = items.iter().position(|i| {
                matches!(i, ResponseItem::LocalShellCall { call_id: Some(existing), .. } if existing == call_id)
            }) {
                items.remove(pos);
            }
        }
        ResponseItem::CustomToolCall { call_id, .. } => {
            remove_first_matching(items, |i| {
                matches!(
                    i,
                    ResponseItem::CustomToolCallOutput {
                        call_id: existing, ..
                    } if existing == call_id
                )
            });
        }
        ResponseItem::CustomToolCallOutput { call_id, .. } => {
            remove_first_matching(
                items,
                |i| matches!(i, ResponseItem::CustomToolCall { call_id: existing, .. } if existing == call_id),
            );
        }
        ResponseItem::LocalShellCall {
            call_id: Some(call_id),
            ..
        } => {
            remove_first_matching(items, |i| {
                matches!(
                    i,
                    ResponseItem::FunctionCallOutput {
                        call_id: existing, ..
                    } if existing == call_id
                )
            });
        }
        _ => {}
    }
}
```

---

## Configuration & Model Families


### Model Family Structure

```rust
pub struct ModelFamily {
    /// Full model slug (e.g., "gpt-4.1-2025-04-14")
    pub slug: String,

    /// Family name (e.g., "gpt-4.1")
    pub family: String,

    /// Percentage of context window usable for inputs (default 95%)
    pub effective_context_window_percent: i64,

    /// Truncation policy for this model family
    pub truncation_policy: TruncationPolicy,

    /// Base instructions for the model
    pub base_instructions: String,

    // ... other model-specific settings
}
```

### Default Truncation Policies by Model


```rust
// GPT-4 family
truncation_policy: TruncationPolicy::Bytes(10_000)

// GPT-5 family
truncation_policy: TruncationPolicy::Bytes(10_000)

// GPT-5.1
truncation_policy: TruncationPolicy::Bytes(10_000)

// Codex/experimental models
truncation_policy: TruncationPolicy::Tokens(10_000)

// Test models
truncation_policy: TruncationPolicy::Tokens(10_000)
```

### Configuration Parameters


```rust
pub struct Config {
    /// Size of the context window for the model, in tokens
    pub model_context_window: Option<i64>,

    /// Maximum number of output tokens
    pub model_max_output_tokens: Option<i64>,

    /// Token usage threshold triggering auto-compaction
    pub model_auto_compact_token_limit: Option<i64>,

    /// Token budget for tool/function outputs
    pub tool_output_token_limit: Option<usize>,

    // ... other config fields
}
```

### Effective Context Window Calculation

```rust
fn calculate_effective_context_window(
    model_context_window: i64,
    effective_percent: i64,
) -> i64 {
    (model_context_window * effective_percent) / 100
}

// Example: GPT-4 with 128k context window
// Effective: (128_000 * 95) / 100 = 121_600 tokens
```

---


## Code Examples

### Example 1: Basic Context Manager Usage

```rust
use context_manager::ContextManager;
use truncate::TruncationPolicy;

// Create context manager
let mut ctx = ContextManager::new();

// Configure truncation policy
let policy = TruncationPolicy::Bytes(10_000);

// Record conversation items
let items = vec![
    ResponseItem::Message {
        id: Some("user-1".to_string()),
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "Please analyze this file...".to_string(),
        }],
    },
    ResponseItem::FunctionCall {
        call_id: "call-1".to_string(),
        name: "read_file".to_string(),
        arguments: r#"{"path": "/path/to/file.txt"}"#.to_string(),
    },
    ResponseItem::FunctionCallOutput {
        call_id: "call-1".to_string(),
        output: FunctionCallOutputPayload {
            content: "File contents here...".to_string(),
            success: true,
            ..Default::default()
        },
    },
];

ctx.record_items(&items, policy);

// Get history for API prompt
let history_for_prompt = ctx.get_history_for_prompt();

// Estimate token count
let token_estimate = ctx.estimate_token_count(&turn_context);
```

### Example 2: Implementing Truncation

```rust
use truncate::{TruncationPolicy, truncate_text};

let long_output = "Very long tool output that exceeds limits...".repeat(1000);

// Truncate using bytes
let policy = TruncationPolicy::Bytes(1000);
let truncated = truncate_text(&long_output, policy);

// Truncated output format:
// "Very long tool output...[…2500 bytes truncated…]...that exceeds limits..."

// Truncate using tokens
let policy = TruncationPolicy::Tokens(100);
let truncated = truncate_text(&long_output, policy);

// Output: "Very long...[…900 tokens truncated…]...limits..."
```

### Example 3: Token Counting

```rust
use truncate::approx_token_count;

let text = "Hello, world! This is a test.";

// Approximate (fast)
let approx_tokens = approx_token_count(text);
// Result: (29 + 3) / 4 = 8 tokens

// Exact (slow, requires tokenizer)
let tokenizer = Tokenizer::for_model("gpt-4");
let exact_tokens = tokenizer.count(text);
// Result: 7 tokens (actual)
```

### Example 4: Handling Context Overflow

```rust
async fn handle_turn(
    ctx: &mut ContextManager,
    turn_context: &TurnContext,
) -> Result<(), CodexErr> {
    // Estimate current token usage
    let estimated_tokens = ctx.estimate_token_count(turn_context);

    if let Some(tokens) = estimated_tokens {
        let threshold = turn_context.config.model_auto_compact_token_limit;

        if let Some(limit) = threshold {
            if tokens >= limit {
                // Trigger auto-compaction
                run_auto_compact_task(ctx, turn_context).await?;
            }
        }
    }

    Ok(())
}
```

### Example 5: Building Compacted History

```rust
use compact::{build_compacted_history, collect_user_messages};

// Extract user messages from history
let user_messages = collect_user_messages(&history);
// Result: ["First message", "Second message", "Third message"]

// Model generates summary
let summary = "User requested feature X, we discussed implementation Y, \
               decided on approach Z. File A was modified, test B is pending.";

// Build compacted history
let initial_context = vec![/* system messages */];
let compacted = build_compacted_history(
    initial_context,
    &user_messages,
    summary,
);

// Result:
// [
//   /* initial_context items */,
//   Message { role: "user", content: "First message" },
//   Message { role: "user", content: "Second message" },
//   Message { role: "user", content: "Third message" },
//   Message { role: "user", content: "## Conversation Summary\n{summary}" },
// ]
```

### Example 6: Normalization in Action

```rust
use normalize::{ensure_call_outputs_present, remove_orphan_outputs};

let mut items = vec![
    ResponseItem::FunctionCall {
        call_id: "call-1".to_string(),
        name: "read_file".to_string(),
        arguments: "{}".to_string(),
    },
    // Missing output for call-1!
    ResponseItem::FunctionCallOutput {
        call_id: "call-999".to_string(),  // Orphan output!
        output: FunctionCallOutputPayload::default(),
    },
];

// Fix missing outputs
ensure_call_outputs_present(&mut items);
// Now items includes synthetic output for call-1:
// ResponseItem::FunctionCallOutput {
//     call_id: "call-1",
//     output: { content: "aborted", ... }
// }

// Remove orphans
remove_orphan_outputs(&mut items);
// Output for call-999 is removed
```

### Example 7: Complete Turn Workflow

```rust
async fn process_turn(
    session: &mut Session,
    user_input: Vec<UserInput>,
    config: &Config,
) -> Result<(), CodexErr> {
    // 1. Create input item
    let input_item = ResponseInputItem::from(user_input);

    // 2. Get truncation policy
    let policy = TruncationPolicy::new(config);

    // 3. Record input to context
    session.context_manager.record_items(&[input_item.into()], policy);

    // 4. Get history for API
    let history = session.context_manager.get_history_for_prompt();

    // 5. Check if compaction needed
    if let Some(tokens) = session.context_manager.estimate_token_count(&turn_context) {
        if let Some(limit) = config.model_auto_compact_token_limit {
            if tokens >= limit {
                run_auto_compact_task(session, &turn_context).await?;
            }
        }
    }

    // 6. Make API call
    let prompt = Prompt { input: history, ..Default::default() };
    let mut stream = client.stream(&prompt).await?;

    // 7. Process response
    while let Some(event) = stream.next().await {
        match event? {
            ResponseEvent::OutputItemDone(item) => {
                session.context_manager.record_items(&[item], policy);
            }
            ResponseEvent::Completed { token_usage, .. } => {
                session.context_manager.update_token_info(
                    &token_usage,
                    config.model_context_window,
                );
                break;
            }
            ResponseEvent::Error(e) => {
                if e.code == "context_length_exceeded" {
                    session.context_manager.set_token_usage_full(
                        config.model_context_window.unwrap_or(128_000)
                    );
                    return Err(CodexErr::ContextWindowExceeded);
                }
            }
            _ => {}
        }
    }

    Ok(())
}
```

---

## Key Design Principles

### 1. **Lazy Truncation**
Only truncate when storing items in the context manager, not proactively. This minimizes computational overhead.

### 2. **FIFO Pruning**
When removing items during compaction, always remove from the beginning (oldest first) to:
- Preserve recent context (most relevant)
- Maximize prompt cache hits (prefix-based caching)

### 3. **Approximate Token Counting**
Use 4 bytes/token heuristic for speed. Only use exact tokenization when precision is critical (e.g., final token usage reporting).

### 4. **UTF-8 Boundary Safety**
All truncation operations must respect UTF-8 character boundaries to prevent corruption.

### 5. **Newline Preference**
When truncating, prefer to break at newline boundaries for better readability.

### 6. **Separate Tool Budgets**
Tool outputs have dedicated token budgets to prevent them from consuming the entire context window.

### 7. **Conversation Invariants**
Always maintain the pairing of tool calls with outputs. Never leave orphans.

### 8. **Conservative Limits**
Use 95% of the context window as the effective limit to provide headroom for:
- System prompts
- Tool schemas
- Model output
- Tokenization variance

### 9. **Graceful Degradation**
Auto-compaction sequence:
1. Try normal operation
2. If overflow, try compaction
3. If still overflow, prune oldest items
4. If still can't fit, error gracefully

### 10. **User Visibility**
Always notify users when:
- Auto-compaction occurs
- Items are pruned
- Multiple compactions happen (warn about accuracy degradation)

---

## Performance Considerations

### Token Estimation Performance

| Method | Speed | Accuracy | Use Case |
|--------|-------|----------|----------|
| Approximate (4 bytes/token) | ~1µs | ±20% | Truncation decisions, quick checks |
| Exact tokenization | ~100µs | 100% | Final reporting, critical decisions |

### Memory Usage

- Each `ResponseItem` ≈ 100-500 bytes (depends on content)
- Typical conversation: 50-200 items = 5-100 KB
- Post-compaction: 10-30 items = 1-15 KB

### Compaction Triggers

Recommended thresholds:
- **128K context window**: Trigger at 100K tokens (78%)
- **200K context window**: Trigger at 160K tokens (80%)
- **1M context window**: Trigger at 800K tokens (80%)

---

## Testing Recommendations

### Unit Tests

1. **Token counting accuracy**
  - Test approximate vs exact on various text samples
  - Verify edge cases (empty, single char, unicode)

2. **Truncation correctness**
  - Middle truncation preserves start/end
  - Line truncation respects boundaries
  - UTF-8 safety maintained

3. **Normalization**
  - Orphan detection
  - Synthetic output generation
  - Paired removal

### Integration Tests

1. **End-to-end conversation**
  - Start conversation
  - Gradually fill context
  - Verify auto-compaction triggers
  - Check conversation quality post-compaction

2. **Error handling**
  - Context overflow recovery
  - Invalid item handling
  - Malformed history repair

### Performance Tests

1. **Token estimation speed**
  - Benchmark approximate vs exact
  - Profile tokenizer overhead

2. **Compaction latency**
  - Measure time to compact at various history sizes
  - Profile memory usage during compaction

---

## Common Pitfalls

### ❌ Don't Do This

```rust
// 1. Don't use exact tokenization in hot paths
for item in items {
    let tokens = tokenizer.count(&item);  // TOO SLOW!
    if tokens > limit { truncate(item); }
}

// 2. Don't forget UTF-8 boundaries
let truncated = &text[..max_bytes];  // DANGER: May split UTF-8!

// 3. Don't leave orphans
items.remove_if(|i| matches!(i, ResponseItem::FunctionCall { .. }));
// Now outputs are orphaned!

// 4. Don't double-truncate
let truncated = truncate_text(&already_truncated, policy);
// Loses information about original size
```

### ✅ Do This Instead

```rust
// 1. Use approximate counting
for item in items {
    let tokens = approx_token_count(&item);  // FAST!
    if tokens > limit { truncate(item); }
}

// 2. Always use boundary-safe functions
let truncated = truncate_on_boundary(text, max_bytes);

// 3. Use normalization functions
remove_corresponding_for(&mut items, &item_to_remove);

// 4. Check for existing truncation
if !content.contains("truncated") {
    let truncated = truncate_text(content, policy);
}
```

---

## Conclusion

This guide provides a complete blueprint for implementing Codex's context management system. The key to success is:

1. **Start with token tracking** - Get accurate usage data
2. **Implement truncation** - Prevent individual items from bloating
3. **Add normalization** - Maintain conversation integrity
4. **Build auto-compaction** - Handle long conversations gracefully

By following this architecture, Hoosh will be able to handle arbitrarily long agentic workflows while staying within context limits and maintaining conversation quality.

---

## References

- **Key Constants:**
  - `APPROX_BYTES_PER_TOKEN = 4`
  - `COMPACT_USER_MESSAGE_MAX_TOKENS = 20_000`
  - `effective_context_window_percent = 95`

---

- [ ] **Implement TokenUsage struct**
  ```
  - input_tokens: i64
  - output_tokens: i64
  - cached_input_tokens: i64
  ```

- [ ] **Implement TokenUsageInfo struct**
  ```
  - total_token_usage: TokenUsage
  - last_token_usage: TokenUsage
  - model_context_window: Option<i64>
  ```

- [ ] **Create ContextManager class**
  ```
  - items: Vec<ResponseItem>
  - token_info: Option<TokenUsageInfo>
  ```

### Phase 2: Token Tracking

- [ ] **Implement approximate token counting**
  ```
  APPROX_BYTES_PER_TOKEN = 4
  approx_token_count(text) = (text.len() + 3) / 4
  ```

- [ ] **Implement exact token counting** (using actual tokenizer)

- [ ] **Add token budget conversions**
  ```
  - approx_bytes_for_tokens(tokens) = tokens * 4
  - approx_tokens_from_byte_count(bytes) = (bytes + 3) / 4
  ```

- [ ] **Implement TokenUsageInfo::new_or_append()** for cumulative tracking

- [ ] **Add estimate_token_count()** to ContextManager

### Phase 3: Truncation System

- [ ] **Define TruncationPolicy enum**
  ```
  - Bytes(usize)
  - Tokens(usize)
  ```

- [ ] **Implement policy methods**
  ```
  - token_budget() -> usize
  - byte_budget() -> usize
  ```

- [ ] **Implement middle truncation**
  ```
  - truncate_with_byte_estimate(s, max_bytes, source) -> String
  - split_budget(budget) -> (left, right)
  - pick_prefix_end(s, budget) -> usize
  - pick_suffix_start(s, budget) -> usize
  ```

- [ ] **Implement line/byte truncation**
  ```
  - truncate_formatted_exec_output(content, total_lines, limit_bytes, limit_lines) -> String
  ```

- [ ] **Implement function output items truncation**
  ```
  - truncate_function_output_items_with_policy(items, policy) -> Vec<Item>
  ```

- [ ] **Create truncation markers**
  ```
  - "[…{count} tokens truncated…]"
  - "[…{count} bytes truncated…]"
  - "[... omitted {count} of {total} lines ...]"
  ```

- [ ] **Ensure UTF-8 boundary safety**
  ```
  - truncate_on_boundary(s, max_len) -> &str
  ```

### Phase 4: History Management

- [ ] **Implement ContextManager::record_items()**
  - Filter to API messages only
  - Apply truncation policy
  - Store in items vector

- [ ] **Implement ContextManager::get_history()**
  - Normalize history
  - Return cloned items

- [ ] **Implement ContextManager::get_history_for_prompt()**
  - Get normalized history
  - Remove ghost snapshots
  - Return API-ready items

- [ ] **Implement ContextManager::remove_first_item()**
  - Remove oldest item (index 0)
  - Remove corresponding call/output pair

- [ ] **Add ContextManager::replace()**
  - Replace entire history vector

### Phase 5: History Normalization

- [ ] **Implement ensure_call_outputs_present()**
  - Scan for calls without outputs
  - Insert synthetic "aborted" outputs
  - Maintain index integrity

- [ ] **Implement remove_orphan_outputs()**
  - Collect all valid call IDs
  - Remove outputs without matching calls

- [ ] **Implement remove_corresponding_for()**
  - Given an item, find its pair
  - Remove the corresponding call or output

### Phase 6: Auto-Compaction

- [ ] **Define compaction trigger conditions**
  - Token usage exceeds threshold
  - Context window exceeded error

- [ ] **Create compaction prompt template**

- [ ] **Implement run_compact_task_inner()**
  - Record compaction request
  - Attempt compaction
  - Handle context overflow by pruning
  - Extract summary from response
  - Build compacted history
  - Replace session history
  - Update token estimates

- [ ] **Implement build_compacted_history()**
  - Select recent user messages (up to 20k tokens)
  - Add summary as user message
  - Preserve initial context

- [ ] **Implement collect_user_messages()**
  - Extract all user messages from history
  - Filter out summary messages

- [ ] **Add retry logic with exponential backoff**

### Phase 7: Error Handling

- [ ] **Detect context window exceeded errors**
  ```
  error.code == "context_length_exceeded"
  ```

- [ ] **Set total tokens to full when exceeded**
  ```
  set_token_usage_full(context_window)
  ```

- [ ] **Return appropriate error responses**

### Phase 8: Model Configuration

- [ ] **Define ModelFamily struct**
  ```
  - slug: String
  - family: String
  - effective_context_window_percent: i64
  - truncation_policy: TruncationPolicy
  - base_instructions: String
  ```

- [ ] **Implement find_family_for_model()**
  - Match model slug to family
  - Return family configuration

- [ ] **Set per-model defaults**
  - GPT-4: Bytes(10_000)
  - GPT-5: Bytes(10_000)
  - Experimental: Tokens(10_000)

- [ ] **Calculate effective context window**
  ```
  effective = (context_window * percent) / 100
  ```

### Phase 9: Configuration

- [ ] **Add configuration parameters**
  ```
  - model_context_window: Option<i64>
  - model_auto_compact_token_limit: Option<i64>
  - tool_output_token_limit: Option<usize>
  ```

- [ ] **Implement TruncationPolicy::new(config)**
  - Merge family defaults with user overrides

### Phase 10: Integration & Testing

- [ ] **Test token counting accuracy**
  - Compare approximate vs exact
  - Verify 4 bytes/token assumption

- [ ] **Test truncation strategies**
  - Middle truncation preserves beginning/end
  - Line truncation respects line boundaries
  - UTF-8 boundaries maintained

- [ ] **Test normalization**
  - Orphan calls get synthetic outputs
  - Orphan outputs removed
  - Paired removal works correctly

- [ ] **Test auto-compaction**
  - Triggers at threshold
  - Successfully prunes when needed
  - Generates useful summaries

- [ ] **Test end-to-end workflow**
  - Long conversation triggers compaction
  - Context stays within limits
  - Conversation quality maintained

---

## Code Examples

### Example 1: Basic Context Manager Usage

```rust
use context_manager::ContextManager;
use truncate::TruncationPolicy;

// Create context manager
let mut ctx = ContextManager::new();

// Configure truncation policy
let policy = TruncationPolicy::Bytes(10_000);

// Record conversation items
let items = vec![
    ResponseItem::Message {
        id: Some("user-1".to_string()),
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "Please analyze this file...".to_string(),
        }],
    },
    ResponseItem::FunctionCall {
        call_id: "call-1".to_string(),
        name: "read_file".to_string(),
        arguments: r#"{"path": "/path/to/file.txt"}"#.to_string(),
    },
    ResponseItem::FunctionCallOutput {
        call_id: "call-1".to_string(),
        output: FunctionCallOutputPayload {
            content: "File contents here...".to_string(),
            success: true,
            ..Default::default()
        },
    },
];

ctx.record_items(&items, policy);

// Get history for API prompt
let history_for_prompt = ctx.get_history_for_prompt();

// Estimate token count
let token_estimate = ctx.estimate_token_count(&turn_context);
```

### Example 2: Implementing Truncation

```rust
use truncate::{TruncationPolicy, truncate_text};

let long_output = "Very long tool output that exceeds limits...".repeat(1000);

// Truncate using bytes
let policy = TruncationPolicy::Bytes(1000);
let truncated = truncate_text(&long_output, policy);

// Truncated output format:
// "Very long tool output...[…2500 bytes truncated…]...that exceeds limits..."

// Truncate using tokens
let policy = TruncationPolicy::Tokens(100);
let truncated = truncate_text(&long_output, policy);

// Output: "Very long...[…900 tokens truncated…]...limits..."
```

### Example 3: Token Counting

```rust
use truncate::approx_token_count;

let text = "Hello, world! This is a test.";

// Approximate (fast)
let approx_tokens = approx_token_count(text);
// Result: (29 + 3) / 4 = 8 tokens

// Exact (slow, requires tokenizer)
let tokenizer = Tokenizer::for_model("gpt-4");
let exact_tokens = tokenizer.count(text);
// Result: 7 tokens (actual)
```

### Example 4: Handling Context Overflow

```rust
async fn handle_turn(
    ctx: &mut ContextManager,
    turn_context: &TurnContext,
) -> Result<(), CodexErr> {
    // Estimate current token usage
    let estimated_tokens = ctx.estimate_token_count(turn_context);

    if let Some(tokens) = estimated_tokens {
        let threshold = turn_context.config.model_auto_compact_token_limit;

        if let Some(limit) = threshold {
            if tokens >= limit {
                // Trigger auto-compaction
                run_auto_compact_task(ctx, turn_context).await?;
            }
        }
    }

    Ok(())
}
```

### Example 5: Building Compacted History

```rust
use compact::{build_compacted_history, collect_user_messages};

// Extract user messages from history
let user_messages = collect_user_messages(&history);
// Result: ["First message", "Second message", "Third message"]

// Model generates summary
let summary = "User requested feature X, we discussed implementation Y, \
               decided on approach Z. File A was modified, test B is pending.";

// Build compacted history
let initial_context = vec![/* system messages */];
let compacted = build_compacted_history(
    initial_context,
    &user_messages,
    summary,
);

// Result:
// [
//   /* initial_context items */,
//   Message { role: "user", content: "First message" },
//   Message { role: "user", content: "Second message" },
//   Message { role: "user", content: "Third message" },
//   Message { role: "user", content: "## Conversation Summary\n{summary}" },
// ]
```

### Example 6: Normalization in Action

```rust
use normalize::{ensure_call_outputs_present, remove_orphan_outputs};

let mut items = vec![
    ResponseItem::FunctionCall {
        call_id: "call-1".to_string(),
        name: "read_file".to_string(),
        arguments: "{}".to_string(),
    },
    // Missing output for call-1!
    ResponseItem::FunctionCallOutput {
        call_id: "call-999".to_string(),  // Orphan output!
        output: FunctionCallOutputPayload::default(),
    },
];

// Fix missing outputs
ensure_call_outputs_present(&mut items);
// Now items includes synthetic output for call-1:
// ResponseItem::FunctionCallOutput {
//     call_id: "call-1",
//     output: { content: "aborted", ... }
// }

// Remove orphans
remove_orphan_outputs(&mut items);
// Output for call-999 is removed
```

### Example 7: Complete Turn Workflow

```rust
async fn process_turn(
    session: &mut Session,
    user_input: Vec<UserInput>,
    config: &Config,
) -> Result<(), CodexErr> {
    // 1. Create input item
    let input_item = ResponseInputItem::from(user_input);

    // 2. Get truncation policy
    let policy = TruncationPolicy::new(config);

    // 3. Record input to context
    session.context_manager.record_items(&[input_item.into()], policy);

    // 4. Get history for API
    let history = session.context_manager.get_history_for_prompt();

    // 5. Check if compaction needed
    if let Some(tokens) = session.context_manager.estimate_token_count(&turn_context) {
        if let Some(limit) = config.model_auto_compact_token_limit {
            if tokens >= limit {
                run_auto_compact_task(session, &turn_context).await?;
            }
        }
    }

    // 6. Make API call
    let prompt = Prompt { input: history, ..Default::default() };
    let mut stream = client.stream(&prompt).await?;

    // 7. Process response
    while let Some(event) = stream.next().await {
        match event? {
            ResponseEvent::OutputItemDone(item) => {
                session.context_manager.record_items(&[item], policy);
            }
            ResponseEvent::Completed { token_usage, .. } => {
                session.context_manager.update_token_info(
                    &token_usage,
                    config.model_context_window,
                );
                break;
            }
            ResponseEvent::Error(e) => {
                if e.code == "context_length_exceeded" {
                    session.context_manager.set_token_usage_full(
                        config.model_context_window.unwrap_or(128_000)
                    );
                    return Err(CodexErr::ContextWindowExceeded);
                }
            }
            _ => {}
        }
    }

    Ok(())
}
```

---

## Key Design Principles

### 1. **Lazy Truncation**
Only truncate when storing items in the context manager, not proactively. This minimizes computational overhead.

### 2. **FIFO Pruning**
When removing items during compaction, always remove from the beginning (oldest first) to:
- Preserve recent context (most relevant)
- Maximize prompt cache hits (prefix-based caching)

### 3. **Approximate Token Counting**
Use 4 bytes/token heuristic for speed. Only use exact tokenization when precision is critical (e.g., final token usage reporting).

### 4. **UTF-8 Boundary Safety**
All truncation operations must respect UTF-8 character boundaries to prevent corruption.

### 5. **Newline Preference**
When truncating, prefer to break at newline boundaries for better readability.

### 6. **Separate Tool Budgets**
Tool outputs have dedicated token budgets to prevent them from consuming the entire context window.

### 7. **Conversation Invariants**
Always maintain the pairing of tool calls with outputs. Never leave orphans.

### 8. **Conservative Limits**
Use 95% of the context window as the effective limit to provide headroom for:
- System prompts
- Tool schemas
- Model output
- Tokenization variance

### 9. **Graceful Degradation**
Auto-compaction sequence:
1. Try normal operation
2. If overflow, try compaction
3. If still overflow, prune oldest items
4. If still can't fit, error gracefully

### 10. **User Visibility**
Always notify users when:
- Auto-compaction occurs
- Items are pruned
- Multiple compactions happen (warn about accuracy degradation)

---

## Performance Considerations

### Token Estimation Performance

| Method | Speed | Accuracy | Use Case |
|--------|-------|----------|----------|
| Approximate (4 bytes/token) | ~1µs | ±20% | Truncation decisions, quick checks |
| Exact tokenization | ~100µs | 100% | Final reporting, critical decisions |

### Memory Usage

- Each `ResponseItem` ≈ 100-500 bytes (depends on content)
- Typical conversation: 50-200 items = 5-100 KB
- Post-compaction: 10-30 items = 1-15 KB

### Compaction Triggers

Recommended thresholds:
- **128K context window**: Trigger at 100K tokens (78%)
- **200K context window**: Trigger at 160K tokens (80%)
- **1M context window**: Trigger at 800K tokens (80%)

---

## Testing Recommendations

### Unit Tests

1. **Token counting accuracy**
   - Test approximate vs exact on various text samples
   - Verify edge cases (empty, single char, unicode)

2. **Truncation correctness**
   - Middle truncation preserves start/end
   - Line truncation respects boundaries
   - UTF-8 safety maintained

3. **Normalization**
   - Orphan detection
   - Synthetic output generation
   - Paired removal

### Integration Tests

1. **End-to-end conversation**
   - Start conversation
   - Gradually fill context
   - Verify auto-compaction triggers
   - Check conversation quality post-compaction

2. **Error handling**
   - Context overflow recovery
   - Invalid item handling
   - Malformed history repair

### Performance Tests

1. **Token estimation speed**
   - Benchmark approximate vs exact
   - Profile tokenizer overhead

2. **Compaction latency**
   - Measure time to compact at various history sizes
   - Profile memory usage during compaction

---

## Common Pitfalls

### ❌ Don't Do This

```rust
// 1. Don't use exact tokenization in hot paths
for item in items {
    let tokens = tokenizer.count(&item);  // TOO SLOW!
    if tokens > limit { truncate(item); }
}

// 2. Don't forget UTF-8 boundaries
let truncated = &text[..max_bytes];  // DANGER: May split UTF-8!

// 3. Don't leave orphans
items.remove_if(|i| matches!(i, ResponseItem::FunctionCall { .. }));
// Now outputs are orphaned!

// 4. Don't double-truncate
let truncated = truncate_text(&already_truncated, policy);
// Loses information about original size
```

### ✅ Do This Instead

```rust
// 1. Use approximate counting
for item in items {
    let tokens = approx_token_count(&item);  // FAST!
    if tokens > limit { truncate(item); }
}

// 2. Always use boundary-safe functions
let truncated = truncate_on_boundary(text, max_bytes);

// 3. Use normalization functions
remove_corresponding_for(&mut items, &item_to_remove);

// 4. Check for existing truncation
if !content.contains("truncated") {
    let truncated = truncate_text(content, policy);
}
```

---

## Conclusion

This guide provides a complete blueprint for implementing Codex's context management system. The key to success is:

1. **Start with token tracking** - Get accurate usage data
2. **Implement truncation** - Prevent individual items from bloating
3. **Add normalization** - Maintain conversation integrity
4. **Build auto-compaction** - Handle long conversations gracefully

By following this architecture, Hoosh will be able to handle arbitrarily long agentic workflows while staying within context limits and maintaining conversation quality.

---

## References

- **Codex Source Files:**
  - `codex-rs/core/src/context_manager/history.rs`
  - `codex-rs/core/src/truncate.rs`
  - `codex-rs/core/src/compact.rs`
  - `codex-rs/core/src/context_manager/normalize.rs`
  - `codex-rs/core/src/model_family.rs`
  - `codex-rs/core/src/config/mod.rs`
  - `codex-rs/protocol/src/protocol.rs`

- **Key Constants:**
  - `APPROX_BYTES_PER_TOKEN = 4`
  - `COMPACT_USER_MESSAGE_MAX_TOKENS = 20_000`
  - `effective_context_window_percent = 95`

---

**Document Version:** 1.0
**Date:** 2025-11-18
**Author:** Claude (Anthropic)
**Target:** Hoosh Coding Agent Implementation
