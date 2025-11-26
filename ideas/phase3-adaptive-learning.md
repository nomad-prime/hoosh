# Phase 3: Adaptive Learning & Optimization - Implementation Ticket

## Overview

Implement adaptive learning system that improves routing accuracy over time using telemetry feedback. Add cost tracking, de-escalation, and advanced optimization features.

**Prerequisites**: Phase 2 completed with telemetry collection  
**Duration**: 3-4 days  
**Goal**: Self-improving routing with cost visibility and advanced features

---

## Architecture Decisions

### Core Concepts

1. **Feedback Loop**: Learn from routing mistakes via telemetry
2. **Cost Tracking**: Measure actual costs per tier and session
3. **Adaptive Thresholds**: Auto-tune routing parameters
4. **De-escalation**: Drop back to cheaper models when safe
5. **Reference Set Expansion**: Automatically add validated examples

### Learning Strategy

```
Telemetry → Analysis → Adjustments → Improved Routing
    ↓           ↓            ↓              ↓
Sessions    Patterns    Thresholds    Better Accuracy
```

---

## Implementation Steps

### Step 1: Enhanced Telemetry Collection (2-3 hours)

**File**: `src/routing/telemetry.rs`

Enhance telemetry with more metadata:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub timestamp: String,
    pub initial_tier: ModelTier,
    pub final_tier: ModelTier,
    pub escalation_count: u32,
    pub message_length: usize,
    pub message_preview: String,
    pub routing_confidence: f32,
    pub session_id: String,
    pub routing_strategy: String,
    
    // NEW: Enhanced metadata
    pub task_completed_successfully: bool,
    pub user_satisfaction: Option<UserSatisfaction>,
    pub cost_estimate: Option<CostEstimate>,
    pub execution_time_ms: u64,
    pub tool_calls_count: usize,
    pub error_occurred: bool,
    pub auto_escalated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserSatisfaction {
    Positive,    // Task completed, no issues
    Neutral,     // Task completed with minor issues
    Negative,    // Task failed or needed escalation
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub estimated_cost_usd: f64,
    pub backend: String,
    pub model: String,
}

impl RoutingDecision {
    pub fn mark_completed(&mut self, success: bool, execution_time_ms: u64) {
        self.task_completed_successfully = success;
        self.execution_time_ms = execution_time_ms;
        
        // Infer satisfaction from success + escalation
        self.user_satisfaction = Some(match (success, self.escalation_count) {
            (true, 0) => UserSatisfaction::Positive,
            (true, _) => UserSatisfaction::Neutral,
            (false, _) => UserSatisfaction::Negative,
        });
    }

    pub fn add_cost_estimate(&mut self, estimate: CostEstimate) {
        self.cost_estimate = Some(estimate);
    }

    pub fn mark_auto_escalated(&mut self) {
        self.auto_escalated = true;
    }

    pub fn add_tool_calls(&mut self, count: usize) {
        self.tool_calls_count = count;
    }
}

/// Aggregate statistics from telemetry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingStatistics {
    pub total_sessions: usize,
    pub tier_distribution: HashMap<String, usize>,
    pub escalation_rate: f32,
    pub average_confidence: f32,
    pub accuracy_by_strategy: HashMap<String, f32>,
    pub cost_by_tier: HashMap<String, f64>,
    pub average_execution_time_ms: u64,
}

impl RoutingTelemetry {
    /// Analyze telemetry data and generate statistics
    pub fn analyze(&self) -> Result<RoutingStatistics> {
        let decisions = self.load_all_decisions()?;
        
        if decisions.is_empty() {
            return Ok(RoutingStatistics::default());
        }

        let total = decisions.len();
        
        // Tier distribution
        let mut tier_dist = HashMap::new();
        for decision in &decisions {
            *tier_dist.entry(decision.initial_tier.to_string()).or_insert(0) += 1;
        }

        // Escalation rate
        let escalated = decisions.iter().filter(|d| d.escalation_count > 0).count();
        let escalation_rate = escalated as f32 / total as f32;

        // Average confidence
        let avg_confidence = decisions.iter()
            .map(|d| d.routing_confidence)
            .sum::<f32>() / total as f32;

        // Accuracy by strategy
        let mut accuracy_by_strategy = HashMap::new();
        for strategy in ["conservative", "embedding", "hybrid"] {
            let strategy_decisions: Vec<_> = decisions.iter()
                .filter(|d| d.routing_strategy == strategy)
                .collect();
            
            if !strategy_decisions.is_empty() {
                let correct = strategy_decisions.iter()
                    .filter(|d| d.initial_tier == d.final_tier)
                    .count();
                let accuracy = correct as f32 / strategy_decisions.len() as f32;
                accuracy_by_strategy.insert(strategy.to_string(), accuracy);
            }
        }

        // Cost by tier
        let mut cost_by_tier = HashMap::new();
        for tier in ["small", "medium", "large"] {
            let tier_costs: Vec<f64> = decisions.iter()
                .filter(|d| d.initial_tier.to_string() == tier)
                .filter_map(|d| d.cost_estimate.as_ref())
                .map(|c| c.estimated_cost_usd)
                .collect();
            
            if !tier_costs.is_empty() {
                let avg_cost = tier_costs.iter().sum::<f64>() / tier_costs.len() as f64;
                cost_by_tier.insert(tier.to_string(), avg_cost);
            }
        }

        // Average execution time
        let avg_time = decisions.iter()
            .map(|d| d.execution_time_ms)
            .sum::<u64>() / total as u64;

        Ok(RoutingStatistics {
            total_sessions: total,
            tier_distribution: tier_dist,
            escalation_rate,
            average_confidence: avg_confidence,
            accuracy_by_strategy,
            cost_by_tier,
            average_execution_time_ms: avg_time,
        })
    }

