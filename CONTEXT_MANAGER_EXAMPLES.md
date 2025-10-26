# Context Manager - Real-World Examples

This document provides practical examples of using the Context Manager in different scenarios.

## Example 1: Default Setup (Recommended)

The simplest way to use context compression with sensible defaults:

```rust
use hoosh::backends::LlmBackend;
use hoosh::conversations::{ContextManager, MessageSummarizer};
use std::sync::Arc;

// Initialize backend and summarizer
let backend: Arc<dyn LlmBackend> = /* create backend */;
let summarizer = Arc::new(MessageSummarizer::new(backend.clone()));

// Create context manager with defaults (128k tokens, 80% threshold)
let context_manager = Arc::new(
    ContextManager::with_default_config(summarizer)
);

// Attach to handler
let handler = ConversationHandler::new(backend, tools, executor)
    .with_context_manager(context_manager);

// Use normally - compression happens automatically
handler.handle_turn(&mut conversation).await?;
```

**What happens:**
- Monitors token count before each LLM call
- If tokens > 102,400 (80% of 128k), triggers compression
- Summarizes first 50% of messages
- Keeps recent 50% intact
- Continues conversation seamlessly

---

## Example 2: Custom Configuration for Different Models

Different models have different token limits. Configure appropriately:

```rust
use hoosh::conversations::ContextManagerConfig;

// For Claude 3 (200k context)
let claude_config = ContextManagerConfig {
    max_tokens: 200_000,
    compression_threshold: 0.80,
    preserve_recent_percentage: 0.50,
};

// For GPT-4 Turbo (128k context)
let gpt4_config = ContextManagerConfig {
    max_tokens: 128_000,
    compression_threshold: 0.80,
    preserve_recent_percentage: 0.50,
};

// For GPT-3.5 (4k context) - needs more aggressive compression
let gpt35_config = ContextManagerConfig {
    max_tokens: 4_000,
    compression_threshold: 0.70,  // Trigger earlier
    preserve_recent_percentage: 0.60,  // Keep more recent
};

let context_manager = Arc::new(
    ContextManager::new(gpt35_config, summarizer)
);
```

---

## Example 3: Load Configuration from File

Persist settings across sessions:

```rust
use hoosh::config::AppConfig;

// Load from ~/.config/hoosh/config.toml
let app_config = AppConfig::load()?;
let cm_config = app_config.get_context_manager_config();

let context_manager = Arc::new(
    ContextManager::new(cm_config, summarizer)
);

// Later: Update and save
let mut app_config = AppConfig::load()?;
app_config.set_context_manager_config(ContextManagerConfig {
    max_tokens: 100_000,
    compression_threshold: 0.75,
    preserve_recent_percentage: 0.40,
});
app_config.save()?;
```

**config.toml:**
```toml
[context_manager]
max_tokens = 100000
compression_threshold = 0.75
preserve_recent_percentage = 0.40
```

---

## Example 4: Monitor Compression Events

Track what's happening with rich event stream:

```rust
use hoosh::conversations::AgentEvent;
use tokio::sync::mpsc;

let (event_tx, mut event_rx) = mpsc::unbounded_channel();

let handler = ConversationHandler::new(backend, tools, executor)
    .with_context_manager(context_manager)
    .with_event_sender(event_tx);

// Spawn event listener
tokio::spawn(async move {
    while let Some(event) = event_rx.recv().await {
        match event {
            AgentEvent::TokenPressureWarning { current_pressure, threshold } => {
                eprintln!(
                    "⚠️  Token pressure warning: {:.0}% (threshold: {:.0}%)",
                    current_pressure * 100.0,
                    threshold * 100.0
                );
            }
            
            AgentEvent::ContextCompressionTriggered {
                original_message_count,
                token_pressure,
                ..
            } => {
                eprintln!(
                    "🔄 Compressing {} messages (pressure: {:.0}%)",
                    original_message_count,
                    token_pressure * 100.0
                );
            }
            
            AgentEvent::ContextCompressionComplete { summary_length } => {
                eprintln!(
                    "✅ Compression complete: {} messages summarized",
                    summary_length
                );
            }
            
            AgentEvent::ContextCompressionError { error } => {
                eprintln!("❌ Compression error: {}", error);
                // Continues without compression
            }
            
            _ => {}
        }
    }
});

// Now use handler - events will be logged
handler.handle_turn(&mut conversation).await?;
```

