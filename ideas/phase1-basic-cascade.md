# Phase 1: Basic Cascade System - Implementation Ticket

## Overview

Implement a basic model cascade system that automatically selects appropriate models based on task complexity. Phase 1 focuses on conservative routing with a Medium-tier default and relies on the escalate tool for corrections.

**Duration**: 2-3 days  
**Goal**: Production-ready opt-in cascade system with telemetry foundation

---

## Architecture Decisions

### Routing Strategy
- **Conservative approach**: Use simple heuristics (primarily length-based)
- **Default to Medium tier**: Safe middle ground for ambiguous cases
- **Rely on escalate tool**: LLM self-corrects routing mistakes
- **No keyword matching**: Avoid brittle pattern matching

### Key Principles
1. Backward compatible (opt-in via config)
2. Fail gracefully (fallback to default backend if cascade fails)
3. Observable (log all tier selections and escalations)
4. Self-correcting (escalate tool provides feedback loop)

---

## Implementation Steps

### Step 1: Configuration System (1-2 hours)

**File**: `src/config/mod.rs`

Add configuration structures for cascade system:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModelTier {
    /// Backend name to use for this tier
    pub backend: String,
    /// Optional model override for this backend
    pub model: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CascadeConfig {
    /// Enable/disable cascade system
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Automatically escalate on errors
    #[serde(default = "default_auto_escalate")]
    pub auto_escalate_on_error: bool,

    /// Small model tier (for simple tasks)
    #[serde(default = "default_small_tier")]
    pub small: ModelTier,

    /// Medium model tier (for moderate tasks)
    #[serde(default = "default_medium_tier")]
    pub medium: ModelTier,

    /// Large model tier (for complex tasks)
    #[serde(default = "default_large_tier")]
    pub large: ModelTier,

    /// Log routing decisions for telemetry
    #[serde(default = "default_telemetry_enabled")]
    pub telemetry_enabled: bool,
}

fn default_enabled() -> bool {
    false // Opt-in for Phase 1
}

fn default_auto_escalate() -> bool {
    true
}

fn default_telemetry_enabled() -> bool {
    true
}

fn default_small_tier() -> ModelTier {
    ModelTier {
        backend: "anthropic".to_string(),
        model: Some("claude-3-5-haiku-20241022".to_string()),
    }
}

fn default_medium_tier() -> ModelTier {
    ModelTier {
        backend: "anthropic".to_string(),
        model: Some("claude-sonnet-4-20250514".to_string()),
    }
}

fn default_large_tier() -> ModelTier {
    ModelTier {
        backend: "anthropic".to_string(),
        model: Some("claude-opus-4-20250514".to_string()),
    }
}
```

Update `AppConfig` struct:

```rust
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    pub default_backend: String,
    #[serde(default)]
    pub backends: HashMap<String, BackendConfig>,
    #[serde(default)]
    pub verbosity: Option<String>,
    #[serde(default)]
    pub default_agent: Option<String>,
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,
    #[serde(default)]
    pub context_manager: Option<ContextManagerConfig>,
    #[serde(default)]
    pub core_reminder_token_threshold: Option<usize>,

    // NEW: Cascade configuration
    #[serde(default)]
    pub cascade: Option<CascadeConfig>,
}
```

Update `ProjectConfig` similarly.

**File**: `example_config.toml`

Add cascade configuration section:

```toml
# Model Cascade System (Optional)
# Automatically routes tasks to appropriate model tiers based on complexity
[cascade]
enabled = false  # Set to true to enable cascade routing
auto_escalate_on_error = true  # Upgrade model tier on errors

# Telemetry: Managed via CLI commands (opt-in)
# Run 'hoosh telemetry enable' to start collecting anonymized routing data
# Run 'hoosh telemetry status' to check current state

# Small tier - for simple, quick tasks
[cascade.small]
backend = "anthropic"
model = "claude-3-5-haiku-20241022"

# Medium tier - default for most tasks
[cascade.medium]
backend = "anthropic"
model = "claude-sonnet-4-20250514"

# Large tier - for complex reasoning and architecture
[cascade.large]
backend = "anthropic"
model = "claude-opus-4-20250514"

# Example: Mixed backend configuration
# [cascade.small]
# backend = "together"
# model = "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo"
#
# [cascade.medium]
# backend = "anthropic"
# model = "claude-sonnet-4-20250514"
#
# [cascade.large]
# backend = "openai"
# model = "gpt-4"
```

**Acceptance Criteria**:
- [ ] Config structs compile without errors
- [ ] Default values are sensible
- [ ] example_config.toml documents cascade feature
- [ ] Cascade config is optional (backward compatible)

---

### Step 2: Routing Module (2-3 hours)

**File**: `src/routing/mod.rs`

```rust
pub mod conservative_router;
pub mod model_tier;
pub mod telemetry;

pub use conservative_router::ConservativeRouter;
pub use model_tier::ModelTier;
pub use telemetry::{RoutingDecision, RoutingTelemetry};
```

**File**: `src/routing/model_tier.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelTier {
    Small,
    Medium,
    Large,
}

impl ModelTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            ModelTier::Small => "small",
            ModelTier::Medium => "medium",
            ModelTier::Large => "large",
        }
    }
    
    pub fn next(&self) -> Option<ModelTier> {
        match self {
            ModelTier::Small => Some(ModelTier::Medium),
            ModelTier::Medium => Some(ModelTier::Large),
            ModelTier::Large => None,
        }
    }
}

impl std::fmt::Display for ModelTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
```

**File**: `src/routing/conservative_router.rs`

```rust
use crate::agent::Conversation;
use crate::routing::ModelTier;

/// Conservative router that uses simple heuristics and defaults to Medium tier.
/// 
/// Philosophy:
/// - Avoid complex keyword matching (too brittle)
/// - Use structural properties (length, code blocks, etc.)
/// - Default to Medium for ambiguous cases
/// - Rely on escalate tool for self-correction
pub struct ConservativeRouter;

impl ConservativeRouter {
    pub fn new() -> Self {
        Self
    }

    /// Suggests initial model tier based on simple heuristics
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

        // Structural analysis (no keywords)
        let char_count = msg.len();
        let word_count = msg.split_whitespace().count();
        let line_count = msg.lines().count();
        let has_code_block = msg.contains("```");
        let code_block_count = msg.matches("```").count() / 2;

        // Rule 1: Very short, simple queries → Small
        // "What is Rust?"
        // "Fix the typo"
        // "Show me the error"
        if char_count < 100 && word_count < 20 && !has_code_block {
            return ModelTier::Small;
        }

        // Rule 2: Very detailed requirements → Large
        // Long specifications, multiple code blocks, detailed context
        if char_count > 1500 || word_count > 400 {
            return ModelTier::Large;
        }

