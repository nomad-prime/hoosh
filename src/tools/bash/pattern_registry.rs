use super::command_pattern::*;

pub struct BashCommandPatternRegistry {
    patterns: Vec<Box<dyn BashCommandPattern>>,
}

impl BashCommandPatternRegistry {
    pub fn new() -> Self {
        let mut patterns: Vec<Box<dyn BashCommandPattern>> = vec![
            Box::new(HeredocPattern),
            Box::new(SubshellPattern),
            Box::new(PipelinePattern),
            Box::new(CommandChainPattern),
            Box::new(SingleCommandPattern),
        ];

        patterns.sort_by_key(|a| std::cmp::Reverse(a.priority()));

        Self { patterns }
    }

    /// Analyze a command and return detailed pattern information including risk assessment
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
            safe: false,
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
        assert!(!result.safe);
    }

    #[test]
    fn test_registry_pipeline_safe() {
        let registry = BashCommandPatternRegistry::new();
        let result = registry.analyze_command("cat file | grep error | wc -l");
        assert_eq!(result.pattern, "cat:*|grep:*|wc:*");
        assert!(!result.safe);
    }

    #[test]
    fn test_registry_command_chain() {
        let registry = BashCommandPatternRegistry::new();
        let result = registry.analyze_command("cargo build && cargo test");
        assert_eq!(result.pattern, "cargo:*");
        assert!(!result.safe);
    }

    #[test]
    fn test_registry_single_command_whitelisted() {
        let registry = BashCommandPatternRegistry::new();
        let result = registry.analyze_command("ls -la");
        assert_eq!(result.pattern, "ls -la:*");
        assert!(result.safe);
    }

    #[test]
    fn test_registry_single_command_not_whitelisted() {
        let registry = BashCommandPatternRegistry::new();
        let result = registry.analyze_command("cargo build");
        assert_eq!(result.pattern, "cargo build:*");
        assert!(!result.safe);
    }

    #[test]
    fn test_registry_pattern_order() {
        let registry = BashCommandPatternRegistry::new();

        // Heredoc should take priority over pipeline
        let result = registry.analyze_command("cat <<EOF | grep test\nEOF");
        assert_eq!(result.pattern, "cat:<<");
        assert!(!result.safe);
    }

    #[test]
    fn test_registry_safe_single_command() {
        let registry = BashCommandPatternRegistry::new();
        assert!(registry.analyze_command("find . -name '*.rs'").safe);
        assert!(registry.analyze_command("cat README.md").safe);
        assert!(registry.analyze_command("pwd").safe);
    }

    #[test]
    fn test_registry_needs_review_commands() {
        let registry = BashCommandPatternRegistry::new();
        assert!(!registry.analyze_command("cargo build").safe);
        assert!(
            !registry
                .analyze_command("sed -i 's/test/TEST/g' file.txt")
                .safe
        );
        assert!(!registry.analyze_command("rm file.txt").safe);
    }

    #[test]
    fn test_security_exploit_subshell_bypass() {
        let registry = BashCommandPatternRegistry::new();

        // THE VULNERABILITY:
        // 1. Parser sees "echo"
        // 2. Whitelist approves "echo"
        // 3. Bash executes "rm" inside $() BEFORE echo runs
        let result = registry.analyze_command("echo $(rm -rf /)");

        // This assertion will FAIL until we apply the fix below
        assert!(
            !result.safe,
            "Subshell injection was incorrectly marked as safe!"
        );
        assert!(result.pattern.contains("subshell") || result.description.contains("subshell"));
    }
}