**Output:**
```
⚠️  Token pressure warning: 75% (threshold: 80%)
🔄 Compressing 50 messages (pressure: 82%)
✅ Compression complete: 25 messages summarized
```

---

## Example 5: Manual Compression Check

Sometimes you want to check pressure without automatic compression:

```rust
let context_manager = ContextManager::with_default_config(summarizer);

// Check current state
let pressure = context_manager.get_token_pressure(&conversation.messages);
let should_compress = context_manager.should_compress(&conversation.messages);

println!("Token pressure: {:.0}%", pressure * 100.0);
println!("Should compress: {}", should_compress);

// If needed, compress manually
if should_compress {
    match context_manager.compress_messages(&conversation.messages).await {
        Ok(compressed) => {
            println!("Compressed {} → {} messages",
                conversation.messages.len(),
                compressed.len()
            );
            conversation.messages = compressed;
        }
        Err(e) => {
            eprintln!("Compression failed: {}", e);
            // Continue with original messages
        }
    }
}
```

---

## Example 6: Progressive Compression Strategy

Compress gradually as pressure increases:

```rust
pub async fn handle_with_progressive_compression(
    handler: &ConversationHandler,
    context_manager: &ContextManager,
    conversation: &mut Conversation,
) -> Result<()> {
    let pressure = context_manager.get_token_pressure(&conversation.messages);
    
    match pressure {
        p if p < 0.70 => {
            // No action needed
        }
        p if p < 0.80 => {
            // Warning level - inform user
            eprintln!("⚠️  Conversation growing ({}% full)", (p * 100.0) as u32);
        }
        p if p < 0.90 => {
            // Compress once
            if let Ok(compressed) = context_manager
                .compress_messages(&conversation.messages)
                .await
            {
                eprintln!("Compressed to {} messages", compressed.len());
                conversation.messages = compressed;
            }
        }
        _ => {
            // Critical - compress aggressively
            if let Ok(compressed) = context_manager
                .compress_messages(&conversation.messages)
                .await
            {
                eprintln!("Aggressive compression: {} → {} messages",
                    conversation.messages.len(),
                    compressed.len()
                );
                conversation.messages = compressed;
            }
        }
    }
    
    handler.handle_turn(conversation).await
}
```

---

## Example 7: Long-Running Conversation Management

Handle very long conversations with periodic compression:

```rust
pub struct ConversationManager {
    handler: Arc<ConversationHandler>,
    context_manager: Arc<ContextManager>,
    max_turns_before_check: usize,
}

impl ConversationManager {
    pub async fn run_conversation(
        &self,
        mut conversation: Conversation,
        turns: usize,
    ) -> Result<()> {
        for turn in 0..turns {
            // Check compression periodically
            if turn % self.max_turns_before_check == 0 {
                let pressure = self.context_manager
                    .get_token_pressure(&conversation.messages);
                
                if pressure > 0.75 {
                    println!("Pressure at {:.0}%, compressing...", pressure * 100.0);
                    if let Ok(compressed) = self.context_manager
                        .compress_messages(&conversation.messages)
                        .await
                    {
                        conversation.messages = compressed;
                    }
                }
            }
            
            // Process turn
            self.handler.handle_turn(&mut conversation).await?;
            
            println!("Turn {}: {} messages, {:.0}% pressure",
                turn,
                conversation.messages.len(),
                self.context_manager.get_token_pressure(&conversation.messages) * 100.0
            );
        }
        
        Ok(())
    }
}

// Usage
let manager = ConversationManager {
    handler: Arc::new(handler),
    context_manager: Arc::new(context_manager),
    max_turns_before_check: 10,
};

manager.run_conversation(conversation, 100).await?;
```

**Output:**
```
Turn 0: 2 messages, 0% pressure
Turn 5: 12 messages, 2% pressure
Turn 10: 22 messages, 5% pressure
Pressure at 76%, compressing...
Turn 11: 14 messages, 3% pressure
Turn 20: 24 messages, 6% pressure
...
```

---

## Example 8: Testing with Mock Backend

Test compression without real LLM calls:

