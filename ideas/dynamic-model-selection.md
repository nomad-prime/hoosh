# Dynamic Model Selection for Hoosh

## Overview

One of the main advantages of hoosh is its multi-LLM backend capability. This document explores options for adding dynamic model selection based on task complexity, allowing hoosh to automatically choose the most appropriate model (simple → complex, small → large) for each task.

---

## Current Architecture Summary

### Multi-LLM Backend Support
Hoosh currently supports 5 backend types:
1. **Anthropic** - Claude Sonnet, Claude Opus
2. **OpenAI Compatible** - GPT-4, GPT-4-turbo, supports Groq
3. **Together AI** - 200+ open source models
4. **Ollama** - Local models for offline operation
5. **Mock** - For testing

### Current Model Selection
- **CLI-level**: `--backend` flag or `default_backend` in config
- **Task-level**: Optional `model` parameter in task tool
- **No automatic routing**: Manual selection only

### Key Extension Points
1. `src/cli/agent.rs:20-22` - Before `create_backend()`
2. `src/tools/task_tool.rs:41-45` - Before task creation
3. `src/agent/core.rs` - Agent execution loop
4. `src/backends/strategy.rs` - Retry logic

---

## Option 1: LLM-as-Router (Self-Selecting)

### How it works
- Add a new tool called `ChangeModel` or `UpgradeModel`/`DowngradeModel`
- The LLM decides when it needs more/less capability
- Tool execution swaps the backend mid-conversation

### Implementation points
- Create tool in `/src/tools/model_selection_tool.rs`
- Add method to `Agent` (src/agent/core.rs:28) to swap backend
- Store backend factory in `SessionConfig` (src/session.rs:42)

### Pros
- ✅ Context-aware decisions (LLM knows task complexity)
- ✅ Self-correcting (can upgrade if struggling)
- ✅ Minimal configuration needed
- ✅ Learns from experience in conversation

### Cons
- ❌ Adds tokens/latency for model switching decisions
- ❌ Smaller models may not know when to upgrade
- ❌ Requires tool call for every switch

### Example tool definition
```rust
{
  "name": "change_model",
  "description": "Switch to a different model based on task complexity",
  "input_schema": {
    "type": "object",
    "properties": {
      "target_model": {
        "type": "string",
        "enum": ["small", "medium", "large"],
        "description": "small: gpt-4o-mini/claude-haiku for simple tasks, medium: gpt-4o/claude-sonnet for moderate complexity, large: o1/claude-opus for complex reasoning"
      },
      "reason": { "type": "string" }
    }
  }
}
```

---

## Option 2: Heuristic-Based Routing (Rule Engine)

### How it works
- Analyze request before sending to LLM
- Use pattern matching, keyword detection, length, etc.
- Route to appropriate model tier automatically

### Implementation points
- Create `ModelRouter` in `/src/routing/heuristic_router.rs`
- Hook into `cli/agent.rs:20` before `create_backend()`
- Define rules in config or code

### Heuristics to consider
```rust
struct RoutingHeuristics {
    // Simple indicators → small model
    - Short prompts (<200 chars)
    - Simple questions ("what is", "how do I")
    - Code formatting/syntax fixes
    - Documentation lookups

    // Complex indicators → large model
    - Multi-step reasoning required
    - Code architecture decisions
    - Debugging complex issues
    - Long context (>2000 tokens)

    // Tool usage patterns
    - No tools needed → smaller model
    - Complex tool sequences → larger model

    // Conversation history
    - Failed attempts → upgrade
    - Simple back-and-forth → downgrade
}
```

### Pros
- ✅ Zero latency (instant routing)
- ✅ No extra LLM calls
- ✅ Predictable behavior
- ✅ Easy to debug/tune

### Cons
- ❌ May misclassify edge cases
- ❌ Requires ongoing rule maintenance
- ❌ Can't adapt to nuance
- ❌ May over/under-estimate complexity

---

## Option 3: Cost-Aware Adaptive System

