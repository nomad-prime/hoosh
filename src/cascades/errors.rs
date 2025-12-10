use std::fmt;

#[derive(Debug, Clone)]
pub enum CascadeError {
    InvalidConfig(String),
    CascadesDisabled,
    AnalysisError(String),
    RoutingError(String),
    EscalationError(String),
    ModelTierNotFound(String),
    ContextError(String),
    AlreadyAtMaxTier,
}

impl fmt::Display for CascadeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CascadeError::InvalidConfig(msg) => write!(f, "Invalid cascade config: {}", msg),
            CascadeError::CascadesDisabled => write!(f, "Cascades feature is disabled"),
            CascadeError::AnalysisError(msg) => write!(f, "Analysis error: {}", msg),
            CascadeError::RoutingError(msg) => write!(f, "Routing error: {}", msg),
            CascadeError::EscalationError(msg) => write!(f, "Escalation error: {}", msg),
            CascadeError::ModelTierNotFound(tier) => write!(f, "Model tier not found: {}", tier),
            CascadeError::ContextError(msg) => write!(f, "Context error: {}", msg),
            CascadeError::AlreadyAtMaxTier => write!(f, "Already at maximum tier"),
        }
    }
}

impl std::error::Error for CascadeError {}