```rust
#[tokio::test]
async fn test_compression_flow() {
    use crate::backends::mock::MockBackend;
    
    let backend = Arc::new(MockBackend::new());
    let summarizer = Arc::new(MessageSummarizer::new(backend.clone()));
    
    let config = ContextManagerConfig {
        max_tokens: 500,  // Small for testing
        compression_threshold: 0.70,
        preserve_recent_percentage: 0.50,
    };
    
    let manager = ContextManager::new(config, summarizer);
    
    // Build test conversation
    let mut conversation = Conversation::new();
    for i in 0..50 {
        conversation.add_user_message(format!("Message {}: content", i));
        conversation.add_assistant_message(Some(format!("Response {}", i)), None);
    }
    
    // Verify compression
    assert!(manager.should_compress(&conversation.messages));
    let pressure = manager.get_token_pressure(&conversation.messages);
    assert!(pressure > 0.70);
    
    // Apply compression
    let compressed = manager
        .compress_messages(&conversation.messages)
        .await
        .unwrap();
    
    assert!(compressed.len() < conversation.messages.len());
    assert!(compressed.iter().any(|m| {
        m.content.as_ref()
            .map(|c| c.contains("CONTEXT COMPRESSION"))
            .unwrap_or(false)
    }));
}
```

---

## Example 9: Builder Pattern for Complex Setup

Chain multiple configurations:

```rust
let context_manager = Arc::new(
    ContextManager::new(
        ContextManagerConfig::default()
            .with_max_tokens(150_000)
            .with_threshold(0.75)
            .with_preserve_percentage(0.45),
        summarizer
    )
);

let handler = ConversationHandler::new(backend, tools, executor)
    .with_context_manager(context_manager)
    .with_event_sender(event_tx)
    .with_max_steps(1000);
```

---

## Example 10: Debugging Token Estimation

Understand how tokens are calculated:

```rust
use hoosh::conversations::TokenEstimator;

let msg = ConversationMessage {
    role: "user".to_string(),
    content: Some("This is a test message".to_string()),
    tool_calls: None,
    tool_call_id: None,
    name: None,
};

let tokens = TokenEstimator::estimate_tokens(&msg);
println!("Message tokens: {}", tokens);

// Estimate batch
let messages = vec![msg1, msg2, msg3];
let total = TokenEstimator::estimate_messages_tokens(&messages);
println!("Batch tokens: {}", total);
println!("Average per message: {}", total / messages.len());
```

**Output:**
```
Message tokens: 10
Batch tokens: 35
Average per message: 11
```

---

## Best Practices

### ✅ DO

1. **Use defaults first** - They're sensible for most cases
2. **Monitor events** - Listen to compression events for insights
3. **Configure per model** - Different models need different limits
4. **Persist config** - Save settings to config file
5. **Test compression** - Verify it works with your data
6. **Handle errors** - Compression failures don't break conversation

### ❌ DON'T

1. **Don't set max_tokens too low** - Defeats the purpose
2. **Don't disable compression** - Let it work automatically
3. **Don't ignore warnings** - Token pressure indicates issues
4. **Don't compress too aggressively** - Lose recent context
5. **Don't modify messages manually** - Use the API

---

## Troubleshooting

### "Compression not triggering"
- Check `max_tokens` is reasonable
- Verify `compression_threshold` (should be 0.0-1.0)
- Monitor token pressure with `get_token_pressure()`

### "Compression too slow"
- Summarization takes time (it's an LLM call)
- This is expected and transparent
- Consider async event monitoring

### "Lost important context"
- Increase `preserve_recent_percentage`
- Reduce `max_tokens` to trigger earlier
- Monitor with events to see what's being summarized

### "Compression not working"
- Ensure `context_manager` is attached to handler
- Check `with_context_manager()` was called
- Verify `ContextManager` is Arc-wrapped

---

## Performance Tips

1. **Token estimation is fast** - O(n) with small constant
2. **Compression is async** - Doesn't block UI
3. **Summarization is the bottleneck** - LLM call time
4. **Memory improves** - Summary replaces old messages
5. **No re-compression** - Once compressed, stays compressed

---

## Summary

The Context Manager provides:
- ✅ Automatic token monitoring
- ✅ Transparent compression
- ✅ Rich event system
- ✅ Flexible configuration
- ✅ Production-ready implementation

Use it to enable longer conversations while maintaining context quality!
