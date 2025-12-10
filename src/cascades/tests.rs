#[cfg(test)]
mod integration_tests {
    use crate::cascades::{
        ComplexityAnalyzer, DefaultComplexityAnalyzer, DefaultCascadeRouter, CascadeRouter,
    };

    #[test]
    fn test_full_routing_pipeline() {
        let analyzer = DefaultComplexityAnalyzer::new();
        let router = DefaultCascadeRouter::new();

        let complexity = analyzer
            .analyze("Simple file read operation")
            .expect("Should analyze");

        let tier = router.route(&complexity).expect("Should route");

        assert!(!complexity.reasoning.is_empty());
        assert!(complexity.confidence > 0.0);
        // Tier should be valid
        assert!(["light", "medium", "heavy"].contains(&tier.to_string().as_str()));
    }
}