### How it works
- Start with cheaper model
- Monitor performance (errors, tool failures, retries)
- Automatically upgrade on failure, downgrade on success
- Track costs and optimize over time

### Implementation points
- Extend `LlmBackend` trait with performance metrics
- Add cost tracking to backends (some already have `pricing()`)
- Create `AdaptiveModelManager` in `/src/routing/adaptive_manager.rs`
- Hook into retry logic (src/backends/strategy.rs)

### Key metrics
```rust
struct PerformanceMetrics {
    tool_call_success_rate: f32,
    avg_turns_to_completion: u32,
    error_rate: f32,
    user_satisfaction_signals: Vec<Signal>,

    // Cost tracking
    total_cost: f64,
    cost_per_task: f64,

    // Upgrade triggers
    - 2+ retries on same request → upgrade
    - Tool call errors → upgrade
    - "I don't understand" → upgrade

    // Downgrade triggers
    - 5+ successful simple tasks → try downgrade
    - Low tool usage → downgrade
    - Short responses → downgrade
}
```

### Pros
- ✅ Self-optimizing
- ✅ Balances cost and capability
- ✅ Learns per-user patterns
- ✅ Graceful degradation

### Cons
- ❌ Complex to implement
- ❌ May waste tokens on failed attempts
- ❌ Requires telemetry infrastructure
- ❌ Delayed optimization (learns over time)

---

## Option 4: Embedding-Based Task Classifier

### How it works
- Embed user prompt
- Compare to labeled examples of simple/complex tasks
- Route based on similarity scores
- Can use small local model for classification

### Implementation points
- Add embedding client (OpenAI/Ollama embeddings)
- Create classifier in `/src/routing/embedding_classifier.rs`
- Train/configure with example tasks
- Hook into request pipeline

### Classification approach
```rust
struct TaskClassifier {
    embeddings: Arc<EmbeddingClient>,

    // Pre-labeled examples
    simple_tasks: Vec<(String, Embedding)>,
    complex_tasks: Vec<(String, Embedding)>,

    async fn classify(&self, prompt: &str) -> TaskComplexity {
        let prompt_emb = self.embeddings.embed(prompt).await?;

        let simple_sim = cosine_similarity(&prompt_emb, &self.simple_tasks);
        let complex_sim = cosine_similarity(&prompt_emb, &self.complex_tasks);

        if complex_sim > simple_sim + 0.1 {
            TaskComplexity::High
        } else if simple_sim > complex_sim + 0.1 {
            TaskComplexity::Low
        } else {
            TaskComplexity::Medium
        }
    }
}
```

### Pros
- ✅ ML-based, handles nuance
- ✅ Can train on your data
- ✅ Fast inference
- ✅ Improves with more examples

### Cons
- ❌ Requires embedding API or local model
- ❌ Needs labeled training data
- ❌ Additional dependency
- ❌ Cold start problem

---

## Option 5: Hybrid Cascade System ⭐ RECOMMENDED

### How it works
- **Always start with small model**
- Small model can explicitly invoke "escalate" tool
- Heuristics catch obvious complex cases upfront
- Cost-aware fallback if small model struggles

### Flow
```
User Request
    ↓
Heuristic Pre-filter (< 1ms)
    ↓
├─ Obviously Complex? → Large Model
└─ Unclear/Simple → Small Model
         ↓
    Executes Task
         ↓
    ├─ Success → Done ✓
    ├─ Explicit Escalation Tool → Large Model
    ├─ Error/Retry → Medium Model
    └─ Continued Failure → Large Model
```

### Implementation points
- Combine Option 1 (tool) + Option 2 (heuristics) + Option 3 (adaptive)
- Create `CascadeRouter` in `/src/routing/cascade_router.rs`
- Reuse existing retry logic (src/backends/strategy.rs)

### Pros
- ✅ Best cost/performance balance
- ✅ Multiple safety nets
- ✅ Fast common path
- ✅ Handles edge cases

### Cons
- ❌ Most complex to implement
- ❌ More moving parts to debug
- ❌ Requires tuning multiple systems

