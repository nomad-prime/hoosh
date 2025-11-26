# Phase 2: Embedding-Based Routing - Implementation Ticket

## Overview

Replace conservative heuristics with semantic embedding-based routing for more accurate tier classification. Use local embedding models for fast, privacy-preserving semantic understanding.

**Prerequisites**: Phase 1 completed with telemetry data  
**Duration**: 2-3 days  
**Goal**: 85%+ routing accuracy with <50ms latency

---

## Architecture Decisions

### Why Embeddings?

**Problems with Phase 1 heuristics:**
- Length-based routing misses semantic complexity
- "Write a simple hello world app" vs "Write a distributed system" both ~30 words
- Cannot distinguish question types: "What is OAuth?" vs "How should we architect OAuth?"

**Embedding advantages:**
- Semantic understanding: similar meaning → similar vectors
- Context-aware: same words, different context → different vectors
- Language-agnostic patterns
- Fast: 10-20ms on CPU

### Model Selection

Use `fastembed` crate with BGE-Small-EN-v1.5:
- **Size**: ~130MB
- **Speed**: 10-20ms per embedding on CPU
- **Quality**: State-of-art for small models
- **License**: MIT (safe for commercial use)

### Reference Set Strategy

Start with ~100 hand-labeled examples, expand with telemetry:
- 30 small tier examples
- 40 medium tier examples  
- 30 large tier examples
- Continuously add from telemetry (validated samples)

---

## Implementation Steps

### Step 1: Add Dependencies (30 mins)

**File**: `Cargo.toml`

```toml
[dependencies]
# ... existing deps ...

# Embedding support
fastembed = "3.0"
ndarray = "0.15"

[dev-dependencies]
approx = "0.5"  # For testing similarity scores
```

**Acceptance Criteria**:
- [ ] Dependencies compile
- [ ] No version conflicts

---

### Step 2: Embedding Router Core (3-4 hours)

**File**: `src/routing/embedding_router.rs`

