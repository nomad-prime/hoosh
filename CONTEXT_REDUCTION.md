## HOOSH-010: Context Reduction via Automatic Summarization

### Description

Implement automatic context compression that detects when conversation history approaches token limits and uses existing
summarization function to compact older messages while preserving recent context.

### Concepts

**Token Pressure Detection**

- Estimate token count for current message history
- Trigger compression at configurable threshold (e.g., 80% of max tokens)
- Different models have different limits (GPT-4: 128k, Claude: 200k, etc.)

**Two-Phase Context**

- **Old messages**: First half of conversation, gets summarized
- **Recent messages**: Second half, stays intact for immediate context

**Summarization Strategy**

- Call existing `summarize()` function with old messages
- Insert summary as system message at start of context
- Append recent messages unchanged
- Result: compressed history maintains continuity

**Transparent Operation**

- No user intervention required
- No LLM awareness needed
- Happens automatically before each LLM call

### Acceptance Criteria

**AC1: Token Estimation**

- Can estimate token count for list of messages
- Handles different message types (system, user, assistant)
- Configurable per LLM backend

**AC2: Compression Trigger**

- Activates when token count exceeds threshold
- Threshold configurable (default: 0.8 * max_tokens)
- Does nothing if under threshold

**AC3: Message Splitting**

- Divides history into old/recent sections
- Recent section remains uncompressed
- Split point configurable (default: 50%)

**AC4: Summary Integration**

- Calls existing summarization function with old messages
- Creates system message containing summary
- Builds new context: [summary_message] + recent_messages

**AC5: Seamless Application**

- Runs before sending messages to LLM
- No changes to LLM prompts or behavior
- No user-facing changes to conversation flow

**AC6: Configuration**

- Max tokens per model
- Compression trigger threshold
- Recent context percentage to preserve

### Technical Notes

- Leverage existing `summarize()` function in './src/conversations/summarizer.rs'
- May need to pass summarizer instance to context manager
- Consider caching token estimates to avoid recalculation
- Handle edge case: what if summary itself is too long?
