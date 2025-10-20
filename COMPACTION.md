# Conversation History Compaction

## Problem

Long conversations consume excessive context tokens, eventually hitting limits and degrading performance. Old messages
continue polluting the context even when no longer relevant.

## Solution

Implement conversation history compaction as a **command** (`/compact`) with automatic prompting and manual execution
options. Use a separate summarization module for reusability.

## Design

### Architecture

**Three-module approach:**

1. **Summarization Module** (`src/conversations/summarizer.rs`)
    - Reusable LLM-based summarization logic
    - Can be used by compact command, future features, or other agents

2. **Compact Command** (`src/commands/compact_command.rs`)
    - User-facing `/compact` command
    - Handles argument parsing and execution

3. **Auto-prompt Logic** (in conversation manager)
    - Detects when compaction is beneficial
    - Prompts user with confirmation dialog

### Implementation Strategy: Auto-prompt + Manual

**Automatic prompting when threshold reached (~50 messages):**

```
┌─────────────────────────────────────────────┐
│ Conversation is getting long (52 messages)  │
│ Compact history to improve performance?     │
│                                             │
│ [Y] Yes, compact now                        │
│ [N] Not now                                 │
│ [V] Never ask again this session            │
└─────────────────────────────────────────────┘
```

**Manual execution anytime:**

```bash
/compact              # Use default (keep 15 recent)
/compact 20           # Keep 20 recent messages
```

## Module Breakdown

### 1. Summarization Module (`src/conversations/summarizer.rs`)

**Purpose:** Reusable LLM-based message summarization

```rust
pub struct MessageSummarizer {
    backend: Arc<dyn LLMBackend>,
}

impl MessageSummarizer {
    /// Summarize messages by making an LLM call with those messages as context
    pub async fn summarize(
        &self,
        messages: &[Message],
        focus_areas: Option<Vec<String>>,
    ) -> Result<String> {
        // The messages themselves are the context - we just need to ask for a summary!
        let summary_request = Message {
            role: MessageRole::User,
            content: self.build_summary_request(focus_areas),
        };

        // Make API call with: messages_to_summarize + summary_request
        let mut context_messages = messages.to_vec();
        context_messages.push(summary_request);

        let response = self.backend.complete(context_messages).await?;
        Ok(response.content)
    }

    fn build_summary_request(&self, focus_areas: Option<Vec<String>>) -> String {
        let mut request = String::from(
            "Summarize our conversation so far concisely. Focus on:\n\
             - Key decisions, configurations, and code changes\n\
             - Important context needed for future reference\n\
             - Unresolved issues or pending tasks\n\
             - Critical file paths, functions, or entities mentioned\n\n"
        );

        if let Some(areas) = focus_areas {
            request.push_str(&format!("Pay special attention to: {}\n\n", areas.join(", ")));
        }

        request.push_str(
            "Omit routine acknowledgments and redundant information.\n\
             Aim for 70% compression while preserving semantic value.\n\
             Provide only the summary, no preamble."
        );

        request
    }
}
```

**How it works:**

- Messages to summarize are passed as context to the API call
- We append a user message asking for a summary
- Claude sees the full conversation and summarizes it
- No need to format/serialize messages into the prompt!

**Why separate module?**

- Can be used by other features (export, context switching, agent handoffs)
- Testable in isolation
- Different agents might need different summarization strategies

### 2. Compact Command (`src/commands/compact_command.rs`)

**Purpose:** User-facing command for manual compaction

```rust
pub struct CompactCommand;

#[async_trait]
impl Command for CompactCommand {
    fn name(&self) -> &str { "compact" }

    fn description(&self) -> &str {
        "Compress old conversation history to save context"
    }

    fn aliases(&self) -> Vec<&str> {
        vec!["summarize", "compress"]
    }

    fn usage(&self) -> &str {
        "/compact [keep_recent] - Summarize old messages, keeping N recent (default: 15)"
    }

    async fn execute(
        &self,
        args: Vec<String>,
        context: &mut CommandContext,
    ) -> Result<CommandResult> {
        let keep_recent = args.get(0)
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(15);

        let messages = context.conversation.messages();

        if messages.len() < 30 {
            return Ok(CommandResult::info(
                "Conversation too short to compact (< 30 messages)"
            ));
        }

        // Use MessageSummarizer
        let summarizer = MessageSummarizer::new(context.backend.clone());
        let old_messages = &messages[..messages.len() - keep_recent];
        let summary = summarizer.summarize(old_messages, None).await?;

        // Replace in conversation
        context.conversation.compact_with_summary(summary, keep_recent);

        Ok(CommandResult::success(
            format!("Compacted {} messages into summary", old_messages.len())
        ))
    }
}
```