```rust
use crate::agent::Conversation;
use crate::routing::ModelTier;
use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::Arc;

/// Embedding-based router using semantic similarity
pub struct EmbeddingRouter {
    model: Arc<TextEmbedding>,
    reference_tasks: Vec<ReferenceTask>,
    k_neighbors: usize,
}

#[derive(Clone)]
struct ReferenceTask {
    embedding: Vec<f32>,
    tier: ModelTier,
    description: String,
}

impl EmbeddingRouter {
    /// Create new router with default model and reference tasks
    pub fn new() -> Result<Self> {
        Self::with_config(EmbeddingConfig::default())
    }

    /// Create router with custom configuration
    pub fn with_config(config: EmbeddingConfig) -> Result<Self> {
        // Initialize embedding model
        let model = TextEmbedding::try_new(InitOptions {
            model_name: EmbeddingModel::BGESmallENV15,
            show_download_progress: false,
            ..Default::default()
        })
        .context("Failed to initialize embedding model")?;

        let model = Arc::new(model);

        // Load reference tasks
        let reference_tasks = Self::load_reference_tasks(&model)?;

        Ok(Self {
            model,
            reference_tasks,
            k_neighbors: config.k_neighbors,
        })
    }

    /// Suggest tier based on semantic similarity to reference tasks
    pub fn suggest_tier(&self, conversation: &Conversation) -> ModelTier {
        let last_user_msg = conversation
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .and_then(|m| m.content.as_ref());

        let Some(msg) = last_user_msg else {
            return ModelTier::Medium;
        };

        // Embed the user message
        let embedding = match self.embed_text(msg) {
            Ok(emb) => emb,
            Err(_) => return ModelTier::Medium, // Fallback on error
        };

        // Find k-nearest neighbors
        let mut similarities: Vec<(f32, ModelTier)> = self
            .reference_tasks
            .iter()
            .map(|ref_task| {
                let sim = cosine_similarity(&embedding, &ref_task.embedding);
                (sim, ref_task.tier)
            })
            .collect();

        // Sort by similarity (descending)
        similarities.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Take top k
        let top_k: Vec<_> = similarities.iter().take(self.k_neighbors).collect();

        // Majority vote with similarity weighting
        self.weighted_vote(&top_k)
    }

    /// Calculate confidence in routing decision
    pub fn routing_confidence(&self, conversation: &Conversation) -> f32 {
        let last_user_msg = conversation
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .and_then(|m| m.content.as_ref());

        let Some(msg) = last_user_msg else {
            return 0.5;
        };

        let embedding = match self.embed_text(msg) {
            Ok(emb) => emb,
            Err(_) => return 0.5,
        };

        // Calculate similarities
        let similarities: Vec<f32> = self
            .reference_tasks
            .iter()
            .map(|ref_task| cosine_similarity(&embedding, &ref_task.embedding))
            .collect();

        // High confidence if top similarity is high
        // and top k neighbors agree
        let max_sim = similarities
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .copied()
            .unwrap_or(0.0);

        // Get top k similarities
        let mut top_sims = similarities.clone();
        top_sims.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let top_k_sims: Vec<f32> = top_sims.iter().take(self.k_neighbors).copied().collect();

        // Calculate variance in top k
        let mean: f32 = top_k_sims.iter().sum::<f32>() / top_k_sims.len() as f32;
        let variance: f32 = top_k_sims
            .iter()
            .map(|s| (s - mean).powi(2))
            .sum::<f32>()
            / top_k_sims.len() as f32;

        // High confidence = high max similarity + low variance
        let similarity_score = max_sim;
        let agreement_score = 1.0 - variance.sqrt();

        (similarity_score * 0.7 + agreement_score * 0.3).clamp(0.0, 1.0)
    }

    /// Embed text using the model
    fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self
            .model
            .embed(vec![text.to_string()], None)
            .context("Failed to generate embedding")?;

        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No embedding generated"))
    }

    /// Weighted majority vote based on similarity scores
    fn weighted_vote(&self, neighbors: &[(f32, ModelTier)]) -> ModelTier {
        let mut votes = std::collections::HashMap::new();

        for (similarity, tier) in neighbors {
            *votes.entry(tier).or_insert(0.0) += similarity;
        }

        votes
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(tier, _)| *tier)
            .unwrap_or(ModelTier::Medium)
    }

    /// Load reference tasks and pre-compute embeddings
    fn load_reference_tasks(model: &TextEmbedding) -> Result<Vec<ReferenceTask>> {
        let examples = Self::get_reference_examples();

        let mut reference_tasks = Vec::with_capacity(examples.len());

        // Batch embed all examples
        let descriptions: Vec<String> = examples.iter().map(|(desc, _)| (*desc).to_string()).collect();
        let embeddings = model
            .embed(descriptions, None)
            .context("Failed to embed reference examples")?;

        for (embedding, (description, tier)) in embeddings.into_iter().zip(examples.iter()) {
            reference_tasks.push(ReferenceTask {
                embedding,
                tier: *tier,
                description: (*description).to_string(),
            });
        }

        Ok(reference_tasks)
    }

    /// Get hand-labeled reference examples
    fn get_reference_examples() -> Vec<(&'static str, ModelTier)> {
        vec![
            // SMALL TIER - Simple, quick tasks
            ("What is Rust?", ModelTier::Small),
            ("Fix typo in README", ModelTier::Small),
            ("What does this error mean?", ModelTier::Small),
            ("Add a comment to this function", ModelTier::Small),
            ("How do I import a module?", ModelTier::Small),
            ("Show me the syntax for a for loop", ModelTier::Small),
            ("What's the difference between & and &mut?", ModelTier::Small),
            ("Explain what this line does", ModelTier::Small),
            ("Add a print statement for debugging", ModelTier::Small),
            ("Remove this unused variable", ModelTier::Small),
            ("Fix the indentation", ModelTier::Small),
            ("Rename this variable to be clearer", ModelTier::Small),
            ("What's the type of this expression?", ModelTier::Small),
            ("How do I convert String to &str?", ModelTier::Small),
            ("Add doc comment to this function", ModelTier::Small),
            ("Is this code idiomatic?", ModelTier::Small),
            ("Should this be mut or not?", ModelTier::Small),
            ("What's the lifetime here?", ModelTier::Small),
            ("Explain the ownership in this code", ModelTier::Small),
            ("Can you format this code?", ModelTier::Small),
            ("Add missing semicolon", ModelTier::Small),
            ("What crate provides this functionality?", ModelTier::Small),
            ("How do I read a file?", ModelTier::Small),
            ("What's the trait bound syntax?", ModelTier::Small),
            ("Explain async/await basics", ModelTier::Small),
            ("What does unwrap do?", ModelTier::Small),
            ("Should I use Option or Result?", ModelTier::Small),
            ("What's a closure?", ModelTier::Small),
            ("How do I iterate over a vector?", ModelTier::Small),
            ("What's the difference between Vec and array?", ModelTier::Small),

            // MEDIUM TIER - Standard development tasks
            ("Add error handling to file operations", ModelTier::Medium),
            ("Implement pagination for API endpoint", ModelTier::Medium),
            ("Create a new struct for user data", ModelTier::Medium),
            ("Write tests for this function", ModelTier::Medium),
            ("Add logging to this module", ModelTier::Medium),
            ("Implement Display trait for this type", ModelTier::Medium),
            ("Create a config parser from TOML", ModelTier::Medium),
            ("Add validation to user input", ModelTier::Medium),
            ("Implement retry logic for API calls", ModelTier::Medium),
            ("Write a CLI command handler", ModelTier::Medium),
            ("Add caching to database queries", ModelTier::Medium),
            ("Create a custom error type", ModelTier::Medium),
            ("Implement builder pattern for this struct", ModelTier::Medium),
            ("Add async support to this function", ModelTier::Medium),
            ("Write integration test for this module", ModelTier::Medium),
            ("Parse JSON response from API", ModelTier::Medium),
            ("Add rate limiting to HTTP client", ModelTier::Medium),
            ("Create middleware for authentication", ModelTier::Medium),
            ("Implement simple state machine", ModelTier::Medium),
            ("Add monitoring metrics to service", ModelTier::Medium),
            ("Write serialization code for this struct", ModelTier::Medium),
            ("Create database migration script", ModelTier::Medium),
            ("Add timeout handling to network calls", ModelTier::Medium),
            ("Implement simple caching strategy", ModelTier::Medium),
            ("Create utility functions for common operations", ModelTier::Medium),
            ("Add connection pooling to database", ModelTier::Medium),
            ("Write data validation rules", ModelTier::Medium),
            ("Implement basic search functionality", ModelTier::Medium),
            ("Add compression to file storage", ModelTier::Medium),
            ("Create event handler system", ModelTier::Medium),
            ("Write request/response mapping code", ModelTier::Medium),
            ("Add filtering to query results", ModelTier::Medium),
            ("Implement simple queue system", ModelTier::Medium),
            ("Create health check endpoint", ModelTier::Medium),
            ("Add graceful shutdown handling", ModelTier::Medium),
            ("Write batch processing logic", ModelTier::Medium),
            ("Implement simple notification system", ModelTier::Medium),
            ("Add request throttling", ModelTier::Medium),
            ("Create simple workflow engine", ModelTier::Medium),
            ("Write data transformation pipeline", ModelTier::Medium),

            // LARGE TIER - Complex architecture and design
            ("Design microservices architecture for our system", ModelTier::Large),
            ("Refactor authentication to use OAuth2 flow", ModelTier::Large),
            ("Migrate from sync to async throughout codebase", ModelTier::Large),
            ("Architect distributed caching strategy", ModelTier::Large),
            ("Design event sourcing system", ModelTier::Large),
            ("Refactor monolith into domain-driven modules", ModelTier::Large),
            ("Implement comprehensive error recovery strategy", ModelTier::Large),
            ("Design scalable data pipeline architecture", ModelTier::Large),
            ("Architect plugin system with dynamic loading", ModelTier::Large),
            ("Design consensus algorithm for distributed state", ModelTier::Large),
            ("Implement zero-downtime deployment strategy", ModelTier::Large),
            ("Architect multi-tenant isolation system", ModelTier::Large),
            ("Design comprehensive testing strategy", ModelTier::Large),
            ("Refactor for horizontal scalability", ModelTier::Large),
            ("Implement sophisticated caching hierarchy", ModelTier::Large),
            ("Design real-time data synchronization system", ModelTier::Large),
            ("Architect security model for sensitive data", ModelTier::Large),
            ("Implement distributed transaction coordinator", ModelTier::Large),
            ("Design fault-tolerant message queue system", ModelTier::Large),
            ("Architect CI/CD pipeline with blue-green deployment", ModelTier::Large),
            ("Implement sophisticated rate limiting strategy", ModelTier::Large),
            ("Design database sharding strategy", ModelTier::Large),
            ("Architect API gateway with service mesh", ModelTier::Large),
            ("Implement advanced monitoring and observability", ModelTier::Large),
            ("Design saga pattern for distributed workflows", ModelTier::Large),
            ("Architect event-driven microservices", ModelTier::Large),
            ("Implement sophisticated authorization system", ModelTier::Large),
            ("Design multi-region replication strategy", ModelTier::Large),
            ("Architect performance optimization strategy", ModelTier::Large),
            ("Implement comprehensive disaster recovery plan", ModelTier::Large),
        ]
    }

    /// Add a new reference task from telemetry
    pub fn add_reference_task(
        &mut self,
        description: String,
        tier: ModelTier,
    ) -> Result<()> {
        let embedding = self.embed_text(&description)?;

        self.reference_tasks.push(ReferenceTask {
            embedding,
            tier,
            description,
        });

        Ok(())
    }

    /// Get reference task count per tier
    pub fn reference_stats(&self) -> (usize, usize, usize) {
        let small = self.reference_tasks.iter().filter(|t| t.tier == ModelTier::Small).count();
        let medium = self.reference_tasks.iter().filter(|t| t.tier == ModelTier::Medium).count();
        let large = self.reference_tasks.iter().filter(|t| t.tier == ModelTier::Large).count();
        (small, medium, large)
    }
}

/// Configuration for embedding router
pub struct EmbeddingConfig {
    pub k_neighbors: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            k_neighbors: 5,
        }
    }
}

/// Calculate cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Message;

    fn create_test_conversation(user_msg: &str) -> Conversation {
        let mut conv = Conversation::new();
        conv.add_user_message(user_msg.to_string());
        conv
    }

    #[test]
    fn test_embedding_router_creation() {
        let router = EmbeddingRouter::new();
        assert!(router.is_ok());
    }

    #[test]
    fn test_simple_question_similarity() {
        let router = EmbeddingRouter::new().unwrap();

        let conv1 = create_test_conversation("What is Rust?");
        let conv2 = create_test_conversation("Explain Rust to me");

        // Both should route to Small (semantically similar)
        assert_eq!(router.suggest_tier(&conv1), ModelTier::Small);
        assert_eq!(router.suggest_tier(&conv2), ModelTier::Small);
    }

    #[test]
    fn test_complex_architecture_task() {
        let router = EmbeddingRouter::new().unwrap();

        let conv = create_test_conversation(
            "Design a comprehensive microservices architecture for our authentication system"
        );

        assert_eq!(router.suggest_tier(&conv), ModelTier::Large);
    }

    #[test]
    fn test_medium_implementation_task() {
        let router = EmbeddingRouter::new().unwrap();

        let conv = create_test_conversation(
            "Implement error handling for the file reader module"
        );

        assert_eq!(router.suggest_tier(&conv), ModelTier::Medium);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let c = vec![0.0, 1.0, 0.0];

        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_confidence_calculation() {
        let router = EmbeddingRouter::new().unwrap();

        // Exact match to reference should have high confidence
        let conv = create_test_conversation("What is Rust?");
        let confidence = router.routing_confidence(&conv);
        assert!(confidence > 0.7);
    }

    #[test]
    fn test_reference_stats() {
        let router = EmbeddingRouter::new().unwrap();
        let (small, medium, large) = router.reference_stats();

        assert!(small > 0);
        assert!(medium > 0);
        assert!(large > 0);
        assert_eq!(small + medium + large, router.reference_tasks.len());
    }
}
```