    /// Load all decisions from telemetry log
    fn load_all_decisions(&self) -> Result<Vec<RoutingDecision>> {
        use std::fs::File;
        use std::io::{BufRead, BufReader};

        if !self.log_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);

        let mut decisions = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if let Ok(decision) = serde_json::from_str::<RoutingDecision>(&line) {
                decisions.push(decision);
            }
        }

        Ok(decisions)
    }

    /// Find misrouted sessions (initial tier != final tier)
    pub fn find_misrouted_sessions(&self) -> Result<Vec<RoutingDecision>> {
        let decisions = self.load_all_decisions()?;
        Ok(decisions.into_iter()
            .filter(|d| d.initial_tier != d.final_tier)
            .collect())
    }
}

impl Default for RoutingStatistics {
    fn default() -> Self {
        Self {
            total_sessions: 0,
            tier_distribution: HashMap::new(),
            escalation_rate: 0.0,
            average_confidence: 0.0,
            accuracy_by_strategy: HashMap::new(),
            cost_by_tier: HashMap::new(),
            average_execution_time_ms: 0,
        }
    }
}
```

**Acceptance Criteria**:
- [ ] Enhanced telemetry captures success/failure
- [ ] Cost estimates tracked
- [ ] Statistics calculation works
- [ ] Can identify misrouted sessions

---

### Step 2: Cost Tracking System (3-4 hours)

**File**: `src/backends/cost_tracker.rs`

```rust
use crate::routing::ModelTier;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Tracks LLM costs across sessions
pub struct CostTracker {
    /// Cost per 1M input tokens (USD) by backend/model
    input_costs: HashMap<String, f64>,
    /// Cost per 1M output tokens (USD) by backend/model
    output_costs: HashMap<String, f64>,
}

impl CostTracker {
    pub fn new() -> Self {
        let mut input_costs = HashMap::new();
        let mut output_costs = HashMap::new();

        // Anthropic pricing (as of Dec 2024)
        input_costs.insert("anthropic/claude-3-5-haiku-20241022".to_string(), 1.00);
        output_costs.insert("anthropic/claude-3-5-haiku-20241022".to_string(), 5.00);
        
        input_costs.insert("anthropic/claude-sonnet-4-20250514".to_string(), 3.00);
        output_costs.insert("anthropic/claude-sonnet-4-20250514".to_string(), 15.00);
        
        input_costs.insert("anthropic/claude-opus-4-20250514".to_string(), 15.00);
        output_costs.insert("anthropic/claude-opus-4-20250514".to_string(), 75.00);

        // OpenAI pricing
        input_costs.insert("openai/gpt-4-turbo".to_string(), 10.00);
        output_costs.insert("openai/gpt-4-turbo".to_string(), 30.00);

        input_costs.insert("openai/gpt-4o".to_string(), 2.50);
        output_costs.insert("openai/gpt-4o".to_string(), 10.00);

        // Together AI pricing (examples)
        input_costs.insert("together/meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo".to_string(), 0.18);
        output_costs.insert("together/meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo".to_string(), 0.18);

        Self {
            input_costs,
            output_costs,
        }
    }

    /// Calculate cost for a completion
    pub fn calculate_cost(
        &self,
        backend: &str,
        model: &str,
        input_tokens: usize,
        output_tokens: usize,
    ) -> Option<f64> {
        let key = format!("{}/{}", backend, model);
        
        let input_cost = self.input_costs.get(&key)?;
        let output_cost = self.output_costs.get(&key)?;

        let cost = (input_tokens as f64 / 1_000_000.0) * input_cost
                 + (output_tokens as f64 / 1_000_000.0) * output_cost;

        Some(cost)
    }

    /// Add custom pricing for a backend/model
    pub fn add_pricing(
        &mut self,
        backend: &str,
        model: &str,
        input_cost_per_1m: f64,
        output_cost_per_1m: f64,
    ) {
        let key = format!("{}/{}", backend, model);
        self.input_costs.insert(key.clone(), input_cost_per_1m);
        self.output_costs.insert(key, output_cost_per_1m);
    }
}