        // Rule 3: Multiple large code blocks → Large
        // Suggests complex refactoring or multi-file changes
        if code_block_count > 2 && char_count > 800 {
            return ModelTier::Large;
        }

        // Rule 4: Moderate length with code → Medium
        // Most coding tasks fall here
        if has_code_block && char_count > 200 {
            return ModelTier::Medium;
        }

        // Rule 5: Multi-paragraph without code → Medium
        // Architectural discussions, design questions
        if line_count > 10 && word_count > 150 {
            return ModelTier::Medium;
        }

        // Default: Medium tier for everything else
        // This is the safe choice - not too expensive, not too limited
        ModelTier::Medium
    }

    /// Calculate confidence in the routing decision (0.0 - 1.0)
    pub fn routing_confidence(&self, conversation: &Conversation) -> f32 {
        let last_user_msg = conversation
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .and_then(|m| m.content.as_ref());

        let Some(msg) = last_user_msg else {
            return 0.5; // Neutral confidence for empty input
        };

        let char_count = msg.len();
        let word_count = msg.split_whitespace().count();

        // High confidence for extreme cases
        if (char_count < 50 && word_count < 10) || char_count > 2000 {
            return 0.9; // Very confident
        }

        // Medium confidence for moderate cases
        if char_count < 200 || char_count > 1000 {
            return 0.7;
        }

        // Low confidence for ambiguous middle range
        0.5
    }
}

impl Default for ConservativeRouter {
    fn default() -> Self {
        Self::new()
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
    fn test_very_short_query_suggests_small() {
        let conv = create_test_conversation("What is Rust?");
        let router = ConservativeRouter::new();
        assert_eq!(router.suggest_tier(&conv), ModelTier::Small);
    }

    #[test]
    fn test_short_command_suggests_small() {
        let conv = create_test_conversation("Fix the typo in line 42");
        let router = ConservativeRouter::new();
        assert_eq!(router.suggest_tier(&conv), ModelTier::Small);
    }

    #[test]
    fn test_medium_task_suggests_medium() {
        let conv = create_test_conversation(
            "Add error handling to the file reader function. \
             It should handle file not found and permission errors."
        );
        let router = ConservativeRouter::new();
        assert_eq!(router.suggest_tier(&conv), ModelTier::Medium);
    }

    #[test]
    fn test_long_detailed_request_suggests_large() {
        let mut long_request = String::from(
            "I need to refactor our authentication system. "
        );
        long_request.push_str(&"Here's the detailed context. ".repeat(100));
        
        let conv = create_test_conversation(&long_request);
        let router = ConservativeRouter::new();
        assert_eq!(router.suggest_tier(&conv), ModelTier::Large);
    }

    #[test]
    fn test_code_block_suggests_medium() {
        let conv = create_test_conversation(
            "Fix this code:\n```rust\nfn broken() { panic!() }\n```"
        );
        let router = ConservativeRouter::new();
        assert_eq!(router.suggest_tier(&conv), ModelTier::Medium);
    }

    #[test]
    fn test_multiple_code_blocks_suggests_large() {
        let conv = create_test_conversation(
            "Compare these implementations:\n\
             ```rust\nfn v1() {}\n```\n\
             ```rust\nfn v2() {}\n```\n\
             ```rust\nfn v3() {}\n```\n\
             Which is best and why?"
        );
        let router = ConservativeRouter::new();
        assert_eq!(router.suggest_tier(&conv), ModelTier::Large);
    }

    #[test]
    fn test_ambiguous_defaults_to_medium() {
        let conv = create_test_conversation(
            "Can you help me understand how async works in this context?"
        );
        let router = ConservativeRouter::new();
        assert_eq!(router.suggest_tier(&conv), ModelTier::Medium);
    }

    #[test]
    fn test_confidence_high_for_extremes() {
        let router = ConservativeRouter::new();
        
        // Very short
        let short_conv = create_test_conversation("Hi");
        assert!(router.routing_confidence(&short_conv) > 0.8);
        
        // Very long
        let long_msg = "x".repeat(2500);
        let long_conv = create_test_conversation(&long_msg);
        assert!(router.routing_confidence(&long_conv) > 0.8);
    }

    #[test]
    fn test_confidence_lower_for_ambiguous() {
        let router = ConservativeRouter::new();
        let conv = create_test_conversation(
            "Can you help with this moderate task?"
        );
        assert!(router.routing_confidence(&conv) < 0.7);
    }
}
```

**File**: `src/routing/telemetry.rs`

```rust
use crate::routing::ModelTier;
use serde::{Deserialize, Serialize};
use std::fs::{OpenOptions, create_dir_all, File};
use std::io::{Write, BufRead, BufReader};
use std::path::PathBuf;
use anyhow::{Result, Context};

/// Anonymized routing decision for telemetry
/// 
/// PRIVACY: No message content, only metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub timestamp: String,
    pub initial_tier: ModelTier,
    pub final_tier: ModelTier,
    pub escalation_count: u32,
    
    // Anonymized metrics - no content
    pub message_length: usize,
    pub message_word_count: usize,
    pub has_code_block: bool,
    pub question_count: usize,
    pub line_count: usize,
    
    pub routing_confidence: f32,
    pub session_id: String,  // Random UUID, not user-identifiable
}

/// Consent state for telemetry collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConsent {
    pub enabled: bool,
    pub consented_at: Option<String>,
    pub version: u32,  // Track consent version for future updates
}

impl Default for TelemetryConsent {
    fn default() -> Self {
        Self {
            enabled: false,  // OPT-IN by default
            consented_at: None,
            version: 1,
        }
    }
}

pub struct RoutingTelemetry {
    log_path: PathBuf,
    consent_path: PathBuf,
    consent: TelemetryConsent,
}