### Implementation Phases

**Phase 1: Basic Cascade (1-2 days)**
- Add model tier config to existing `AppConfig`
- Create simple heuristic router
- Add `escalate` tool for LLM self-selection
- Hook into `cli/agent.rs` and `agent/core.rs`

**Phase 2: Adaptive Layer (3-5 days)**
- Add performance tracking to `LlmBackend`
- Implement cost tracking
- Auto-upgrade on retry failures
- Add metrics dashboard

**Phase 3: ML Enhancement (optional)**
- Add embedding-based classifier
- Train on real hoosh usage data
- A/B test against heuristics

---

## Option 6: User-Driven Tiers (Enhanced Config)

### How it works
- Extend config with task→model mappings
- User defines routing rules in TOML
- Support regex patterns, keywords, context size
- Optional ML suggestions for config updates

### Config example
```toml
[routing]
default_tier = "medium"

[[routing.rules]]
pattern = "^(what|how|why|explain)"
model_tier = "small"
max_tokens = 2000

[[routing.rules]]
pattern = "design|architect|complex|debug"
model_tier = "large"

[[routing.rules]]
tools_required = ["bash", "edit"]
min_model_tier = "medium"

[routing.tiers]
small = { backend = "anthropic", model = "claude-3-haiku" }
medium = { backend = "anthropic", model = "claude-3-5-sonnet" }
large = { backend = "anthropic", model = "claude-opus" }
```

### Implementation points
- Extend `AppConfig` (src/config/mod.rs:83)
- Create `RoutingConfig` struct
- Pattern matcher in routing module
- CLI flag for rule overrides

### Pros
- ✅ User control and transparency
- ✅ No AI decision overhead
- ✅ Easy to customize per workflow
- ✅ Deterministic

### Cons
- ❌ Requires user expertise
- ❌ Manual rule maintenance
- ❌ Can't adapt automatically
- ❌ Initial setup burden

---

## Recommendation

**Start with Option 5 (Hybrid Cascade)** because:

1. **Leverages existing architecture** - hoosh already has retry logic, tool system, and multi-backend support
2. **Balances cost and capability** - starts cheap, escalates when needed
3. **Multiple safety mechanisms** - heuristics catch obvious cases, LLM can self-escalate, adaptive fallback
4. **Iterative implementation** - can start simple (heuristics + tool) and add adaptive layer later
5. **Best user experience** - fast for simple tasks, capable for complex ones

---

## Next Steps

When ready to implement:
1. Design the detailed architecture for the hybrid cascade system
2. Define model tiers and cost/capability matrix
3. Implement Phase 1 (basic cascade with heuristics + escalation tool)
4. Test with real workloads
5. Add Phase 2 (adaptive layer) based on learnings
6. Consider Phase 3 (ML enhancement) for advanced use cases

---

## Related Files

### Core Backend System
- `src/backends/mod.rs` - Trait definition
- `src/backends/backend_factory.rs` - Factory pattern
- `src/backends/anthropic.rs` - Anthropic implementation
- `src/backends/openai_compatible.rs` - OpenAI implementation
- `src/backends/together_ai.rs` - Together AI implementation
- `src/backends/ollama.rs` - Ollama implementation

### Configuration
- `src/config/mod.rs` - Config management
- `example_config.toml` - Example configuration

### Session & Agent
- `src/session.rs` - Session initialization
- `src/agent/core.rs` - Agent execution logic
- `src/agent/conversation.rs` - Conversation management

### Task Management
- `src/task_management/mod.rs` - Task definitions
- `src/task_management/task_manager.rs` - Task execution
- `src/tools/task_tool.rs` - Task tool implementation

### Error Handling
- `src/backends/llm_error.rs` - Error types
- `src/backends/strategy.rs` - Retry logic
- `src/backends/executor.rs` - Request executor

### CLI & Entry
- `src/main.rs` - Main entry point
- `src/cli/mod.rs` - CLI definition
- `src/cli/agent.rs` - Agent handler