/// Session-level cost tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCosts {
    pub session_id: String,
    pub total_cost: f64,
    pub cost_by_tier: HashMap<String, f64>,
    pub token_usage: TokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
}

impl SessionCosts {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            total_cost: 0.0,
            cost_by_tier: HashMap::new(),
            token_usage: TokenUsage::default(),
        }
    }

    pub fn add_completion(
        &mut self,
        tier: ModelTier,
        cost: f64,
        input_tokens: usize,
        output_tokens: usize,
    ) {
        self.total_cost += cost;
        *self.cost_by_tier.entry(tier.to_string()).or_insert(0.0) += cost;
        self.token_usage.input_tokens += input_tokens;
        self.token_usage.output_tokens += output_tokens;
        self.token_usage.total_tokens += input_tokens + output_tokens;
    }

    /// Calculate savings from using cascade vs always using large model
    pub fn calculate_savings(&self, large_tier_cost: f64) -> f64 {
        let sessions = self.cost_by_tier.values().len() as f64;
        let hypothetical_cost = large_tier_cost * sessions;
        hypothetical_cost - self.total_cost
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_calculation() {
        let tracker = CostTracker::new();

        // Test Haiku cost
        let cost = tracker.calculate_cost(
            "anthropic",
            "claude-3-5-haiku-20241022",
            1_000_000,
            1_000_000,
        );
        assert_eq!(cost, Some(6.00)); // $1 input + $5 output

        // Test Sonnet cost
        let cost = tracker.calculate_cost(
            "anthropic",
            "claude-sonnet-4-20250514",
            1_000_000,
            1_000_000,
        );
        assert_eq!(cost, Some(18.00)); // $3 input + $15 output
    }

    #[test]
    fn test_session_costs() {
        let mut session = SessionCosts::new("test-123".to_string());

        session.add_completion(ModelTier::Small, 0.01, 1000, 500);
        session.add_completion(ModelTier::Medium, 0.05, 2000, 1000);

        assert_eq!(session.total_cost, 0.06);
        assert_eq!(session.token_usage.input_tokens, 3000);
        assert_eq!(session.token_usage.output_tokens, 1500);
    }

    #[test]
    fn test_savings_calculation() {
        let mut session = SessionCosts::new("test-123".to_string());

        // Used small and medium tiers
        session.add_completion(ModelTier::Small, 0.01, 1000, 500);
        session.add_completion(ModelTier::Medium, 0.05, 2000, 1000);

        // If we had used large tier twice at $0.20 each
        let savings = session.calculate_savings(0.20);
        assert_eq!(savings, 0.40 - 0.06); // $0.34 saved
    }
}
```

**Acceptance Criteria**:
- [ ] Cost tracker initialized with standard pricing
- [ ] Cost calculation accurate
- [ ] Session costs tracked
- [ ] Savings calculation works

---

### Step 3: Adaptive Router (4-5 hours)

**File**: `src/routing/adaptive_router.rs`

```rust
use crate::agent::Conversation;
use crate::routing::{EmbeddingRouter, ModelTier, RoutingStatistics, RoutingTelemetry};
use anyhow::Result;
use std::sync::{Arc, RwLock};

/// Router that adapts based on telemetry feedback
pub struct AdaptiveRouter {
    embedding_router: EmbeddingRouter,
    telemetry: Arc<RoutingTelemetry>,
    
    // Adaptive parameters
    thresholds: Arc<RwLock<AdaptiveThresholds>>,
    
    // Statistics cache
    stats_cache: Arc<RwLock<Option<RoutingStatistics>>>,
}

#[derive(Debug, Clone)]
struct AdaptiveThresholds {
    /// Confidence threshold for small tier (lower = more aggressive)
    small_confidence_threshold: f32,
    
    /// Confidence threshold for large tier (lower = less aggressive)
    large_confidence_threshold: f32,
    
    /// Length multiplier for heuristic routing
    length_weight: f32,
    
    /// Escalation penalty (increases threshold after escalation)
    escalation_penalty: f32,
}

impl Default for AdaptiveThresholds {
    fn default() -> Self {
        Self {
            small_confidence_threshold: 0.7,
            large_confidence_threshold: 0.8,
            length_weight: 1.0,
            escalation_penalty: 0.1,
        }
    }
}

impl AdaptiveRouter {
    pub fn new(telemetry: Arc<RoutingTelemetry>) -> Result<Self> {
        let embedding_router = EmbeddingRouter::new()?;
        
        Ok(Self {
            embedding_router,
            telemetry,
            thresholds: Arc::new(RwLock::new(AdaptiveThresholds::default())),
            stats_cache: Arc::new(RwLock::new(None)),
        })
    }