**Acceptance Criteria**:
- [ ] Router initializes with embedding model
- [ ] Reference tasks are pre-embedded
- [ ] Cosine similarity calculation works
- [ ] K-NN classification works
- [ ] Confidence calculation is reasonable
- [ ] All tests pass

---

### Step 3: Hybrid Router (2 hours)

**File**: `src/routing/hybrid_router.rs`

```rust
use crate::agent::Conversation;
use crate::routing::{ConservativeRouter, EmbeddingRouter, ModelTier};
use anyhow::Result;

/// Hybrid router that uses fast heuristics for obvious cases,
/// embeddings for ambiguous cases
pub struct HybridRouter {
    conservative: ConservativeRouter,
    embedding: Option<EmbeddingRouter>,
    confidence_threshold: f32,
}

impl HybridRouter {
    /// Create hybrid router with embedding support
    pub fn new() -> Result<Self> {
        Self::with_threshold(0.7)
    }

    /// Create with custom confidence threshold
    pub fn with_threshold(threshold: f32) -> Result<Self> {
        let embedding = match EmbeddingRouter::new() {
            Ok(router) => Some(router),
            Err(e) => {
                eprintln!("Warning: Failed to initialize embedding router: {}", e);
                eprintln!("Falling back to conservative routing only");
                None
            }
        };

        Ok(Self {
            conservative: ConservativeRouter::new(),
            embedding,
            confidence_threshold: threshold,
        })
    }

    /// Suggest tier using hybrid approach
    pub fn suggest_tier(&self, conversation: &Conversation) -> ModelTier {
        // First, try fast heuristic check
        let heuristic_tier = self.conservative.suggest_tier(conversation);
        let heuristic_confidence = self.conservative.routing_confidence(conversation);

        // If heuristic is confident, use it
        if heuristic_confidence >= self.confidence_threshold {
            return heuristic_tier;
        }

        // Otherwise, fall back to embedding-based routing
        if let Some(embedding_router) = &self.embedding {
            embedding_router.suggest_tier(conversation)
        } else {
            // No embedding router available, use heuristic
            heuristic_tier
        }
    }

    /// Calculate confidence (uses embedding if available and heuristic uncertain)
    pub fn routing_confidence(&self, conversation: &Conversation) -> f32 {
        let heuristic_confidence = self.conservative.routing_confidence(conversation);

        if heuristic_confidence >= self.confidence_threshold {
            return heuristic_confidence;
        }

        // Use embedding confidence if available
        if let Some(embedding_router) = &self.embedding {
            embedding_router.routing_confidence(conversation)
        } else {
            heuristic_confidence
        }
    }

    /// Get routing strategy used for telemetry
    pub fn routing_strategy(&self, conversation: &Conversation) -> &'static str {
        let heuristic_confidence = self.conservative.routing_confidence(conversation);

        if heuristic_confidence >= self.confidence_threshold {
            "heuristic"
        } else if self.embedding.is_some() {
            "embedding"
        } else {
            "heuristic_fallback"
        }
    }
}

impl Default for HybridRouter {
    fn default() -> Self {
        Self::new().expect("Failed to create hybrid router")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Message;

    fn create_test_conversation(user_msg: &str) -> Conversation {
        let mut conv = Conversation::new();
        conv.add_user_message(user_msg.to_string());
        conv
    }

    #[test]
    fn test_hybrid_router_creation() {
        let router = HybridRouter::new();
        assert!(router.is_ok());
    }

    #[test]
    fn test_obvious_small_uses_heuristic() {
        let router = HybridRouter::new().unwrap();
        let conv = create_test_conversation("Hi");

        assert_eq!(router.suggest_tier(&conv), ModelTier::Small);
        assert_eq!(router.routing_strategy(&conv), "heuristic");
    }

    #[test]
    fn test_ambiguous_uses_embedding() {
        let router = HybridRouter::new().unwrap();
        let conv = create_test_conversation(
            "Can you help me with authentication?"
        );

        let strategy = router.routing_strategy(&conv);
        // Should use embedding for ambiguous case
        if router.embedding.is_some() {
            assert_eq!(strategy, "embedding");
        }
    }
}
```