### 3. Auto-prompt Logic (Conversation Manager)

**Purpose:** Detect when compaction is beneficial and prompt user

```rust
// In conversation manager or event loop
impl ConversationManager {
    fn should_prompt_compact(&self) -> bool {
        if self.compact_prompt_disabled {
            return false;
        }

        let message_count = self.conversation.messages().len();
        message_count > 50 && message_count % 10 == 0  // Every 10 messages after 50
    }

    async fn prompt_compact(&mut self) -> CompactChoice {
        // Show TUI dialog
        let dialog = CompactPromptDialog::new(message_count);
        dialog.show_and_wait().await
    }
}

enum CompactChoice {
    CompactNow,
    NotNow,
    NeverThisSession,
}
```

**Dialog appearance:**

```
┌────────────────────────────────────────────────┐
│ 📦 Conversation History Compaction             │
│                                                │
│ Your conversation has 52 messages and may be   │
│ affecting performance. Compact older messages  │
│ while preserving the most recent context?      │
│                                                │
│ [Y] Yes, compact now (keep last 15)            │
│ [N] Not now                                    │
│ [V] Never ask again this session               │
│                                                │
│ Tip: Use /compact anytime to compact manually  │
└────────────────────────────────────────────────┘
```

## Implementation Details

### Conversation Data Structure Changes

Add compaction support to `Conversation` struct:

```rust
// In src/conversations/mod.rs or conversation.rs
impl Conversation {
    pub fn compact_with_summary(&mut self, summary: String, keep_recent: usize) {
        let total = self.messages.len();

        if total <= keep_recent {
            return; // Nothing to compact
        }

        // Keep system message if present
        let system_msg = self.messages.iter()
            .find(|m| matches!(m.role, MessageRole::System))
            .cloned();

        // Get recent messages
        let recent: Vec<_> = self.messages
            .iter()
            .skip(total - keep_recent)
            .cloned()
            .collect();

        // Create summary message
        let summary_msg = Message {
            role: MessageRole::User,
            content: format!(
                "[CONVERSATION HISTORY SUMMARY - {} messages]\n\n{}\n\n[END SUMMARY - Recent conversation continues below]",
                total - keep_recent,
                summary
            ),
        };

        // Rebuild messages: system + summary + recent
        self.messages.clear();
        if let Some(sys) = system_msg {
            self.messages.push(sys);
        }
        self.messages.push(summary_msg);
        self.messages.extend(recent);
    }

    pub fn is_compacted(&self) -> bool {
        self.messages.iter().any(|m|
            m.content.starts_with("[CONVERSATION HISTORY SUMMARY")
        )
    }
}
```

### Registration

Add to `src/commands/register.rs`:

```rust
use super::compact_command::CompactCommand;

pub fn register_default_commands(registry: &mut CommandRegistry) -> Result<()> {
    // ... existing commands ...
    registry.register(Arc::new(CompactCommand))?;
    Ok(())
}
```

Add to `src/commands/mod.rs`:

```rust
mod compact_command;
// ... exports ...
```

### Prompt Timing

**When to show auto-prompt:**

- Message count reaches 50, 60, 70, etc. (every 10 messages)
- Check happens after user sends message (before API call)
- Don't interrupt during tool execution
- Skip if already compacted in last 20 messages

**State tracking:**

```rust
pub struct ConversationState {
    compact_prompt_disabled: bool,
    last_compact_at_message: usize,
    // ... other state ...
}
```

## Key Design Decisions

### Thresholds

- **Auto-prompt trigger**: 50 messages initially, then every 10 messages
- **Minimum to compact**: 30 messages (prevents compacting very short conversations)
- **Keep recent**: Default 15 messages, user-configurable via command arg
- **Re-prompt spacing**: Don't prompt again until 20+ new messages since last compact

### Compression Strategy

- **Target ratio**: 70% compression (100 old messages → ~30 message summary)
- **Model for summarization**: Same model as conversation (for consistency)
- **Focus areas**: Automatically detect based on conversation content
    - If code-heavy: emphasize file paths, function names, errors
    - If planning: emphasize decisions, task breakdowns
    - If Q&A: emphasize key facts and conclusions