    pub fn suggest_tier(&self, conversation: &Conversation) -> ModelTier {
        // Get base suggestion from embedding router
        let base_tier = self.embedding_router.suggest_tier(conversation);
        let confidence = self.embedding_router.routing_confidence(conversation);

        // Apply adaptive adjustments
        let thresholds = self.thresholds.read().unwrap();
        
        // Adjust based on recent escalation rate
        let stats = self.get_cached_stats();
        let escalation_adjustment = if let Some(stats) = stats {
            if stats.escalation_rate > 0.3 {
                // High escalation rate - be more conservative (prefer higher tiers)
                ModelTier::Medium  // Bump small -> medium
            } else if stats.escalation_rate < 0.1 {
                // Low escalation rate - can be more aggressive (prefer lower tiers)
                base_tier  // Keep original
            } else {
                base_tier
            }
        } else {
            base_tier
        };

        // Apply confidence thresholds
        match (escalation_adjustment, confidence) {
            (ModelTier::Small, conf) if conf < thresholds.small_confidence_threshold => {
                ModelTier::Medium  // Not confident enough for small
            }
            (ModelTier::Large, conf) if conf < thresholds.large_confidence_threshold => {
                ModelTier::Medium  // Not confident enough for large
            }
            _ => escalation_adjustment,
        }
    }

    pub fn routing_confidence(&self, conversation: &Conversation) -> f32 {
        self.embedding_router.routing_confidence(conversation)
    }

    /// Update adaptive thresholds based on telemetry analysis
    pub fn update_thresholds(&self) -> Result<()> {
        let stats = self.telemetry.analyze()?;
        
        let mut thresholds = self.thresholds.write().unwrap();
        
        // Adjust based on escalation rate
        if stats.escalation_rate > 0.3 {
            // Too many escalations - be more conservative
            thresholds.small_confidence_threshold += 0.05;
            thresholds.large_confidence_threshold -= 0.05;
        } else if stats.escalation_rate < 0.1 {
            // Few escalations - can be more aggressive
            thresholds.small_confidence_threshold -= 0.02;
            thresholds.large_confidence_threshold += 0.02;
        }

        // Clamp thresholds to reasonable ranges
        thresholds.small_confidence_threshold = thresholds.small_confidence_threshold.clamp(0.5, 0.9);
        thresholds.large_confidence_threshold = thresholds.large_confidence_threshold.clamp(0.6, 0.95);

        // Update stats cache
        *self.stats_cache.write().unwrap() = Some(stats);

        Ok(())
    }

    /// Expand reference set from validated telemetry
    pub fn expand_reference_set(&mut self) -> Result<usize> {
        let decisions = self.telemetry.load_all_decisions()?;
        
        // Find high-quality examples:
        // - High confidence
        // - No escalation
        // - Successful completion
        let candidates: Vec<_> = decisions
            .into_iter()
            .filter(|d| {
                d.routing_confidence > 0.8
                    && d.escalation_count == 0
                    && d.task_completed_successfully
                    && d.message_length > 20  // Not too short
                    && d.message_length < 500  // Not too long
            })
            .collect();

        let mut added = 0;
        for decision in candidates.iter().take(50) {  // Limit growth
            // Reconstruct message from preview (or store full message in telemetry)
            if decision.message_preview.len() >= 20 {
                self.embedding_router.add_reference_task(
                    decision.message_preview.clone(),
                    decision.initial_tier,
                )?;
                added += 1;
            }
        }

        Ok(added)
    }

    fn get_cached_stats(&self) -> Option<RoutingStatistics> {
        self.stats_cache.read().unwrap().clone()
    }

    /// Get current adaptive thresholds for logging
    pub fn get_thresholds(&self) -> AdaptiveThresholds {
        self.thresholds.read().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adaptive_router_creation() {
        let telemetry = Arc::new(RoutingTelemetry::new(false).unwrap());
        let router = AdaptiveRouter::new(telemetry);
        assert!(router.is_ok());
    }

    #[test]
    fn test_threshold_adjustment() {
        let telemetry = Arc::new(RoutingTelemetry::new(false).unwrap());
        let router = AdaptiveRouter::new(telemetry).unwrap();

        let initial = router.get_thresholds();
        
        // Simulate high escalation rate
        // (In real usage, this would come from telemetry)
        // For testing, we just verify thresholds stay in bounds

        assert!(initial.small_confidence_threshold >= 0.5);
        assert!(initial.small_confidence_threshold <= 0.9);
    }
}
```

**Acceptance Criteria**:
- [ ] Adaptive router compiles
- [ ] Thresholds adjust based on escalation rate
- [ ] Reference set expands from telemetry
- [ ] Thresholds stay within reasonable bounds

---

### Step 4: De-escalation System (2-3 hours)

**File**: `src/agent/de_escalation.rs`

```rust
use crate::agent::Conversation;
use crate::routing::ModelTier;

/// Determines if it's safe to de-escalate to a lower tier
pub struct DeEscalationPolicy {
    /// Number of consecutive simple turns before de-escalation
    simple_turn_threshold: usize,
    
    /// Enable/disable de-escalation
    enabled: bool,
}

impl DeEscalationPolicy {
    pub fn new(enabled: bool) -> Self {
        Self {
            simple_turn_threshold: 3,
            enabled,
        }
    }