Update `src/routing/mod.rs`:

```rust
pub mod conservative_router;
pub mod embedding_router;
pub mod hybrid_router;
pub mod model_tier;
pub mod telemetry;

pub use conservative_router::ConservativeRouter;
pub use embedding_router::EmbeddingRouter;
pub use hybrid_router::HybridRouter;
pub use model_tier::ModelTier;
pub use telemetry::{RoutingDecision, RoutingTelemetry};
```

**Acceptance Criteria**:
- [ ] Hybrid router compiles
- [ ] Falls back gracefully if embedding fails
- [ ] Uses heuristic for high-confidence cases
- [ ] Uses embedding for ambiguous cases
- [ ] Tests pass

---

### Step 4: Configuration Updates (1 hour)

**File**: `src/config/mod.rs`

Add routing strategy config:

```rust
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CascadeConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    
    #[serde(default = "default_auto_escalate")]
    pub auto_escalate_on_error: bool,
    
    #[serde(default = "default_telemetry_enabled")]
    pub telemetry_enabled: bool,
    
    // NEW: Routing strategy
    #[serde(default = "default_routing_strategy")]
    pub routing_strategy: String,  // "conservative", "embedding", or "hybrid"
    
    #[serde(default = "default_small_tier")]
    pub small: ModelTier,
    
    #[serde(default = "default_medium_tier")]
    pub medium: ModelTier,
    
    #[serde(default = "default_large_tier")]
    pub large: ModelTier,
}

fn default_routing_strategy() -> String {
    "hybrid".to_string()
}
```

