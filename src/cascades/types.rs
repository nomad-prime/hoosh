use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub enum ComplexityLevel {
    Simple,
    Moderate,
    Complex,
}

impl std::fmt::Display for ComplexityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComplexityLevel::Simple => write!(f, "simple"),
            ComplexityLevel::Moderate => write!(f, "moderate"),
            ComplexityLevel::Complex => write!(f, "complex"),
        }
    }
}

impl std::str::FromStr for ComplexityLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "simple" => Ok(ComplexityLevel::Simple),
            "moderate" => Ok(ComplexityLevel::Moderate),
            "complex" => Ok(ComplexityLevel::Complex),
            _ => Err(format!("Unknown complexity level: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub enum ExecutionTier {
    Light,
    Medium,
    Heavy,
}

impl std::fmt::Display for ExecutionTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionTier::Light => write!(f, "light"),
            ExecutionTier::Medium => write!(f, "medium"),
            ExecutionTier::Heavy => write!(f, "heavy"),
        }
    }
}

impl std::str::FromStr for ExecutionTier {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "light" => Ok(ExecutionTier::Light),
            "medium" => Ok(ExecutionTier::Medium),
            "heavy" => Ok(ExecutionTier::Heavy),
            _ => Err(format!("Unknown execution tier: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskComplexity {
    pub level: ComplexityLevel,
    pub tier: ExecutionTier,
    pub confidence: f32,
    pub reasoning: String,
    pub signals: ComplexitySignals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexitySignals {
    pub structural_depth: f32,
    pub action_density: f32,
    pub code_signals: f32,
    pub concept_count: f32,
}

impl TaskComplexity {
    pub fn new(
        level: ComplexityLevel,
        tier: ExecutionTier,
        confidence: f32,
        reasoning: String,
        signals: ComplexitySignals,
    ) -> Self {
        Self {
            level,
            tier,
            confidence,
            reasoning,
            signals,
        }
    }
}