    /// Check if we should de-escalate based on recent conversation
    pub fn should_deescalate(
        &self,
        current_tier: ModelTier,
        conversation: &Conversation,
    ) -> Option<ModelTier> {
        if !self.enabled || current_tier == ModelTier::Small {
            return None;
        }

        // Analyze recent user messages
        let recent_messages: Vec<_> = conversation
            .messages
            .iter()
            .rev()
            .filter(|m| m.role == "user")
            .take(self.simple_turn_threshold)
            .collect();

        if recent_messages.len() < self.simple_turn_threshold {
            return None;  // Not enough history
        }

        // Check if all recent messages are simple
        let all_simple = recent_messages.iter().all(|msg| {
            if let Some(content) = &msg.content {
                self.is_simple_message(content)
            } else {
                false
            }
        });

        if all_simple {
            // De-escalate one tier
            match current_tier {
                ModelTier::Large => Some(ModelTier::Medium),
                ModelTier::Medium => Some(ModelTier::Small),
                ModelTier::Small => None,
            }
        } else {
            None
        }
    }

    fn is_simple_message(&self, content: &str) -> bool {
        let word_count = content.split_whitespace().count();
        let char_count = content.len();

        // Simple = short, no code blocks, single question
        word_count < 30
            && char_count < 200
            && !content.contains("```")
            && content.matches('?').count() <= 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Message;

    #[test]
    fn test_should_not_deescalate_without_history() {
        let policy = DeEscalationPolicy::new(true);
        let mut conv = Conversation::new();
        conv.add_user_message("Hi".to_string());

        assert_eq!(
            policy.should_deescalate(ModelTier::Large, &conv),
            None
        );
    }

    #[test]
    fn test_deescalate_after_simple_turns() {
        let policy = DeEscalationPolicy::new(true);
        let mut conv = Conversation::new();

        // Add 3 simple messages
        for _ in 0..3 {
            conv.add_user_message("OK".to_string());
            conv.add_assistant_message("Done".to_string());
        }

        assert_eq!(
            policy.should_deescalate(ModelTier::Large, &conv),
            Some(ModelTier::Medium)
        );
    }

    #[test]
    fn test_no_deescalate_with_complex_message() {
        let policy = DeEscalationPolicy::new(true);
        let mut conv = Conversation::new();

        conv.add_user_message("OK".to_string());
        conv.add_assistant_message("Done".to_string());
        conv.add_user_message("Now refactor the entire authentication system".to_string());

        assert_eq!(
            policy.should_deescalate(ModelTier::Large, &conv),
            None
        );
    }

    #[test]
    fn test_is_simple_message() {
        let policy = DeEscalationPolicy::new(true);

        assert!(policy.is_simple_message("What is this?"));
        assert!(policy.is_simple_message("OK"));
        assert!(policy.is_simple_message("Can you explain?"));

        assert!(!policy.is_simple_message("```rust\nfn main() {}\n```"));
        assert!(!policy.is_simple_message(&"word ".repeat(50)));
    }
}
```

Update `src/agent/core.rs` to use de-escalation:

```rust
use crate::agent::de_escalation::DeEscalationPolicy;

impl Agent {
    // Add to Agent struct:
    de_escalation_policy: Option<DeEscalationPolicy>,