**File**: `example_config.toml`

```toml
[cascade]
enabled = false
auto_escalate_on_error = true
telemetry_enabled = true

# Routing strategy: "conservative", "embedding", or "hybrid"
# - conservative: Fast, length-based heuristics only
# - embedding: Semantic understanding (requires model download ~130MB)
# - hybrid: Use heuristics for obvious cases, embeddings for ambiguous (recommended)
routing_strategy = "hybrid"

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
- [ ] Config supports routing_strategy
- [ ] Default is "hybrid"
- [ ] example_config.toml documents options

---

### Step 5: CLI Integration (2 hours)

**File**: `src/cli/agent.rs`

Update to use hybrid router:

```rust
use crate::routing::{ConservativeRouter, EmbeddingRouter, HybridRouter};

pub async fn handle_agent(
    backend_name: Option<String>,
    add_dirs: Vec<String>,
    skip_permissions: bool,
    continue_last: bool,
    config: &AppConfig,
) -> anyhow::Result<()> {
    // ... existing code ...

    let cascade_enabled = config
        .cascade
        .as_ref()
        .map(|c| c.enabled)
        .unwrap_or(false);

    let (backend, cascade_setup) = if cascade_enabled && backend_name.is_none() {
        console.info("Cascade mode enabled");

        // Determine routing strategy
        let routing_strategy = config
            .cascade
            .as_ref()
            .and_then(|c| Some(c.routing_strategy.as_str()))
            .unwrap_or("hybrid");

        // Create appropriate router
        let (initial_tier, confidence, strategy_name) = match routing_strategy {
            "conservative" => {
                console.info("Using conservative (heuristic) routing");
                let router = ConservativeRouter::new();
                let tier = router.suggest_tier(&temp_conv);
                let conf = router.routing_confidence(&temp_conv);
                (tier, conf, "conservative")
            }
            "embedding" => {
                console.info("Using embedding-based routing");
                let router = EmbeddingRouter::new()
                    .context("Failed to initialize embedding router")?;
                let tier = router.suggest_tier(&temp_conv);
                let conf = router.routing_confidence(&temp_conv);
                (tier, conf, "embedding")
            }
            "hybrid" | _ => {
                console.info("Using hybrid routing");
                let router = HybridRouter::new()
                    .context("Failed to initialize hybrid router")?;
                let tier = router.suggest_tier(&temp_conv);
                let conf = router.routing_confidence(&temp_conv);
                let strategy = router.routing_strategy(&temp_conv);
                (tier, conf, strategy)
            }
        };

        console.info(&format!(
            "Starting with {} tier (confidence: {:.1}%, strategy: {})",
            initial_tier,
            confidence * 100.0,
            strategy_name
        ));

        // ... rest of cascade setup ...
    } else {
        // ... non-cascade setup ...
    };

    // ... rest of function ...
}
```

**Acceptance Criteria**:
- [ ] CLI supports all three routing strategies
- [ ] Logs which strategy is being used
- [ ] Falls back gracefully on errors
- [ ] Backward compatible

---

### Step 6: Telemetry Enhancement (1 hour)

**File**: `src/routing/telemetry.rs`

Add routing strategy to telemetry:

```rust
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
    
    // NEW: Track which routing strategy was used
    pub routing_strategy: String,  // "conservative", "embedding", "hybrid"
}

