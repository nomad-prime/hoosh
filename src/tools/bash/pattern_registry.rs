use super::command_pattern::*;

pub struct BashCommandPatternRegistry {
    patterns: Vec<Box<dyn BashCommandPattern>>,
}

impl BashCommandPatternRegistry {
    pub fn new() -> Self {
        let mut patterns: Vec<Box<dyn BashCommandPattern>> = vec![
            Box::new(HeredocPattern),
            Box::new(PipelinePattern),
            Box::new(CommandChainPattern),
            Box::new(SingleCommandPattern),
        ];

        patterns.sort_by(|a, b| b.priority().cmp(&a.priority()));

        Self { patterns }
    }

    pub fn analyze_command(&self, command: &str) -> CommandPatternResult {
        for pattern in &self.patterns {
            if pattern.matches(command) {
                return pattern.analyze(command);
            }
        }

        CommandPatternResult {
            description: "bash command".to_string(),
            pattern: "*".to_string(),
            persistent_message: "don't ask me again for bash in this project".to_string(),
        }
    }
}

impl Default for BashCommandPatternRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_heredoc_priority() {
        let registry = BashCommandPatternRegistry::new();
        let result = registry.analyze_command("cat <<EOF\nHello\nEOF");
        assert_eq!(result.pattern, "cat:<<");
        assert!(result.persistent_message.contains("heredoc"));
    }

    #[test]
    fn test_registry_pipeline() {
        let registry = BashCommandPatternRegistry::new();
        let result = registry.analyze_command("cat file | grep error | wc -l");
        assert_eq!(result.pattern, "cat:*|grep:*|wc:*");
    }

    #[test]
    fn test_registry_command_chain() {
        let registry = BashCommandPatternRegistry::new();
        let result = registry.analyze_command("cargo build && cargo test");
        assert_eq!(result.pattern, "cargo:*");
    }

    #[test]
    fn test_registry_single_command() {
        let registry = BashCommandPatternRegistry::new();
        let result = registry.analyze_command("ls -la");
        assert_eq!(result.pattern, "ls:*");
    }

    #[test]
    fn test_registry_pattern_order() {
        let registry = BashCommandPatternRegistry::new();

        // Heredoc should take priority over pipeline
        let result = registry.analyze_command("cat <<EOF | grep test\nEOF");
        assert_eq!(result.pattern, "cat:<<");
    }
}