    // Check for de-escalation opportunity before each turn:
    async fn check_deescalation(&mut self, conversation: &Conversation) -> Result<()> {
        if let Some(policy) = &self.de_escalation_policy {
            let current = *self.current_tier.read().unwrap();
            
            if let Some(target_tier) = policy.should_deescalate(current, conversation) {
                self.send_event(AgentEvent::Info(format!(
                    "De-escalating from {} to {} after simple interactions",
                    current,
                    target_tier
                )));

                // Swap to lower tier
                if let Some(factory) = &self.backend_factory {
                    let new_backend = factory.create_for_tier(target_tier)?;
                    new_backend.initialize().await?;
                    self.backend = Arc::from(new_backend);
                    
                    if let Ok(mut tier) = self.current_tier.write() {
                        *tier = target_tier;
                    }

                    self.send_event(AgentEvent::ModelDeescalated {
                        from: current.to_string(),
                        to: target_tier.to_string(),
                    });
                }
            }
        }

        Ok(())
    }
}
```

**Acceptance Criteria**:
- [ ] De-escalation policy implemented
- [ ] Simple message detection works
- [ ] Agent checks for de-escalation
- [ ] Events emitted for de-escalation

---

### Step 5: CLI Commands for Analysis (2-3 hours)

**File**: `src/cli/analyze.rs`

```rust
use crate::routing::RoutingTelemetry;
use anyhow::Result;

pub async fn handle_analyze_routing() -> Result<()> {
    let telemetry = RoutingTelemetry::new(true)?;
    let stats = telemetry.analyze()?;

    println!("=== Routing Statistics ===\n");
    println!("Total sessions: {}", stats.total_sessions);
    println!("\nTier Distribution:");
    for (tier, count) in &stats.tier_distribution {
        let percentage = (*count as f32 / stats.total_sessions as f32) * 100.0;
        println!("  {}: {} ({:.1}%)", tier, count, percentage);
    }

    println!("\nEscalation rate: {:.1}%", stats.escalation_rate * 100.0);
    println!("Average confidence: {:.1}%", stats.average_confidence * 100.0);

    println!("\nAccuracy by strategy:");
    for (strategy, accuracy) in &stats.accuracy_by_strategy {
        println!("  {}: {:.1}%", strategy, accuracy * 100.0);
    }

    if !stats.cost_by_tier.is_empty() {
        println!("\nAverage cost by tier:");
        for (tier, cost) in &stats.cost_by_tier {
            println!("  {}: ${:.4}", tier, cost);
        }
    }

    println!("\nAverage execution time: {}ms", stats.average_execution_time_ms);

    // Find misrouted sessions
    let misrouted = telemetry.find_misrouted_sessions()?;
    if !misrouted.is_empty() {
        println!("\n=== Misrouted Sessions ({}) ===", misrouted.len());
        for (i, decision) in misrouted.iter().take(10).enumerate() {
            println!(
                "\n{}. {} -> {} (conf: {:.1}%)",
                i + 1,
                decision.initial_tier,
                decision.final_tier,
                decision.routing_confidence * 100.0
            );
            println!("   Preview: {}", decision.message_preview);
        }
        
        if misrouted.len() > 10 {
            println!("\n... and {} more", misrouted.len() - 10);
        }
    }

    println!("\nTelemetry log: {:?}", telemetry.log_path());

    Ok(())
}

pub async fn handle_tune_routing() -> Result<()> {
    use crate::routing::AdaptiveRouter;
    use std::sync::Arc;

    let telemetry = Arc::new(RoutingTelemetry::new(true)?);
    let mut router = AdaptiveRouter::new(telemetry.clone())?;

    println!("Analyzing telemetry...");
    router.update_thresholds()?;

    let thresholds = router.get_thresholds();
    println!("\nUpdated thresholds:");
    println!("  Small confidence: {:.2}", thresholds.small_confidence_threshold);
    println!("  Large confidence: {:.2}", thresholds.large_confidence_threshold);

    println!("\nExpanding reference set from validated examples...");
    let added = router.expand_reference_set()?;
    println!("Added {} new reference examples", added);

    println!("\nRouting parameters optimized!");

    Ok(())
}
```

Update `src/cli/mod.rs`:

```rust
#[derive(Subcommand)]
pub enum Commands {
    // ... existing commands ...
    
    /// Analyze routing telemetry
    AnalyzeRouting,
    
    /// Tune routing parameters based on telemetry
    TuneRouting,
}

// In handle_command:
Commands::AnalyzeRouting => {
    analyze::handle_analyze_routing().await
}
Commands::TuneRouting => {
    analyze::handle_tune_routing().await
}
```

**Acceptance Criteria**:
- [ ] `hoosh analyze-routing` shows statistics
- [ ] `hoosh tune-routing` updates parameters
- [ ] Commands are user-friendly
- [ ] Error handling is robust

---

### Step 6: Configuration Updates (1 hour)

**File**: `src/config/mod.rs`

```rust
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CascadeConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    
    #[serde(default = "default_auto_escalate")]
    pub auto_escalate_on_error: bool,
    
    #[serde(default = "default_telemetry_enabled")]
    pub telemetry_enabled: bool,
    
    #[serde(default = "default_routing_strategy")]
    pub routing_strategy: String,
    
    // NEW: Adaptive learning
    #[serde(default = "default_adaptive_enabled")]
    pub adaptive_learning: bool,
    
    // NEW: De-escalation
    #[serde(default = "default_deescalation_enabled")]
    pub deescalation_enabled: bool,
    
    // NEW: Cost tracking
    #[serde(default = "default_cost_tracking")]
    pub cost_tracking_enabled: bool,
    
    #[serde(default = "default_small_tier")]
    pub small: ModelTier,
    
    #[serde(default = "default_medium_tier")]
    pub medium: ModelTier,
    