impl RoutingDecision {
    pub fn new(
        initial_tier: ModelTier,
        message: &str,
        routing_confidence: f32,
        session_id: String,
        routing_strategy: String,
    ) -> Self {
        let message_preview = message.chars().take(100).collect::<String>();

        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            initial_tier,
            final_tier: initial_tier,
            escalation_count: 0,
            message_length: message.len(),
            message_preview,
            routing_confidence,
            session_id,
            routing_strategy,
        }
    }
}
```

**Acceptance Criteria**:
- [ ] Telemetry includes routing_strategy
- [ ] Backward compatible with Phase 1 logs
- [ ] Can differentiate heuristic vs embedding decisions

---

### Step 7: Testing (3-4 hours)

**File**: `tests/embedding_routing_test.rs`

```rust
use hoosh::routing::{EmbeddingRouter, ModelTier};
use hoosh::agent::Conversation;

#[test]
fn test_semantic_similarity() {
    let router = EmbeddingRouter::new().unwrap();

    // These should route to same tier (semantically similar)
    let pairs = vec![
        ("What is Rust?", "Explain Rust to me"),
        ("Fix the typo", "Correct the spelling error"),
        ("Design a microservices architecture", "Architect a distributed system"),
    ];

    for (msg1, msg2) in pairs {
        let mut conv1 = Conversation::new();
        conv1.add_user_message(msg1.to_string());
        
        let mut conv2 = Conversation::new();
        conv2.add_user_message(msg2.to_string());

        let tier1 = router.suggest_tier(&conv1);
        let tier2 = router.suggest_tier(&conv2);

        assert_eq!(tier1, tier2, "Failed for: '{}' vs '{}'", msg1, msg2);
    }
}

