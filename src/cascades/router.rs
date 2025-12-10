use crate::cascades::{ComplexityLevel, ExecutionTier, TaskComplexity};

pub trait CascadeRouter {
    fn route(&self, complexity: &TaskComplexity) -> Result<ExecutionTier, String>;
}

#[derive(Debug, Clone)]
pub struct DefaultCascadeRouter {
    pub default_tier: ExecutionTier,
    pub confidence_threshold: f32,
}

impl DefaultCascadeRouter {
    pub fn new() -> Self {
        Self {
            default_tier: ExecutionTier::Medium,
            confidence_threshold: 0.7,
        }
    }

    pub fn with_threshold(threshold: f32) -> Self {
        Self {
            default_tier: ExecutionTier::Medium,
            confidence_threshold: threshold.clamp(0.0, 1.0),
        }
    }
}

impl Default for DefaultCascadeRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl CascadeRouter for DefaultCascadeRouter {
    fn route(&self, complexity: &TaskComplexity) -> Result<ExecutionTier, String> {
        if complexity.confidence < self.confidence_threshold {
            return Ok(self.default_tier);
        }

        let tier = match complexity.level {
            ComplexityLevel::Simple => ExecutionTier::Light,
            ComplexityLevel::Moderate => ExecutionTier::Medium,
            ComplexityLevel::Complex => ExecutionTier::Heavy,
        };

        Ok(tier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cascades::ComplexitySignals;

    fn create_test_complexity(
        level: ComplexityLevel,
        confidence: f32,
    ) -> TaskComplexity {
        let signals = ComplexitySignals {
            structural_depth: 0.3,
            action_density: 0.4,
            code_signals: 0.2,
            concept_count: 0.1,
        };
        TaskComplexity::new(
            level,
            ExecutionTier::Medium,
            confidence,
            "Test".to_string(),
            signals,
        )
    }

    #[test]
    fn test_route_simple_high_confidence() {
        let router = DefaultCascadeRouter::new();
        let complexity = create_test_complexity(ComplexityLevel::Simple, 0.9);
        let tier = router.route(&complexity).expect("Should route successfully");
        assert_eq!(tier, ExecutionTier::Light);
    }

    #[test]
    fn test_route_complex_high_confidence() {
        let router = DefaultCascadeRouter::new();
        let complexity = create_test_complexity(ComplexityLevel::Complex, 0.9);
        let tier = router.route(&complexity).expect("Should route successfully");
        assert_eq!(tier, ExecutionTier::Heavy);
    }

    #[test]
    fn test_route_low_confidence_uses_default() {
        let router = DefaultCascadeRouter::new();
        let complexity = create_test_complexity(ComplexityLevel::Simple, 0.5);
        let tier = router.route(&complexity).expect("Should route successfully");
        assert_eq!(tier, ExecutionTier::Medium); // conservative default
    }

    #[test]
    fn test_route_at_confidence_threshold() {
        let router = DefaultCascadeRouter::new();
        let complexity = create_test_complexity(ComplexityLevel::Moderate, 0.7);
        let tier = router.route(&complexity).expect("Should route successfully");
        assert_eq!(tier, ExecutionTier::Medium);
    }
}