    #[serde(default = "default_large_tier")]
    pub large: ModelTier,
}

fn default_adaptive_enabled() -> bool {
    true
}

fn default_deescalation_enabled() -> bool {
    true
}

fn default_cost_tracking() -> bool {
    true
}
```

**File**: `example_config.toml`

```toml
[cascade]
enabled = false
auto_escalate_on_error = true
telemetry_enabled = true
routing_strategy = "hybrid"

# Phase 3 features
adaptive_learning = true  # Learn from telemetry to improve routing
deescalation_enabled = true  # Drop to cheaper models when safe
cost_tracking_enabled = true  # Track costs per session

[cascade.small]
backend = "anthropic"
model = "claude-3-5-haiku-20241022"

[cascade.medium]
backend = "anthropic"
model = "claude-sonnet-4-20250514"

[cascade.large]
backend = "anthropic"
model = "claude-opus-4-20250514"
```

**Acceptance Criteria**:
- [ ] Config supports Phase 3 features
- [ ] Defaults are sensible
- [ ] example_config.toml documented

---

### Step 7: Testing (3-4 hours)

**File**: `tests/adaptive_learning_test.rs`

```rust
use hoosh::routing::{AdaptiveRouter, RoutingTelemetry};
use std::sync::Arc;

#[test]
fn test_adaptive_router_threshold_updates() {
    let telemetry = Arc::new(RoutingTelemetry::new(false).unwrap());
    let router = AdaptiveRouter::new(telemetry).unwrap();

    let initial = router.get_thresholds();
    
    // Update based on (empty) telemetry
    router.update_thresholds().ok();
    
    let updated = router.get_thresholds();
    
    // Thresholds should stay in valid ranges
    assert!(updated.small_confidence_threshold >= 0.5);
    assert!(updated.small_confidence_threshold <= 0.9);
}

#[test]
fn test_reference_set_expansion() {
    let telemetry = Arc::new(RoutingTelemetry::new(false).unwrap());
    let mut router = AdaptiveRouter::new(telemetry).unwrap();

    let (small, medium, large) = router.reference_stats();
    let initial_count = small + medium + large;

    // Try to expand (will be 0 with empty telemetry)
    let added = router.expand_reference_set().unwrap();
    
    let (small, medium, large) = router.reference_stats();
    let new_count = small + medium + large;
    
    assert_eq!(new_count, initial_count + added);
}

#[tokio::test]
async fn test_cost_tracking_integration() {
    use hoosh::backends::cost_tracker::CostTracker;

    let tracker = CostTracker::new();

    // Simulate a session
    let cost = tracker.calculate_cost(
        "anthropic",
        "claude-3-5-haiku-20241022",
        10_000,
        5_000,
    );

    assert!(cost.is_some());
    assert!(cost.unwrap() > 0.0);
}

#[test]
fn test_deescalation_detection() {
    use hoosh::agent::de_escalation::DeEscalationPolicy;
    use hoosh::agent::Conversation;
    use hoosh::routing::ModelTier;

    let policy = DeEscalationPolicy::new(true);
    let mut conv = Conversation::new();

    // Add simple interactions
    for _ in 0..4 {
        conv.add_user_message("OK".to_string());
        conv.add_assistant_message("Done".to_string());
    }

    let result = policy.should_deescalate(ModelTier::Large, &conv);
    assert_eq!(result, Some(ModelTier::Medium));
}
```

**Manual Testing Workflow**:

```bash
# 1. Generate some telemetry
hoosh --cascade
# ... use for a few sessions ...

# 2. Analyze telemetry
hoosh analyze-routing

# 3. Tune parameters
hoosh tune-routing

# 4. Test with tuned parameters
hoosh --cascade
# ... verify improved routing ...

# 5. Check costs
hoosh analyze-routing  # Should show cost data
```

**Acceptance Criteria**:
- [ ] All tests pass
- [ ] Manual workflow completes successfully
- [ ] Telemetry analysis produces reasonable results
- [ ] Tuning improves accuracy
- [ ] Cost tracking shows data

---

### Step 8: Documentation (2-3 hours)

**File**: `docs/adaptive-learning.md`

```markdown
# Adaptive Learning System

## Overview

Phase 3 implements a feedback loop that continuously improves routing accuracy based on real usage data.

## How It Works

```
Session → Telemetry → Analysis → Adjustments → Better Routing
   ↓          ↓           ↓            ↓              ↓
Usage     Logging    Patterns    Thresholds    Higher Accuracy
```

### Learning Process

1. **Data Collection**: Every session logs routing decision + outcome
2. **Analysis**: Periodic analysis identifies patterns and mistakes
3. **Threshold Tuning**: Adjusts confidence thresholds based on escalation rate
4. **Reference Expansion**: Adds validated examples to training set
5. **Improved Routing**: Next sessions benefit from learned patterns

## Features

### Adaptive Thresholds

The system automatically adjusts routing confidence thresholds:

- **High escalation rate** (>30%) → Be more conservative, prefer higher tiers
- **Low escalation rate** (<10%) → Be more aggressive, prefer lower tiers
- **Balanced** (10-30%) → Maintain current thresholds

### Reference Set Expansion

High-quality sessions automatically expand the reference set:

**Criteria for addition**:
- High confidence (>80%)
- No escalation
- Successful completion
- Appropriate message length

**Growth control**: Limited to ~50 examples per tuning session to prevent overfitting

### Cost Tracking

Track actual costs per session and tier:

```
Session XYZ:
  Small tier: 3 completions, $0.012
  Medium tier: 2 completions, $0.085
  Total: $0.097
  
Savings vs always-large: $0.453 (82%)
```

### De-escalation

After consecutive simple interactions, system drops to cheaper models:

```
[Large tier] → "OK" → "Done" → "Thanks" → [Medium tier]
```

**Criteria**:
- 3+ consecutive simple messages (<30 words, no code)
- Current tier is Medium or Large
- Enabled in config

## CLI Commands

### Analyze Routing

```bash
hoosh analyze-routing
```

Shows:
- Tier distribution
- Escalation rate
- Accuracy by strategy
- Average costs
- Misrouted sessions

### Tune Routing

```bash
hoosh tune-routing
```

Performs:
- Threshold adjustment based on telemetry
- Reference set expansion from validated examples
- Parameter optimization

Run weekly or after significant usage.

## Configuration

```toml
[cascade]
adaptive_learning = true  # Enable adaptive system
deescalation_enabled = true  # Allow tier downgrades
cost_tracking_enabled = true  # Track costs
```

## Best Practices

### Initial Phase (Weeks 1-2)

- Use default thresholds
- Collect telemetry data
- Review misrouted sessions
- Don't tune yet (insufficient data)

### Optimization Phase (Week 3+)

- Run `hoosh analyze-routing` weekly
- Review top misrouted sessions
- Run `hoosh tune-routing` monthly
- Monitor accuracy improvements

### Maintenance

- Telemetry grows ~1MB per 1000 sessions
- Archive old telemetry quarterly
- Re-tune after major usage pattern changes

## Metrics

### Success Indicators

- Escalation rate: <15% (good), <10% (excellent)
- Routing accuracy: >85% (good), >90% (excellent)
- Cost savings: >50% vs always-large

### Warning Signs

- Escalation rate >30% → System too aggressive
- Accuracy <70% → Need more reference examples
- Cost savings <30% → Check tier distribution

## Troubleshooting

### High escalation rate

**Cause**: Router routing to small tier too aggressively

**Fix**:
```bash
hoosh tune-routing  # Will raise small confidence threshold
```

### Low accuracy

**Cause**: Insufficient reference examples

**Fix**:
1. Collect more telemetry (100+ sessions)
2. Run `hoosh tune-routing` to expand reference set
3. Consider adding domain-specific examples

### Costs not tracked

**Cause**: Backend pricing not configured

**Fix**: Add custom pricing in code or config:
```rust
cost_tracker.add_pricing("mybackend", "mymodel", 1.0, 5.0);
```

## Telemetry Privacy

All telemetry is **local only**:
- Stored in `~/.hoosh/routing_telemetry.jsonl`
- Never sent to external servers
- Contains message previews (first 100 chars)
- Can be disabled with `telemetry_enabled = false`
```

Update `README.md`:

```markdown
## Adaptive Learning (Phase 3)

Hoosh learns from your usage to continuously improve routing accuracy:

- **Automatic tuning**: Adjusts routing based on real outcomes
- **Reference expansion**: Adds validated examples to training set
- **Cost tracking**: Monitor savings from cascade system
- **De-escalation**: Drops to cheaper models when safe

### Usage

```bash
# Analyze routing performance
hoosh analyze-routing

# Tune routing parameters
hoosh tune-routing

# View cost savings
hoosh analyze-routing  # Includes cost breakdown
```

See [docs/adaptive-learning.md](docs/adaptive-learning.md) for details.
```

**Acceptance Criteria**:
- [ ] Adaptive learning documented
- [ ] CLI commands explained
- [ ] Best practices provided
- [ ] Troubleshooting guide complete

---

## Phase 3 Completion Checklist

### Code
- [ ] Enhanced telemetry implemented
- [ ] Cost tracking system working
- [ ] Adaptive router functional
- [ ] De-escalation system working
- [ ] CLI commands implemented
- [ ] All tests pass

### Functionality
- [ ] Thresholds adapt based on telemetry
- [ ] Reference set expands from validated sessions
- [ ] Costs tracked per session
- [ ] De-escalation triggers appropriately
- [ ] CLI analysis commands work

### Documentation
- [ ] Adaptive learning documented
- [ ] Cost tracking explained
- [ ] De-escalation documented
- [ ] CLI commands documented
- [ ] Best practices guide written

### Testing
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Manual workflow validated
- [ ] Cost tracking accurate
- [ ] Performance acceptable

---

## Success Metrics

After Phase 3:

1. **Self-improvement**: System accuracy increases over time
2. **Cost optimization**: 60-80% cost savings vs always-large
3. **Escalation rate**: <15% (down from 20% in Phase 1)
4. **Routing accuracy**: >90% (up from 85% in Phase 2)
5. **User visibility**: Clear cost/savings reporting

---

## Future Enhancements (Phase 4+)

- **Multi-agent learning**: Different reference sets per agent type
- **User feedback**: Explicit user ratings of routing quality
- **A/B testing**: Test routing strategies against each other
- **Cloud sync**: Optional telemetry sharing (opt-in)
- **Advanced features**: Hybrid model voting, ensemble routing