#[test]
fn test_embedding_accuracy() {
    let router = EmbeddingRouter::new().unwrap();

    // Test cases with expected tiers
    let test_cases = vec![
        ("What is Rust?", ModelTier::Small),
        ("Add error handling to file operations", ModelTier::Medium),
        ("Design comprehensive microservices architecture", ModelTier::Large),
        ("Fix typo", ModelTier::Small),
        ("Implement pagination", ModelTier::Medium),
        ("Architect distributed caching strategy", ModelTier::Large),
    ];

    let mut correct = 0;
    let total = test_cases.len();

    for (message, expected) in test_cases {
        let mut conv = Conversation::new();
        conv.add_user_message(message.to_string());

        let predicted = router.suggest_tier(&conv);
        if predicted == expected {
            correct += 1;
        } else {
            println!("Mismatch: '{}' -> predicted {}, expected {}", 
                message, predicted, expected);
        }
    }

    let accuracy = correct as f32 / total as f32;
    println!("Accuracy: {:.1}% ({}/{})", accuracy * 100.0, correct, total);

    // Should be at least 80% accurate on these clear examples
    assert!(accuracy >= 0.8, "Accuracy too low: {:.1}%", accuracy * 100.0);
}

#[test]
fn test_confidence_correlation() {
    let router = EmbeddingRouter::new().unwrap();

    // Exact reference match should have high confidence
    let mut exact = Conversation::new();
    exact.add_user_message("What is Rust?".to_string());
    let exact_conf = router.routing_confidence(&exact);

    // Ambiguous phrasing should have lower confidence
    let mut ambiguous = Conversation::new();
    ambiguous.add_user_message("Can you help with the thing?".to_string());
    let ambig_conf = router.routing_confidence(&ambiguous);

    assert!(exact_conf > ambig_conf);
    assert!(exact_conf > 0.7);
}

#[test]
fn test_reference_task_addition() {
    let mut router = EmbeddingRouter::new().unwrap();

    let initial_count = router.reference_stats();
    let initial_total = initial_count.0 + initial_count.1 + initial_count.2;

    router.add_reference_task(
        "Test task description".to_string(),
        ModelTier::Medium,
    ).unwrap();

    let new_count = router.reference_stats();
    let new_total = new_count.0 + new_count.1 + new_count.2;

    assert_eq!(new_total, initial_total + 1);
}
```

**Benchmarking**:

```rust
// File: benches/routing_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use hoosh::routing::{ConservativeRouter, EmbeddingRouter, HybridRouter};
use hoosh::agent::Conversation;

fn benchmark_conservative_routing(c: &mut Criterion) {
    let router = ConservativeRouter::new();
    let mut conv = Conversation::new();
    conv.add_user_message("Add error handling to this function".to_string());

    c.bench_function("conservative_routing", |b| {
        b.iter(|| router.suggest_tier(black_box(&conv)))
    });
}

fn benchmark_embedding_routing(c: &mut Criterion) {
    let router = EmbeddingRouter::new().unwrap();
    let mut conv = Conversation::new();
    conv.add_user_message("Add error handling to this function".to_string());

    c.bench_function("embedding_routing", |b| {
        b.iter(|| router.suggest_tier(black_box(&conv)))
    });
}

fn benchmark_hybrid_routing(c: &mut Criterion) {
    let router = HybridRouter::new().unwrap();
    let mut conv = Conversation::new();
    conv.add_user_message("Add error handling to this function".to_string());

    c.bench_function("hybrid_routing", |b| {
        b.iter(|| router.suggest_tier(black_box(&conv)))
    });
}

criterion_group!(
    benches,
    benchmark_conservative_routing,
    benchmark_embedding_routing,
    benchmark_hybrid_routing
);
criterion_main!(benches);
```

**Manual Testing**:

Create test script:

```bash
#!/bin/bash
# File: test_embedding_routing.sh

echo "Testing embedding-based routing..."

# Test small tier
hoosh --cascade <<EOF
What is Rust?
EOF

# Test medium tier  
hoosh --cascade <<EOF
Add error handling to the file reader module
EOF

# Test large tier
hoosh --cascade <<EOF
Design a comprehensive microservices architecture for authentication
with event sourcing, CQRS, and distributed caching
EOF
```

**Acceptance Criteria**:
- [ ] Semantic similarity test passes
- [ ] Accuracy test achieves >80%
- [ ] Confidence correlation test passes
- [ ] Embedding routing completes in <50ms
- [ ] Manual tests route correctly

---

### Step 8: Documentation (2 hours)

**File**: `docs/embedding-routing.md`

```markdown
# Embedding-Based Routing

## Overview

Phase 2 replaces simple heuristics with semantic understanding using local embedding models.

## How It Works

### Embedding Model

- **Model**: BGE-Small-EN-v1.5
- **Size**: ~130MB (one-time download)
- **Speed**: 10-20ms on CPU
- **Quality**: State-of-art for small models