### User Experience

- **Manual override**: `/compact` always available, regardless of auto-prompt state
- **Never ask again**: Only for current session, resets on restart
- **Visual feedback**: Show compaction status in TUI
    - Before: `[52 messages]`
    - After: `[Summary + 15 messages]` or `[📦 Compacted]`
- **Transparency**: User can see what was summarized (maybe `/compact --preview`?)

### Summary Message Format

```
[CONVERSATION HISTORY SUMMARY - 37 messages]

Key Context:
- Implemented /save, /load, /list commands for conversation persistence
- Fixed serialization bug in ConversationStore where timestamps weren't properly formatted
- Created unit tests for all commands in tests/commands/
- Files modified: src/commands/{save,load,list}_command.rs, src/conversations/store.rs
- Outstanding issue: Need to add error handling for corrupted conversation files

[END SUMMARY - Recent conversation continues below]
```

## Testing Considerations

- Test with conversations of varying lengths (50, 100, 200+ messages)
- Verify important context is preserved after compaction
- Check that recent messages remain untouched
- Ensure compacted conversations still produce coherent responses
- Measure token savings achieved

## What to Preserve in Summaries

**Must preserve:**

- Tool calls made and their outcomes
- Configuration changes
- File paths, function names, important entities
- Decisions made
- Errors encountered and resolutions
- Unresolved issues or TODOs
- Key code snippets or algorithms discussed
- Important context dependencies

**Should omit:**

- Routine acknowledgments ("Sure, I'll help with that")
- Redundant explanations
- Temporary debugging output
- Repetitive back-and-forth
- Verbose explanations that can be condensed
- Social niceties and chitchat

**Summarization hints for LLM:**

```
Focus on ACTIONABLE information:
- What was decided or configured
- What code/files were changed
- What errors occurred and how they were fixed
- What remains to be done

Use bullet points for key facts.
Use a "Context" section for dependencies.
Keep it concise but information-dense.
```

## Testing Strategy

### Unit Tests

**Summarization module:**

```rust
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_summarize_code_conversation() {
        // Test summarizing a conversation with code changes
    }

    #[tokio::test]
    async fn test_summarize_preserves_critical_info() {
        // Verify file paths, decisions are preserved
    }
}
```

**Compact command:**

```rust
#[tokio::test]
async fn test_compact_command_execution() {
    // Test command with various arguments
}

#[tokio::test]
async fn test_compact_rejects_short_conversation() {
    // Should reject conversations < 30 messages
}
```

### Integration Tests

1. **Long conversation flow:**
    - Create 60+ message conversation
    - Verify auto-prompt appears
    - Execute compaction
    - Verify message count reduced
    - Verify recent messages preserved exactly
    - Verify summary is coherent

2. **Manual compact:**
    - Execute `/compact` before threshold
    - Execute with custom keep_recent value
    - Verify error handling for invalid args

3. **Never ask again:**
    - Select "never ask" in prompt
    - Generate more messages
    - Verify prompt doesn't reappear
    - Verify manual `/compact` still works

### Quality Tests

**Summary quality checks:**

- Does summary capture key decisions?
- Are file paths and entities preserved?
- Is it actually ~70% shorter?
- Can conversation continue naturally after compaction?
- Do follow-up questions work with summarized context?

## Future Enhancements

### Progressive Compaction

For very long conversations (200+ messages):

```
[SUMMARY 1-50] → [SUMMARY 51-100] → [SUMMARY 101-150] → [Recent 15]
```

Or even compress the summaries:

```
[META-SUMMARY of summaries 1-150] → [Recent 15]
```

### Semantic Chunking

Group related message sequences before summarizing:

```
[Planning phase] → [Implementation phase] → [Debugging phase] → [Recent]
```

### Smart Focus Detection

Automatically detect conversation type and adjust focus:

- Code review → emphasize critique points
- Feature implementation → emphasize architecture decisions
- Debugging → emphasize error patterns and solutions

### Summary Metadata

Track compaction history:

```rust
struct CompactionMetadata {
    original_message_count: usize,
    compacted_at: DateTime,
    keep_recent: usize,
    compression_ratio: f64,
}
```

### Export with Expansion

When exporting conversation, optionally include full uncompacted history:

```
/export --include-full-history
```

### Undo Compaction

Keep last compacted state in memory for quick undo:

```
/compact undo  # Restore pre-compaction state
```
