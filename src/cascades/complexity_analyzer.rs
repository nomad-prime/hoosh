use crate::cascades::{ComplexityLevel, ComplexitySignals, ExecutionTier, TaskComplexity};

pub trait ComplexityAnalyzer {
    fn analyze(&self, task_description: &str) -> Result<TaskComplexity, String>;
}

#[derive(Debug, Clone)]
pub struct DefaultComplexityAnalyzer {
    pub simple_depth_threshold: f32,
    pub moderate_depth_threshold: f32,
}

impl DefaultComplexityAnalyzer {
    pub fn new() -> Self {
        Self {
            simple_depth_threshold: 0.25,
            moderate_depth_threshold: 0.50,
        }
    }

    fn calculate_structural_depth(text: &str) -> f32 {
        let mut depth = 0.0;

        let lines: Vec<&str> = text.lines().collect();
        let line_count = lines.len() as f32;
        if line_count > 5.0 {
            depth += 0.1;
        }
        if line_count > 20.0 {
            depth += 0.15;
        }

        let brace_count = text.matches('{').count() as f32;
        let bracket_count = text.matches('[').count() as f32;
        depth += (brace_count + bracket_count) * 0.05;

        if text.contains("if ") || text.contains("if(") {
            depth += 0.1;
        }
        if text.contains("match ") || text.contains("switch ") {
            depth += 0.15;
        }
        if text.contains("loop ") || text.contains("for ") || text.contains("while ") {
            depth += 0.15;
        }

        if text.contains("recursive") || text.contains("recursion") {
            depth += 0.2;
        }

        depth.min(1.0)
    }

    fn calculate_action_density(text: &str) -> f32 {
        let word_count = text.split_whitespace().count() as f32;
        let mut density = (word_count / 100.0).min(1.0);

        let tasks = [
            "create", "modify", "delete", "read", "write", "update", "implement", "refactor",
            "add", "remove", "fix", "test", "verify", "validate", "debug", "analyze", "generate",
        ];

        for task in tasks.iter() {
            if text.to_lowercase().contains(task) {
                density += 0.05;
            }
        }

        density.min(1.0)
    }

    fn calculate_code_signals(text: &str) -> f32 {
        let mut signals: f32 = 0.0;

        if text.contains("```") || text.contains("code") {
            signals += 0.2;
        }
        if text.contains("fn ") || text.contains("function") {
            signals += 0.15;
        }
        if text.contains("struct ") || text.contains("class ") {
            signals += 0.15;
        }
        if text.contains("trait ") || text.contains("interface ") {
            signals += 0.15;
        }
        if text.contains("test") || text.contains("unit test") {
            signals += 0.1;
        }

        signals.min(1.0)
    }

    fn calculate_concept_count(text: &str) -> f32 {
        let mut concepts: f32 = 0.0;

        if text.contains("error") || text.contains("exception") {
            concepts += 0.1;
        }
        if text.contains("state") {
            concepts += 0.1;
        }
        if text.contains("dependency") || text.contains("dependencies") {
            concepts += 0.1;
        }
        if text.contains("integration") || text.contains("interop") {
            concepts += 0.15;
        }
        if text.contains("security") || text.contains("authentication") {
            concepts += 0.15;
        }
        if text.contains("performance") || text.contains("optimization") {
            concepts += 0.1;
        }
        if text.contains("concurrency") || text.contains("async") || text.contains("parallel") {
            concepts += 0.2;
        }

        concepts.min(1.0)
    }
}

impl Default for DefaultComplexityAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl ComplexityAnalyzer for DefaultComplexityAnalyzer {
    fn analyze(&self, task_description: &str) -> Result<TaskComplexity, String> {
        let structural_depth = Self::calculate_structural_depth(task_description);
        let action_density = Self::calculate_action_density(task_description);
        let code_signals = Self::calculate_code_signals(task_description);
        let concept_count = Self::calculate_concept_count(task_description);

        let signals = ComplexitySignals {
            structural_depth,
            action_density,
            code_signals,
            concept_count,
        };

        let overall_score = (structural_depth + action_density + code_signals + concept_count) / 4.0;

        let (level, tier, reasoning) = if overall_score < self.simple_depth_threshold {
            (
                ComplexityLevel::Simple,
                ExecutionTier::Light,
                format!(
                    "Task classified as simple based on low complexity signals (score: {:.2})",
                    overall_score
                ),
            )
        } else if overall_score < self.moderate_depth_threshold {
            (
                ComplexityLevel::Moderate,
                ExecutionTier::Medium,
                format!(
                    "Task classified as moderate based on moderate complexity signals (score: {:.2})",
                    overall_score
                ),
            )
        } else {
            (
                ComplexityLevel::Complex,
                ExecutionTier::Heavy,
                format!(
                    "Task classified as complex based on high complexity signals (score: {:.2})",
                    overall_score
                ),
            )
        };

        Ok(TaskComplexity::new(
            level,
            tier,
            overall_score,
            reasoning,
            signals,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_returns_result() {
        let analyzer = DefaultComplexityAnalyzer::new();
        let result = analyzer
            .analyze("Read a file and print its contents")
            .expect("Should analyze successfully");

        assert!(!result.reasoning.is_empty());
        assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
    }

    #[test]
    fn test_analyze_simple_vs_complex_scoring() {
        let analyzer = DefaultComplexityAnalyzer::new();

        let simple = analyzer
            .analyze("Read file")
            .expect("Should analyze");

        let complex = analyzer
            .analyze("Design recursive state machine with async error handling, security and testing")
            .expect("Should analyze");

        // Complex should have higher confidence than simple
        assert!(complex.confidence > simple.confidence);
    }

    #[test]
    fn test_analyze_incremental_complexity() {
        let analyzer = DefaultComplexityAnalyzer::new();

        let result1 = analyzer.analyze("Read file").expect("Should analyze");
        let result2 = analyzer.analyze("Read file and validate").expect("Should analyze");
        let result3 = analyzer
            .analyze("Read file, validate format, handle errors, test coverage")
            .expect("Should analyze");

        // Scores should increase with more complexity indicators
        assert!(result2.confidence >= result1.confidence);
        assert!(result3.confidence >= result2.confidence);
    }
}