### Classification Process

1. **Embed user message** using BGE model
2. **Find k-nearest neighbors** (default k=5) in reference set
3. **Weighted vote** based on cosine similarity
4. **Return majority tier** with confidence score

### Reference Set

100 hand-labeled examples covering:
- 30 small tier tasks (simple queries, quick fixes)
- 40 medium tier tasks (standard implementation)
- 30 large tier tasks (architecture, complex design)

Reference set automatically expands from validated telemetry.

## Semantic Understanding

Embeddings capture meaning, not just keywords:

```
"What is Rust?" → [0.32, -0.15, 0.87, ...]
"Explain Rust" → [0.31, -0.14, 0.85, ...]  # Very similar!

"Fix the typo" → [0.12, 0.44, -0.23, ...]
"Design a system" → [-0.45, 0.78, 0.56, ...]  # Very different!
```

## Configuration

```toml
[cascade]
routing_strategy = "embedding"  # Use pure embedding routing
```

Or use hybrid (recommended):

```toml
[cascade]
routing_strategy = "hybrid"  # Fast heuristics + embeddings
```

## Performance

Benchmarks on MacBook Pro (M1):
- Conservative: ~0.1ms
- Embedding: ~15ms
- Hybrid: ~0.1-15ms (adaptive)

## Accuracy

Expected routing accuracy:
- Conservative: ~70%
- Embedding: ~85%
- Hybrid: ~85% with better latency

## Expanding Reference Set

Add examples from telemetry:

```rust
let mut router = EmbeddingRouter::new()?;

// Add validated task from telemetry
router.add_reference_task(
    "Description of task".to_string(),
    ModelTier::Medium,
)?;
```

Future: Automatic expansion from high-confidence telemetry.

## Troubleshooting

### Model download fails
- Check network connectivity
- Model downloads to `~/.cache/fastembed/`
- Requires ~130MB free space

### Routing seems wrong
- Check reference set coverage
- Review telemetry for patterns
- Consider adding domain-specific examples

### Too slow
- Use `routing_strategy = "hybrid"`
- Only ambiguous cases use embeddings
- Conservative routing is <1ms
```

Update `README.md`:

```markdown
## Routing Strategies

Hoosh supports three routing strategies:

### Conservative (Fast)
- Simple length-based heuristics
- ~0.1ms latency
- ~70% accuracy
- No model download required

### Embedding (Accurate)
- Semantic understanding via BGE model
- ~15ms latency
- ~85% accuracy
- One-time 130MB model download

### Hybrid (Recommended)
- Best of both worlds
- Uses heuristics for obvious cases
- Falls back to embeddings for ambiguous cases
- ~0.1-15ms adaptive latency
- ~85% accuracy

Configure in `config.toml`:
```toml
[cascade]
routing_strategy = "hybrid"  # conservative | embedding | hybrid
```
```

**Acceptance Criteria**:
- [ ] Embedding routing documented
- [ ] Performance characteristics documented
- [ ] Configuration options explained
- [ ] Troubleshooting guide included
- [ ] README updated

---

## Phase 2 Completion Checklist

### Code
- [ ] fastembed dependency added
- [ ] EmbeddingRouter implemented
- [ ] HybridRouter implemented
- [ ] All tests pass
- [ ] Benchmarks run successfully

### Functionality
- [ ] Embedding-based routing works
- [ ] Semantic similarity detection works
- [ ] Hybrid router switches strategies appropriately
- [ ] Reference set pre-embeds correctly
- [ ] Confidence calculation is accurate

### Performance
- [ ] Embedding routing <50ms
- [ ] Hybrid routing uses fast path when possible
- [ ] No memory leaks
- [ ] Model loads once at startup

### Documentation
- [ ] Embedding routing documented
- [ ] Configuration options explained
- [ ] Performance characteristics documented
- [ ] Troubleshooting guide written

### Testing
- [ ] Unit tests pass
- [ ] Integration tests pass
- [ ] Accuracy tests achieve >80%
- [ ] Manual testing validates routing
- [ ] Benchmarks confirm performance

---

## Success Metrics

After Phase 2:
1. **Routing accuracy**: >85% (up from 70%)
2. **Latency**: <50ms for embedding, <1ms for heuristic
3. **Semantic understanding**: Can distinguish similar phrasings
4. **Confidence correlation**: High confidence → high accuracy

---

## Migration from Phase 1

Users can opt-in to embedding routing:

```toml
[cascade]
enabled = true
routing_strategy = "hybrid"  # Add this line
```

Phase 1 users keep working - embedding is opt-in via strategy selection.