impl RoutingTelemetry {
    /// Create new telemetry instance, loading existing consent
    pub fn new() -> Result<Self> {
        let config_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
            .join(".hoosh");

        let log_path = config_dir.join("routing_telemetry.jsonl");
        let consent_path = config_dir.join("telemetry_consent.json");

        // Load existing consent or use default (disabled)
        let consent = if consent_path.exists() {
            let content = std::fs::read_to_string(&consent_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            TelemetryConsent::default()
        };

        Ok(Self {
            log_path,
            consent_path,
            consent,
        })
    }

    /// Check if telemetry is enabled
    pub fn is_enabled(&self) -> bool {
        self.consent.enabled
    }

    /// Enable telemetry with user consent
    pub fn enable_with_consent(&mut self) -> Result<()> {
        self.consent.enabled = true;
        self.consent.consented_at = Some(chrono::Utc::now().to_rfc3339());
        self.save_consent()?;

        // Create directory for telemetry
        if let Some(parent) = self.log_path.parent() {
            create_dir_all(parent)?;
        }

        Ok(())
    }

    /// Disable telemetry
    pub fn disable(&mut self) -> Result<()> {
        self.consent.enabled = false;
        self.save_consent()?;
        Ok(())
    }

    /// Get consent status
    pub fn get_consent(&self) -> &TelemetryConsent {
        &self.consent
    }

    /// Save consent to disk
    fn save_consent(&self) -> Result<()> {
        if let Some(parent) = self.consent_path.parent() {
            create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(&self.consent)?;
        std::fs::write(&self.consent_path, json)?;
        Ok(())
    }

    /// Log a routing decision (only if enabled)
    pub fn log_decision(&self, decision: &RoutingDecision) -> Result<()> {
        if !self.consent.enabled {
            return Ok(());
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .context("Failed to open telemetry log")?;

        let json = serde_json::to_string(decision)?;
        writeln!(file, "{}", json)?;

        Ok(())
    }

    /// Get path to telemetry log
    pub fn log_path(&self) -> &PathBuf {
        &self.log_path
    }

    /// Get path to consent file
    pub fn consent_path(&self) -> &PathBuf {
        &self.consent_path
    }

    /// Clear all telemetry data
    pub fn clear_data(&self) -> Result<()> {
        if self.log_path.exists() {
            std::fs::remove_file(&self.log_path)
                .context("Failed to delete telemetry log")?;
        }
        Ok(())
    }

    /// Get telemetry file size
    pub fn data_size(&self) -> Result<u64> {
        if !self.log_path.exists() {
            return Ok(0);
        }
        let metadata = std::fs::metadata(&self.log_path)?;
        Ok(metadata.len())
    }

    /// Count number of logged decisions
    pub fn count_decisions(&self) -> Result<usize> {
        if !self.log_path.exists() {
            return Ok(0);
        }

        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);
        Ok(reader.lines().count())
    }
}

impl RoutingDecision {
    /// Create new routing decision from message (anonymized)
    pub fn new(
        initial_tier: ModelTier,
        message: &str,
        routing_confidence: f32,
        session_id: String,
    ) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            initial_tier,
            final_tier: initial_tier,
            escalation_count: 0,
            
            // Anonymized metrics only
            message_length: message.len(),
            message_word_count: message.split_whitespace().count(),
            has_code_block: message.contains("```"),
            question_count: message.matches('?').count(),
            line_count: message.lines().count(),
            
            routing_confidence,
            session_id,
        }
    }

    /// Record an escalation
    pub fn record_escalation(&mut self, new_tier: ModelTier) {
        self.escalation_count += 1;
        self.final_tier = new_tier;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_telemetry_defaults_to_disabled() {
        let consent = TelemetryConsent::default();
        assert!(!consent.enabled);
        assert!(consent.consented_at.is_none());
    }

    #[test]
    fn test_routing_decision_anonymization() {
        let decision = RoutingDecision::new(
            ModelTier::Medium,
            "This is a secret message with sensitive data",
            0.75,
            "test-session".to_string(),
        );

        // Verify NO content is stored
        let json = serde_json::to_string(&decision).unwrap();
        assert!(!json.contains("secret"));
        assert!(!json.contains("sensitive"));
        
        // Verify metadata IS stored
        assert!(json.contains("message_length"));
        assert!(json.contains("message_word_count"));
    }

    #[test]
    fn test_consent_enable_disable() {
        let mut telemetry = RoutingTelemetry::new().unwrap();
        
        assert!(!telemetry.is_enabled());
        
        telemetry.enable_with_consent().unwrap();
        assert!(telemetry.is_enabled());
        
        telemetry.disable().unwrap();
        assert!(!telemetry.is_enabled());
    }
}
```

Update `src/lib.rs`:

```rust
pub mod routing;
```

**Acceptance Criteria**:
- [ ] ConservativeRouter correctly routes based on length/structure
- [ ] All unit tests pass
- [ ] Telemetry logs to ~/.hoosh/routing_telemetry.jsonl
- [ ] No keyword-based pattern matching
- [ ] Confidence scores are reasonable

---

### Step 3: Escalate Tool (1-2 hours)

**File**: `src/tools/escalate_tool.rs`

```rust
use crate::tools::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

/// Tool that allows the LLM to request escalation to a more capable model.
/// 
/// This tool's execution is intercepted by the Agent - the agent detects
/// the escalate tool call and switches to the next higher tier before
/// the tool actually executes.
pub struct EscalateTool;

impl EscalateTool {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Deserialize)]
struct EscalateArgs {
    reason: String,
}

#[async_trait]
impl Tool for EscalateTool {
    async fn execute(
        &self,
        args: &Value,
        _context: &crate::tools::ToolExecutionContext,
    ) -> ToolResult<String> {
        let args: EscalateArgs = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::InvalidArguments {
                tool: "escalate".to_string(),
                message: e.to_string(),
            })?;

        // This should not normally execute - the Agent intercepts this call
        // But if it does, return a marker that can be detected
        Ok(format!("ESCALATE_REQUEST: {}", args.reason))
    }

    fn name(&self) -> &'static str {
        "escalate"
    }

    fn display_name(&self) -> &'static str {
        "Escalate to More Capable Model"
    }

    fn description(&self) -> &'static str {
        "Request escalation to a more capable model when the current task exceeds your capabilities.\n\n\
        USE THIS TOOL WHEN:\n\
        - The task requires more complex reasoning than you can reliably provide\n\
        - You've attempted the task but recognize you need more computational resources\n\
        - The problem involves intricate multi-step logic beyond your capacity\n\
        - You need better understanding of nuanced requirements\n\
        - You're uncertain about handling the complexity correctly\n\n\
        DO NOT USE THIS TOOL WHEN:\n\
        - You can handle the task with your current capabilities\n\
        - You haven't attempted the task yet (try first, escalate if needed)\n\
        - You just need more information from the user (ask instead)\n\
        - The task is simple but you're being overly cautious\n\n\
        Be honest about your limitations - escalation helps ensure high-quality results."
    }

    fn parameter_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "reason": {
                    "type": "string",
                    "description": "Explain specifically why you need a more capable model. What aspect of the task exceeds your current capabilities? Be concrete and honest."
                }
            },
            "required": ["reason"]
        })
    }

    fn is_hidden(&self) -> bool {
        false
    }

    fn format_call_display(&self, args: &Value) -> String {
        if let Ok(parsed) = serde_json::from_value::<EscalateArgs>(args.clone()) {
            format!("Escalate ({})", parsed.reason)
        } else {
            "Escalate to More Capable Model".to_string()
        }
    }
}

impl Default for EscalateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_escalate_tool_execution() {
        let tool = EscalateTool::new();
        let args = json!({
            "reason": "Task requires complex architectural design beyond my capabilities"
        });
        
