// Trait Contracts for Cascade Module
// 
// Note: This module extends existing Hoosh infrastructure:
// - Tool trait: Defined in src/tools/mod.rs (reused for escalate tool)
// - ApprovalResponse: Defined in src/agent/core.rs (reused for escalation approvals)
// - ApprovalHandler TUI: Defined in src/tui/handlers/approval_handler.rs (reused)
//
// NEW traits defined here are specific to cascade routing and observability.

use anyhow::Result;
use std::time::SystemTime;

/// ComplexityAnalyzer: Measures task complexity across multiple signals
/// 
/// This is a domain-specific analyzer for cascade routing.
/// It analyzes incoming task descriptions to determine appropriate model tier.
pub trait ComplexityAnalyzer {
    /// Analyze task description and return complexity classification + metrics
    /// 
    /// Returns TaskComplexity with:
    /// - level: Light | Medium | Heavy
    /// - confidence: 0.0-1.0 (0.7+ is "high confidence", <0.7 triggers Medium default)
    /// - metrics: detailed breakdown of signals
    fn analyze(&self, task_description: &str) -> Result<TaskComplexity>;
}

/// CascadeRouter: Routes tasks to appropriate model tier based on complexity
/// 
/// This router uses complexity analysis + config to make tier assignment decisions.
/// It is NOT the same as Tool routing; it's specifically for model selection.
pub trait CascadeRouter {
    /// Given complexity analysis, determine initial execution tier
    /// Always returns a tier (never fails); defaults to Medium for ambiguous cases
    fn route(&self, complexity: &TaskComplexity) -> ExecutionTier;
    
    /// Determine if escalation from current tier is valid
    /// Light → Medium → Heavy (no escalation from Heavy)
    fn can_escalate_from(&self, current: &ExecutionTier) -> bool;
    
    /// Get next tier in escalation path
    fn next_tier(&self, current: &ExecutionTier) -> Option<ExecutionTier>;
}

/// CascadeEventLogger: Structured event logging for cascade observability
/// 
/// This logger emits cascade lifecycle events (creation, escalation, completion)
/// for monitoring, debugging, and cost tracking. Events are persisted to JSONL.
pub trait CascadeEventLogger {
    /// Emit cascade lifecycle event (non-blocking; errors logged to stderr)
    async fn emit(&self, event: CascadeEvent) -> Result<()>;
    
    /// Query historical events by filters (for debugging and metrics)
    async fn query(&self, filters: EventFilters) -> Result<Vec<CascadeEvent>>;
}

// ============================================================================
// Data Structures (contracts for data flow between modules)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionTier {
    Light,   // Fast, cheap model (e.g., Haiku)
    Medium,  // Balanced tier (e.g., Sonnet)
    Heavy,   // High-capability model (e.g., Opus)
}

#[derive(Debug, Clone)]
pub struct TaskComplexity {
    pub level: ComplexityLevel,
    pub confidence: f32,          // 0.0-1.0
    pub reasoning: String,
    pub metrics: ComplexityMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplexityLevel {
    Light,
    Medium,
    Heavy,
}

#[derive(Debug, Clone)]
pub struct ComplexityMetrics {
    pub structural_depth: u32,       // 1-5 scale
    pub structural_depth_score: f32, // 0.0-1.0 contribution
    pub action_density: usize,       // verb count
    pub action_density_score: f32,   // 0.0-1.0 contribution
    pub code_signals_score: f32,     // 0.0-1.0 contribution
    pub concept_count: usize,        // unique entities
}

/// Reused from crate::agent::ApprovalResponse
/// When agent requests escalation via escalate tool, response is ApprovalResponse {
///   tool_call_id: <escalate_tool_call_id>,
///   approved: true/false,
///   rejection_reason: optional operator feedback
/// }

#[derive(Debug, Clone)]
pub struct CascadeEvent {
    pub event_id: String,
    pub event_type: CascadeEventType,
    pub task_id: String,
    pub tier: ExecutionTier,
    pub timestamp: SystemTime,
    pub duration_ms: Option<u64>,
    pub reason: String,
    pub metrics: EventMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CascadeEventType {
    CascadeCreated,
    TaskRouted,
    EscalationRequested,
    EscalationApproved,
    EscalationRejected,
    EscalationExecuted,
    TaskCompleted,
    TaskFailed,
}

#[derive(Debug, Clone, Default)]
pub struct EventMetrics {
    pub success: bool,
    pub input_tokens: Option<usize>,
    pub output_tokens: Option<usize>,
    pub escalation_count: u32,
    pub retry_count: u32,
    pub latency_excluding_llm_ms: Option<u64>,
}

/// Query filters for historical event lookup
#[derive(Debug, Clone, Default)]
pub struct EventFilters {
    pub task_id: Option<String>,
    pub tier: Option<ExecutionTier>,
    pub event_type: Option<CascadeEventType>,
    pub success_only: Option<bool>,
    pub time_range: Option<(SystemTime, SystemTime)>,
}

/// Result types specific to cascade operations
#[derive(Debug)]
pub enum CascadeError {
    AlreadyAtMaxTier,
    InvalidCurrentTier(String),
    ApprovalTimeout,
    OperatorRejected(String),
    AnalysisError(String),
    RoutingError(String),
}

impl std::fmt::Display for CascadeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyAtMaxTier => write!(f, "Already at maximum tier (Heavy)"),
            Self::InvalidCurrentTier(t) => write!(f, "Invalid tier: {}", t),
            Self::ApprovalTimeout => write!(f, "Approval request timed out (5 minutes)"),
            Self::OperatorRejected(reason) => write!(f, "Operator rejected escalation: {}", reason),
            Self::AnalysisError(e) => write!(f, "Complexity analysis failed: {}", e),
            Self::RoutingError(e) => write!(f, "Routing error: {}", e),
        }
    }
}

impl std::error::Error for CascadeError {}