        let ctx = crate::tools::ToolExecutionContext {
            tool_call_id: "test_123".to_string(),
            event_tx: None,
            parent_conversation_id: None,
        };

        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("ESCALATE_REQUEST"));
    }

    #[test]
    fn test_tool_metadata() {
        let tool = EscalateTool::new();
        assert_eq!(tool.name(), "escalate");
        assert!(!tool.is_hidden());
        assert!(tool.description().contains("more capable model"));
    }

    #[test]
    fn test_parameter_schema() {
        let tool = EscalateTool::new();
        let schema = tool.parameter_schema();
        
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["reason"].is_object());
        assert!(schema["required"].as_array().unwrap().contains(&json!("reason")));
    }
}
```

**File**: `src/tools/builtin_tool_provider.rs`

Add escalate tool to the registry:

```rust
use crate::tools::escalate_tool::EscalateTool;

// In the tool registration section:
pub fn create_builtin_tools(/* ... */) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    
    // ... existing tools ...
    
    // Add escalate tool
    registry.register(Arc::new(EscalateTool::new()));
    
    registry
}
```

**Acceptance Criteria**:
- [ ] EscalateTool compiles and tests pass
- [ ] Tool is registered in builtin provider
- [ ] Tool description clearly explains when to use/not use
- [ ] Parameter schema is valid

---

### Step 4: Backend Factory (2-3 hours)

**File**: `src/backends/cascade_factory.rs`

```rust
use crate::backends::{create_backend, LlmBackend};
use crate::config::AppConfig;
use crate::routing::ModelTier;
use anyhow::{Context, Result};
use std::sync::Arc;

/// Factory for creating backends at different model tiers
pub trait CascadeBackendFactory: Send + Sync {
    fn create_for_tier(&self, tier: ModelTier) -> Result<Box<dyn LlmBackend>>;
    fn get_tier_info(&self, tier: ModelTier) -> Option<(String, Option<String>)>;
}

/// Configuration-backed factory that creates backends from AppConfig
pub struct ConfigBackedFactory {
    config: AppConfig,
}

impl ConfigBackedFactory {
    pub fn new(config: AppConfig) -> Result<Self> {
        // Validate that cascade config exists
        if config.cascade.is_none() {
            anyhow::bail!("Cascade configuration not found in config");
        }

        // Validate that all tier backends are configured
        let cascade = config.cascade.as_ref().unwrap();
        for (tier_name, tier_config) in [
            ("small", &cascade.small),
            ("medium", &cascade.medium),
            ("large", &cascade.large),
        ] {
            if config.backends.get(&tier_config.backend).is_none() {
                anyhow::bail!(
                    "Backend '{}' for {} tier is not configured. Add it to [backends] section.",
                    tier_config.backend,
                    tier_name
                );
            }
        }

        Ok(Self { config })
    }
}

impl CascadeBackendFactory for ConfigBackedFactory {
    fn create_for_tier(&self, tier: ModelTier) -> Result<Box<dyn LlmBackend>> {
        let cascade_config = self
            .config
            .cascade
            .as_ref()
            .context("Cascade config not available")?;

        let tier_config = match tier {
            ModelTier::Small => &cascade_config.small,
            ModelTier::Medium => &cascade_config.medium,
            ModelTier::Large => &cascade_config.large,
        };

        // Get the base backend configuration
        let mut backend_config = self
            .config
            .backends
            .get(&tier_config.backend)
            .context(format!(
                "Backend '{}' not found for tier {}",
                tier_config.backend,
                tier.as_str()
            ))?
            .clone();

        // Override model if tier specifies one
        if let Some(model) = &tier_config.model {
            backend_config.model = Some(model.clone());
        }

        // Create the backend
        create_backend(&tier_config.backend, &self.config)
            .context(format!("Failed to create backend for tier {}", tier.as_str()))
    }

    fn get_tier_info(&self, tier: ModelTier) -> Option<(String, Option<String>)> {
        let cascade_config = self.config.cascade.as_ref()?;
        
        let tier_config = match tier {
            ModelTier::Small => &cascade_config.small,
            ModelTier::Medium => &cascade_config.medium,
            ModelTier::Large => &cascade_config.large,
        };

        Some((tier_config.backend.clone(), tier_config.model.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BackendConfig, CascadeConfig, ModelTier as ConfigModelTier};
    use std::collections::HashMap;

    fn create_test_config() -> AppConfig {
        let mut backends = HashMap::new();
        backends.insert(
            "test_backend".to_string(),
            BackendConfig {
                api_key: Some("test_key".to_string()),
                model: Some("test-model".to_string()),
                ..Default::default()
            },
        );

        AppConfig {
            default_backend: "test_backend".to_string(),
            backends,
            cascade: Some(CascadeConfig {
                enabled: true,
                auto_escalate_on_error: true,
                telemetry_enabled: true,
                small: ConfigModelTier {
                    backend: "test_backend".to_string(),
                    model: Some("small-model".to_string()),
                },
                medium: ConfigModelTier {
                    backend: "test_backend".to_string(),
                    model: Some("medium-model".to_string()),
                },
                large: ConfigModelTier {
                    backend: "test_backend".to_string(),
                    model: Some("large-model".to_string()),
                },
            }),
            ..Default::default()
        }
    }

    #[test]
    fn test_factory_creation_validates_config() {
        let config = create_test_config();
        let factory = ConfigBackedFactory::new(config);
        assert!(factory.is_ok());
    }

    #[test]
    fn test_factory_fails_without_cascade_config() {
        let mut config = create_test_config();
        config.cascade = None;
        
        let factory = ConfigBackedFactory::new(config);
        assert!(factory.is_err());
    }

    #[test]
    fn test_get_tier_info() {
        let config = create_test_config();
        let factory = ConfigBackedFactory::new(config).unwrap();

        let (backend, model) = factory.get_tier_info(ModelTier::Small).unwrap();
        assert_eq!(backend, "test_backend");
        assert_eq!(model, Some("small-model".to_string()));
    }
}
```

Update `src/backends/mod.rs`:

```rust
pub mod cascade_factory;

pub use cascade_factory::{CascadeBackendFactory, ConfigBackedFactory};
```

**Acceptance Criteria**:
- [ ] Factory validates config on creation
- [ ] Factory can create backends for all tiers
- [ ] Factory returns appropriate error messages
- [ ] Unit tests pass

---

### Step 5: Agent Core Changes (3-4 hours)

**File**: `src/agent/core.rs`

Add fields to Agent struct:

```rust
use std::sync::{Arc, RwLock};
use crate::routing::ModelTier;
use crate::backends::CascadeBackendFactory;

pub struct Agent {
    backend: Arc<dyn LlmBackend>,
    tool_registry: Arc<ToolRegistry>,
    tool_executor: Arc<ToolExecutor>,
    max_steps: usize,
    event_sender: Option<mpsc::UnboundedSender<AgentEvent>>,
    context_manager: Option<Arc<ContextManager>>,
    system_reminder: Option<Arc<SystemReminder>>,
    
    // NEW: Cascade support
    current_tier: Arc<RwLock<ModelTier>>,
    backend_factory: Option<Arc<dyn CascadeBackendFactory>>,
    telemetry: Option<Arc<RoutingTelemetry>>,
    routing_decision: Option<Arc<RwLock<RoutingDecision>>>,
}
```

Add builder methods:

```rust
impl Agent {
    pub fn with_cascade_support(
        mut self,
        initial_tier: ModelTier,
        factory: Arc<dyn CascadeBackendFactory>,
        telemetry: Option<Arc<RoutingTelemetry>>,
    ) -> Self {
        self.current_tier = Arc::new(RwLock::new(initial_tier));
        self.backend_factory = Some(factory);
        self.telemetry = telemetry;
        self
    }

    pub fn set_routing_decision(&mut self, decision: RoutingDecision) {
        self.routing_decision = Some(Arc::new(RwLock::new(decision)));
    }

    /// Escalate to next higher model tier
    async fn escalate_model(&mut self, reason: &str) -> Result<bool> {
        let current = *self.current_tier.read().unwrap();
        
        let next_tier = match current.next() {
            Some(tier) => tier,
            None => {
                self.send_event(AgentEvent::Info(
                    "Already at highest tier, cannot escalate further".to_string()
                ));
                return Ok(false);
            }
        };

        let factory = match &self.backend_factory {
            Some(f) => f,
            None => {
                self.send_event(AgentEvent::Warning(
                    "Escalation requested but cascade not enabled".to_string()
                ));
                return Ok(false);
            }
        };

        // Create new backend at higher tier
        let new_backend = factory.create_for_tier(next_tier)
            .context("Failed to create backend for escalated tier")?;
        
        new_backend.initialize().await?;

        // Get tier info for logging
        let tier_info = factory.get_tier_info(next_tier)
            .map(|(b, m)| format!("{}/{}", b, m.unwrap_or_else(|| "default".to_string())))
            .unwrap_or_else(|| "unknown".to_string());

        // Swap backend
        self.backend = Arc::from(new_backend);
        
        // Update tier
        if let Ok(mut tier) = self.current_tier.write() {
            *tier = next_tier;
        }

        // Update telemetry
        if let Some(decision) = &self.routing_decision {
            if let Ok(mut d) = decision.write() {
                d.record_escalation(next_tier);
            }
        }

        // Emit event
        self.send_event(AgentEvent::ModelEscalated {
            from: current.to_string(),
            to: next_tier.to_string(),
            reason: reason.to_string(),
            model: tier_info,
        });

        Ok(true)
    }
}
```

Update `handle_tool_calls` to intercept escalate:

```rust
async fn handle_tool_calls(
    &mut self,  // Now mutable
    conversation: &mut Conversation,
    response: LlmResponse,
) -> Result<TurnStatus> {
    let tool_calls = response
        .tool_calls
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Expected tool calls but none found"))?;

    // Check for escalate tool call
    if let Some(escalate_call) = tool_calls.iter().find(|tc| tc.function.name == "escalate") {
        // Extract reason
        let reason = if let Ok(args) = serde_json::from_str::<serde_json::Value>(
            &escalate_call.function.arguments
        ) {
            args.get("reason")
                .and_then(|r| r.as_str())
                .unwrap_or("No reason provided")
                .to_string()
        } else {
            "No reason provided".to_string()
        };

        // Attempt escalation
        let escalated = self.escalate_model(&reason).await?;

        if escalated {
            // Add acknowledgment to conversation
            conversation.add_user_message(
                format!(
                    "I've escalated you to a more capable model tier. Please continue with the task.\n\n\
                    Original request: {}",
                    conversation.messages.iter()
                        .rev()
                        .find(|m| m.role == "user")
                        .and_then(|m| m.content.as_ref())
                        .unwrap_or(&"".to_string())
                )
            );

            return Ok(TurnStatus::Continue);
        } else {
            // Could not escalate (already at max tier)
            conversation.add_user_message(
                "Escalation was requested but you're already at the highest available tier. \
                 Please proceed with your best effort.".to_string()
            );
            return Ok(TurnStatus::Continue);
        }
    }

    // ... existing tool call handling ...
}
```

Update error handling for auto-escalation:

```rust
// In run() method, after catching errors:
Err(e) if e.should_send_to_llm() => {
    // Check for auto-escalation on error
    if let Some(cascade_config) = self.config.cascade.as_ref() {
        if cascade_config.auto_escalate_on_error {
            let current = *self.current_tier.read().unwrap();
            
            if current != ModelTier::Large {
                self.send_event(AgentEvent::Info(
                    format!("Auto-escalating due to error: {}", e.user_message())
                ));
                
                self.escalate_model("Auto-escalation due to error")
                    .await
                    .ok(); // Don't fail if escalation fails
            }
        }
    }

    let error_msg = e.user_message();
    conversation.add_user_message(error_msg);
    continue;
}
```

**File**: `src/agent/agent_events.rs`

Add escalation event:

```rust
#[derive(Debug, Clone)]
pub enum AgentEvent {
    // ... existing variants ...
    
    ModelEscalated {
        from: String,
        to: String,
        reason: String,
        model: String,
    },
}
```

**Acceptance Criteria**:
- [ ] Agent can swap backends mid-conversation
- [ ] Escalate tool calls trigger model upgrade
- [ ] Auto-escalation on errors works (when enabled)
- [ ] Telemetry records escalations
- [ ] Events are emitted for escalations

---

### Step 6: Telemetry Consent CLI (1-2 hours)

**File**: `src/cli/telemetry.rs`

```rust
use crate::routing::RoutingTelemetry;
use anyhow::Result;

pub async fn handle_enable_telemetry() -> Result<()> {
    let mut telemetry = RoutingTelemetry::new()?;

    if telemetry.is_enabled() {
        println!("✓ Telemetry is already enabled");
        return Ok(());
    }

    println!("=== Hoosh Telemetry Consent ===\n");
    println!("Hoosh can collect anonymous routing data to improve cascade accuracy.\n");
    
    println!("What we collect:");
    println!("  ✓ Routing decisions (which tier was chosen)");
    println!("  ✓ Escalation events");
    println!("  ✓ Message metadata (length, word count, has code blocks)");
    println!("  ✓ Routing confidence scores\n");
    
    println!("What we DON'T collect:");
    println!("  ✗ Message content or code");
    println!("  ✗ File paths or names");
    println!("  ✗ Personal information");
    println!("  ✗ API keys or credentials\n");
    
    println!("All data stays LOCAL on your machine:");
    println!("  • Stored in: ~/.hoosh/routing_telemetry.jsonl");
    println!("  • Never sent to external servers");
    println!("  • You can delete it anytime with: hoosh telemetry clear\n");

    print!("Enable telemetry? [y/N]: ");
    std::io::Write::flush(&mut std::io::stdout())?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() == "y" {
        telemetry.enable_with_consent()?;
        println!("\n✓ Telemetry enabled!");
        println!("  Consent recorded in: {:?}", telemetry.consent_path());
        println!("  Data will be logged to: {:?}", telemetry.log_path());
    } else {
        println!("\nTelemetry not enabled. You can enable it later with:");
        println!("  hoosh telemetry enable");
    }

    Ok(())
}

pub async fn handle_disable_telemetry() -> Result<()> {
    let mut telemetry = RoutingTelemetry::new()?;

    if !telemetry.is_enabled() {
        println!("Telemetry is already disabled");
        return Ok(());
    }

    telemetry.disable()?;
    println!("✓ Telemetry disabled");
    println!("\nExisting telemetry data has NOT been deleted.");
    println!("To delete it, run: hoosh telemetry clear");

    Ok(())
}

pub async fn handle_telemetry_status() -> Result<()> {
    let telemetry = RoutingTelemetry::new()?;
    let consent = telemetry.get_consent();

    println!("=== Telemetry Status ===\n");
    
    if consent.enabled {
        println!("Status: ✓ ENABLED");
        if let Some(date) = &consent.consented_at {
            println!("Consented at: {}", date);
        }
    } else {
        println!("Status: ✗ DISABLED");
    }

    println!("\nData location: {:?}", telemetry.log_path());
    
    if telemetry.log_path().exists() {
        let count = telemetry.count_decisions()?;
        let size = telemetry.data_size()?;
        println!("Decisions logged: {}", count);
        println!("Data size: {:.2} KB", size as f64 / 1024.0);
    } else {
        println!("No data collected yet");
    }

    println!("\nConsent file: {:?}", telemetry.consent_path());

    if !consent.enabled {
        println!("\nTo enable telemetry, run: hoosh telemetry enable");
    }

    Ok(())
}

pub async fn handle_clear_telemetry() -> Result<()> {
    let telemetry = RoutingTelemetry::new()?;

    if !telemetry.log_path().exists() {
        println!("No telemetry data to clear");
        return Ok(());
    }

    let count = telemetry.count_decisions()?;
    let size = telemetry.data_size()?;

    println!("This will delete {} routing decisions ({:.2} KB)", count, size as f64 / 1024.0);
    print!("Are you sure? [y/N]: ");
    std::io::Write::flush(&mut std::io::stdout())?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() == "y" {
        telemetry.clear_data()?;
        println!("✓ Telemetry data cleared");
    } else {
        println!("Cancelled");
    }

    Ok(())
}
```

Update `src/cli/mod.rs`:

```rust
mod telemetry;

#[derive(Subcommand)]
pub enum Commands {
    // ... existing commands ...
    
    /// Manage telemetry settings
    #[command(subcommand)]
    Telemetry(TelemetryCommands),
}

#[derive(Subcommand)]
pub enum TelemetryCommands {
    /// Enable telemetry collection (with consent prompt)
    Enable,
    
    /// Disable telemetry collection
    Disable,
    
    /// Show telemetry status and data size
    Status,
    
    /// Clear all collected telemetry data
    Clear,
}

// In handle_command:
Commands::Telemetry(cmd) => match cmd {
    TelemetryCommands::Enable => telemetry::handle_enable_telemetry().await,
    TelemetryCommands::Disable => telemetry::handle_disable_telemetry().await,
    TelemetryCommands::Status => telemetry::handle_telemetry_status().await,
    TelemetryCommands::Clear => telemetry::handle_clear_telemetry().await,
}
```

**Acceptance Criteria**:
- [ ] `hoosh telemetry enable` shows consent prompt
- [ ] `hoosh telemetry disable` works
- [ ] `hoosh telemetry status` shows current state
- [ ] `hoosh telemetry clear` deletes data
- [ ] Consent is persistent across sessions
- [ ] Clear privacy information displayed

---

### Step 7: CLI Integration (2-3 hours)

**File**: `src/cli/agent.rs`

Update `handle_agent` function:

```rust
use crate::routing::{ConservativeRouter, RoutingDecision, RoutingTelemetry};
use crate::backends::ConfigBackedFactory;

pub async fn handle_agent(
    backend_name: Option<String>,
    add_dirs: Vec<String>,
    skip_permissions: bool,
    continue_last: bool,
    config: &AppConfig,
) -> anyhow::Result<()> {
    let console = Console::new();

    // Initialize telemetry (respects consent)
    let telemetry = Arc::new(RoutingTelemetry::new()?);
    
    // Check if telemetry is enabled, show prompt on first cascade use
    let should_prompt_telemetry = config
        .cascade
        .as_ref()
        .map(|c| c.enabled)
        .unwrap_or(false)
        && !telemetry.is_enabled()
        && telemetry.get_consent().consented_at.is_none();

    if should_prompt_telemetry {
        console.info("Cascade mode is enabled for the first time.");
        console.info("Telemetry helps improve routing accuracy over time.");
        console.info("Run 'hoosh telemetry enable' to opt-in (recommended)");
        console.info("Run 'hoosh telemetry status' for more information\n");
    }

    // Check if cascade is enabled
    let cascade_enabled = config
        .cascade
        .as_ref()
        .map(|c| c.enabled)
        .unwrap_or(false);

    // Setup backend and cascade
    let (backend, cascade_setup): (
        Box<dyn LlmBackend>,
        Option<(ModelTier, Arc<dyn CascadeBackendFactory>, Arc<RoutingTelemetry>)>,
    ) = if cascade_enabled && backend_name.is_none() {
        console.info("Cascade mode enabled");

        // Create router
        let router = ConservativeRouter::new();

        // Load conversation for routing decision
        // For now, route based on empty conversation
        // TODO: If continue_last, load last message for routing
        let temp_conv = Conversation::new();
        let initial_tier = router.suggest_tier(&temp_conv);
        let confidence = router.routing_confidence(&temp_conv);

        console.info(&format!(
            "Starting with {} tier (confidence: {:.1}%)",
            initial_tier,
            confidence * 100.0
        ));

        // Create factory
        let factory = Arc::new(ConfigBackedFactory::new(config.clone())?) as Arc<dyn CascadeBackendFactory>;

        // Create initial backend
        let backend = factory.create_for_tier(initial_tier)?;

        (
            backend,
            Some((initial_tier, factory.clone(), telemetry.clone())),
        )
    } else {
        // Non-cascade mode
        let backend_name = backend_name.unwrap_or_else(|| config.default_backend.clone());
        console.info(&format!("Using backend: {}", backend_name));
        
        let backend = create_backend(&backend_name, config)?;
        (backend, None)
    };

    backend.initialize().await?;
    let backend_arc = Arc::from(backend);

    // ... existing tool setup code ...

    // Create agent
    let mut agent = Agent::new(backend_arc.clone(), tool_registry, tool_executor)
        .with_max_steps(max_steps);

    // Add cascade support if enabled
    if let Some((initial_tier, factory, tel)) = cascade_setup {
        agent = agent.with_cascade_support(
            initial_tier,
            factory,
            Some(tel.clone()),
        );

        // Create routing decision for telemetry (only if enabled)
        if tel.is_enabled() {
            let session_id = uuid::Uuid::new_v4().to_string();
            let decision = RoutingDecision::new(
                initial_tier,
                "", // Will be updated with actual message metadata
                0.0, // Will be updated
                session_id,
            );
            agent.set_routing_decision(decision);
        }
    }

    // ... rest of existing agent setup and execution ...

    // Log telemetry at end of session (only if enabled)
    if cascade_enabled {
        if let Some(tel) = agent.telemetry() {
            if tel.is_enabled() {
                if let Some(decision) = agent.routing_decision() {
                    tel.log_decision(&decision).ok();
                }
            }
        }
    }

    Ok(())
}
```

**Acceptance Criteria**:
- [ ] CLI detects cascade config and enables it
- [ ] Shows telemetry prompt on first cascade use
- [ ] Telemetry only logs if user consented
- [ ] No data logged without consent
- [ ] Backward compatible with non-cascade usage

---

### Step 8: Testing & Validation (2-3 hours)

**File**: `tests/cascade_integration_test.rs`

```rust
use hoosh::agent::{Agent, Conversation};
use hoosh::backends::{ConfigBackedFactory, LlmBackend};
use hoosh::config::{AppConfig, BackendConfig, CascadeConfig, ModelTier as ConfigModelTier};
use hoosh::routing::{ConservativeRouter, ModelTier, RoutingTelemetry};
use std::collections::HashMap;
use std::sync::Arc;

fn create_test_config() -> AppConfig {
    let mut backends = HashMap::new();
    
    backends.insert(
        "mock".to_string(),
        BackendConfig {
            api_key: Some("test".to_string()),
            model: Some("test-model".to_string()),
            ..Default::default()
        },
    );

    AppConfig {
        default_backend: "mock".to_string(),
        backends,
        cascade: Some(CascadeConfig {
            enabled: true,
            auto_escalate_on_error: true,
            telemetry_enabled: false, // Disable for tests
            small: ConfigModelTier {
                backend: "mock".to_string(),
                model: Some("small".to_string()),
            },
            medium: ConfigModelTier {
                backend: "mock".to_string(),
                model: Some("medium".to_string()),
            },
            large: ConfigModelTier {
                backend: "mock".to_string(),
                model: Some("large".to_string()),
            },
        }),
        ..Default::default()
    }
}

#[test]
fn test_router_basic_classification() {
    let router = ConservativeRouter::new();
    
    // Test small tier
    let mut conv = Conversation::new();
    conv.add_user_message("What is Rust?".to_string());
    assert_eq!(router.suggest_tier(&conv), ModelTier::Small);
    
    // Test medium tier
    let mut conv = Conversation::new();
    conv.add_user_message("Add error handling to this function that reads files".to_string());
    assert_eq!(router.suggest_tier(&conv), ModelTier::Medium);
    
    // Test large tier
    let mut conv = Conversation::new();
    let long_msg = "x".repeat(2000);
    conv.add_user_message(long_msg);
    assert_eq!(router.suggest_tier(&conv), ModelTier::Large);
}

#[test]
fn test_backend_factory_creation() {
    let config = create_test_config();
    let factory = ConfigBackedFactory::new(config);
    assert!(factory.is_ok());
}

#[test]
fn test_telemetry_logging() {
    let telemetry = RoutingTelemetry::new(false);
    assert!(telemetry.is_ok());
}

// TODO: Add mock backend tests for full agent escalation flow
```

**Manual Testing Checklist**:

- [ ] Test cascade with Anthropic backend (all three tiers)
- [ ] Test simple query → small tier
- [ ] Test complex query → large tier
- [ ] Test escalate tool call → tier upgrade
- [ ] Test auto-escalation on error
- [ ] Test with cascade disabled (backward compat)
- [ ] Test with invalid cascade config → error message

**Telemetry Privacy Testing**:
- [ ] First cascade use shows telemetry prompt
- [ ] `hoosh telemetry status` shows disabled by default
- [ ] `hoosh telemetry enable` shows consent prompt
- [ ] Consent prompt clearly explains what's collected
- [ ] After consent, telemetry file is created
- [ ] Telemetry file contains NO message content
- [ ] Telemetry file contains only metadata (verify with `cat ~/.hoosh/routing_telemetry.jsonl`)
- [ ] `hoosh telemetry disable` stops logging
- [ ] `hoosh telemetry clear` deletes data
- [ ] Consent persists across sessions

---

### Step 9: Documentation (1-2 hours)

**File**: `README.md`

Add section:

```markdown
## Dynamic Model Selection (Cascade Mode)

Hoosh can automatically route tasks to appropriate model tiers based on complexity, optimizing cost and performance.

### How It Works

1. **Conservative Routing**: Analyzes request structure (length, code blocks, etc.)
2. **Starts Small**: Begins with cost-effective models for simple tasks
3. **Self-Correcting**: LLMs can request escalation via the `escalate` tool
4. **Auto-Escalation**: Optionally upgrades on errors

### Configuration

Enable in your `config.toml`:

```toml
[cascade]
enabled = true
auto_escalate_on_error = true

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

### Telemetry (Optional, Opt-In)

Hoosh can collect anonymized routing data to improve accuracy over time:

```bash
# Enable telemetry (shows consent prompt)
hoosh telemetry enable

# Check status
hoosh telemetry status

# Disable telemetry
hoosh telemetry disable

# Clear collected data
hoosh telemetry clear
```

**Privacy Guarantee**:
- ✓ All data stays local (never sent to servers)
- ✓ No message content collected (only metadata)
- ✓ No file paths or personal information
- ✓ Opt-in by explicit consent only
- ✓ Can be disabled or cleared anytime

### Mixed Backends

You can use different backends for different tiers:

```toml
[cascade.small]
backend = "together"
model = "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo"

[cascade.medium]
backend = "anthropic"
model = "claude-sonnet-4-20250514"

[cascade.large]
backend = "openai"
model = "gpt-4-turbo"
```
```

**File**: `docs/cascade-system.md`

Add privacy section:

```markdown
## Telemetry & Privacy

### What We Collect

When telemetry is enabled (opt-in), Hoosh logs:
- Routing tier selections
- Escalation events  
- Message metadata: length, word count, has_code_block, question_count
- Routing confidence scores
- Anonymous session IDs (random UUIDs)

### What We DON'T Collect

- ✗ Message content or code
- ✗ File paths or names
- ✗ API keys or credentials
- ✗ Personal information
- ✗ IP addresses or network data

### Data Storage

All telemetry is stored locally:
```
~/.hoosh/routing_telemetry.jsonl  # Routing decisions
~/.hoosh/telemetry_consent.json   # Consent state
```

Data is never sent to external servers.

### Consent Management

```bash
# Enable with consent prompt
hoosh telemetry enable

# Check current status
hoosh telemetry status

# Disable collection
hoosh telemetry disable

# Delete all data
hoosh telemetry clear
```

### Data Format Example

```json
{
  "timestamp": "2024-01-15T10:30:00Z",
  "initial_tier": "medium",
  "final_tier": "large",
  "escalation_count": 1,
  "message_length": 450,
  "message_word_count": 75,
  "has_code_block": true,
  "question_count": 1,
  "line_count": 12,
  "routing_confidence": 0.65,
  "session_id": "abc-123-def-456"
}
```

Note: No message content is present.

### Why Telemetry?

Telemetry helps improve routing accuracy:
- Identify common routing mistakes
- Tune confidence thresholds
- Expand reference examples
- Track cascade effectiveness

All improvements benefit all users while respecting individual privacy.
```

```markdown
# Cascade System Documentation

## Overview

The cascade system enables dynamic model selection based on task complexity, optimizing for cost and capability.

## Architecture

### Components

1. **ConservativeRouter**: Analyzes requests and suggests initial tier
2. **CascadeBackendFactory**: Creates backends for different tiers
3. **EscalateTool**: Allows LLMs to request more capable models
4. **RoutingTelemetry**: Logs decisions for analysis

### Routing Logic

The router uses structural analysis:
- Message length and complexity
- Code block presence
- Multi-step indicators
- Historical context

**No keyword matching** - avoids brittle pattern dependencies.

### Default Behavior

- Short queries (<100 chars, <20 words) → Small
- Very detailed (>1500 chars, >400 words) → Large
- Everything else → Medium (safe default)

## Usage

### Basic Setup

1. Configure tiers in `config.toml`
2. Set `enabled = true`
3. Run normally - cascade is automatic

### The Escalate Tool

LLMs can call `escalate` when tasks exceed their capability:

```json
{
  "tool": "escalate",
  "arguments": {
    "reason": "Task requires complex architectural reasoning beyond my current tier"
  }
}
```

### Auto-Escalation

When `auto_escalate_on_error = true`, the system automatically upgrades tiers on errors.

## Telemetry

Routing decisions are logged to `~/.hoosh/routing_telemetry.jsonl`:

```json
{
  "timestamp": "2024-01-15T10:30:00Z",
  "initial_tier": "medium",
  "final_tier": "large",
  "escalation_count": 1,
  "message_length": 450,
  "message_preview": "Design a comprehensive authentication system...",
  "routing_confidence": 0.65,
  "session_id": "abc-123-def"
}
```

Use this data to:
- Analyze routing accuracy
- Identify patterns in misrouted requests
- Tune thresholds (Phase 2)

## Best Practices

1. **Start conservative**: Default to Medium for ambiguous cases
2. **Trust the escalate tool**: LLMs are good at knowing their limits
3. **Review telemetry**: Periodically check routing decisions
4. **Mixed backends**: Use cheap local models for small tier

## Troubleshooting

### Escalation not working
- Check `cascade.enabled = true`
- Verify backend configs exist for all tiers
- Check logs for factory errors

### Too many escalations
- Router may be too aggressive with Small tier
- Check telemetry for patterns
- Consider adjusting defaults

### Cascade disabled in config but still routing
- Restart session
- Check project config override

## Future Improvements

Phase 2 will add:
- Embedding-based routing
- Adaptive learning from telemetry
- Cost tracking per tier
- De-escalation after complex tasks
```

**Acceptance Criteria**:
- [ ] README explains cascade feature
- [ ] Detailed docs in docs/cascade-system.md
- [ ] Configuration examples documented
- [ ] Troubleshooting guide included

---

## Phase 1 Completion Checklist

### Code
- [ ] All files compile without errors
- [ ] All unit tests pass
- [ ] Integration tests pass
- [ ] No clippy warnings

### Functionality
- [ ] Cascade can be enabled via config
- [ ] Router correctly classifies simple/complex requests
- [ ] Escalate tool triggers tier upgrade
- [ ] Auto-escalation on errors works
- [ ] Backward compatible (disabled by default)

### Privacy & Consent
- [ ] Telemetry disabled by default (opt-in only)
- [ ] `hoosh telemetry enable` shows clear consent prompt
- [ ] Consent prompt explains what is/isn't collected
- [ ] Telemetry respects consent state
- [ ] No data logged without explicit consent
- [ ] `hoosh telemetry status` shows current state
- [ ] `hoosh telemetry disable` stops logging
- [ ] `hoosh telemetry clear` deletes data
- [ ] Telemetry data contains NO message content
- [ ] Only metadata logged (length, word count, etc.)

### Documentation
- [ ] README updated with telemetry consent info
- [ ] Configuration documented
- [ ] Privacy guarantees documented
- [ ] Detailed docs written
- [ ] Example config includes cascade section
- [ ] Telemetry CLI commands documented

### Testing
- [ ] Manual testing with real backends
- [ ] Tested all three tiers
- [ ] Tested escalation flow
- [ ] Tested with cascade disabled
- [ ] Verified telemetry is opt-in
- [ ] Verified no content in telemetry logs
- [ ] Verified consent persistence
- [ ] Tested data deletion

---

## Success Metrics

After Phase 1 completion:

1. **Routing accuracy**: ~70% of requests route to appropriate tier initially
2. **Escalation rate**: <20% of sessions require escalation
3. **Cost savings**: Small tier handles 20-30% of simple requests
4. **Zero regressions**: Existing functionality unchanged
5. **Privacy**: 100% opt-in consent, zero content leakage
6. **User trust**: Clear, honest communication about data collection

---

## Known Limitations

Phase 1 intentionally omits:
- Keyword-based routing (too brittle)
- ML-based classification (Phase 2)
- Cost tracking (Phase 2)
- De-escalation (Phase 2)
- Per-agent tier configs (Phase 2)

These will be addressed in subsequent phases based on telemetry data.
