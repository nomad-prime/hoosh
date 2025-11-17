use super::BashCommandParser;

#[derive(Debug, Clone, PartialEq)]
pub struct CommandPatternResult {
    pub description: String,
    pub pattern: String,
    pub persistent_message: String,
}

pub trait BashCommandPattern {
    fn matches(&self, command: &str) -> bool;
    fn analyze(&self, command: &str) -> CommandPatternResult;
    fn priority(&self) -> u32;
}

pub struct HeredocPattern;

impl BashCommandPattern for HeredocPattern {
    fn matches(&self, command: &str) -> bool {
        command.contains("<<")
    }

    fn analyze(&self, command: &str) -> CommandPatternResult {
        let base_commands = BashCommandParser::extract_base_commands(command);
        let cmd = base_commands.first().map(|s| s.as_str()).unwrap_or("*");

        CommandPatternResult {
            description: format!("{} with heredoc", cmd),
            pattern: format!("{}:<<", cmd),
            persistent_message: format!(
                "don't ask me again for \"{}\" commands with heredoc (<<) in this project",
                cmd
            ),
        }
    }

    fn priority(&self) -> u32 {
        100
    }
}

pub struct PipelinePattern;

impl BashCommandPattern for PipelinePattern {
    fn matches(&self, command: &str) -> bool {
        command.contains('|') && !command.contains("||")
    }

    fn analyze(&self, command: &str) -> CommandPatternResult {
        let base_commands = BashCommandParser::extract_base_commands(command);

        if base_commands.is_empty() {
            return CommandPatternResult {
                description: "pipeline".to_string(),
                pattern: "*".to_string(),
                persistent_message: "don't ask me again for bash in this project".to_string(),
            };
        }

        if base_commands.iter().all(|c| c == &base_commands[0]) {
            CommandPatternResult {
                description: base_commands[0].clone(),
                pattern: format!("{}:*", base_commands[0]),
                persistent_message: format!(
                    "don't ask me again for \"{}\" commands in this project",
                    base_commands[0]
                ),
            }
        } else {
            let display = base_commands.join(", ");
            let pattern = base_commands
                .iter()
                .map(|cmd| format!("{}:*", cmd))
                .collect::<Vec<_>>()
                .join("|");

            CommandPatternResult {
                description: display.clone(),
                pattern,
                persistent_message: format!(
                    "don't ask me again for pipe combination of \"{}\" commands in this project",
                    display
                ),
            }
        }
    }

    fn priority(&self) -> u32 {
        80
    }
}

pub struct CommandChainPattern;

impl BashCommandPattern for CommandChainPattern {
    fn matches(&self, command: &str) -> bool {
        command.contains("&&") || command.contains("||") || command.contains(';')
    }

    fn analyze(&self, command: &str) -> CommandPatternResult {
        let base_commands = BashCommandParser::extract_base_commands(command);

        if base_commands.is_empty() {
            return CommandPatternResult {
                description: "command chain".to_string(),
                pattern: "*".to_string(),
                persistent_message: "don't ask me again for bash in this project".to_string(),
            };
        }

        if base_commands.iter().all(|c| c == &base_commands[0]) {
            CommandPatternResult {
                description: base_commands[0].clone(),
                pattern: format!("{}:*", base_commands[0]),
                persistent_message: format!(
                    "don't ask me again for \"{}\" commands in this project",
                    base_commands[0]
                ),
            }
        } else {
            let display = base_commands.join(", ");

            CommandPatternResult {
                description: display.clone(),
                pattern: "*".to_string(),
                persistent_message: format!(
                    "don't ask me again for \"{}\" command combinations in this project",
                    display
                ),
            }
        }
    }

    fn priority(&self) -> u32 {
        60
    }
}

pub struct SingleCommandPattern;

impl BashCommandPattern for SingleCommandPattern {
    fn matches(&self, _command: &str) -> bool {
        true
    }

    fn analyze(&self, command: &str) -> CommandPatternResult {
        let base_commands = BashCommandParser::extract_base_commands(command);

        if let Some(cmd) = base_commands.first() {
            CommandPatternResult {
                description: cmd.clone(),
                pattern: format!("{}:*", cmd),
                persistent_message: format!(
                    "don't ask me again for \"{}\" commands in this project",
                    cmd
                ),
            }
        } else {
            CommandPatternResult {
                description: "bash command".to_string(),
                pattern: "*".to_string(),
                persistent_message: "don't ask me again for bash in this project".to_string(),
            }
        }
    }

    fn priority(&self) -> u32 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heredoc_pattern_matches() {
        let pattern = HeredocPattern;
        assert!(pattern.matches("cat <<EOF\nHello\nEOF"));
        assert!(pattern.matches("cat <<'EOF'\nHello\nEOF"));
        assert!(!pattern.matches("cat file.txt"));
    }

    #[test]
    fn test_heredoc_pattern_analyze() {
        let pattern = HeredocPattern;
        let result = pattern.analyze("cat <<EOF\nHello\nEOF");
        assert_eq!(result.pattern, "cat:<<");
        assert!(result.persistent_message.contains("heredoc"));
    }

    #[test]
    fn test_pipeline_pattern_matches() {
        let pattern = PipelinePattern;
        assert!(pattern.matches("cat file | grep error"));
        assert!(!pattern.matches("cat file || echo failed"));
        assert!(!pattern.matches("cat file.txt"));
    }

    #[test]
    fn test_pipeline_pattern_same_command() {
        let pattern = PipelinePattern;
        let result = pattern.analyze("cat file | cat");
        assert_eq!(result.pattern, "cat:*");
    }

    #[test]
    fn test_pipeline_pattern_different_commands() {
        let pattern = PipelinePattern;
        let result = pattern.analyze("cat file | grep error | wc -l");
        assert_eq!(result.pattern, "cat:*|grep:*|wc:*");
        assert!(result.persistent_message.contains("pipe combination"));
    }

    #[test]
    fn test_command_chain_pattern_matches() {
        let pattern = CommandChainPattern;
        assert!(pattern.matches("cargo build && cargo test"));
        assert!(pattern.matches("ls || echo failed"));
        assert!(pattern.matches("ls; pwd; echo done"));
        assert!(!pattern.matches("ls -la"));
    }

    #[test]
    fn test_command_chain_pattern_same_command() {
        let pattern = CommandChainPattern;
        let result = pattern.analyze("cargo build && cargo test");
        assert_eq!(result.pattern, "cargo:*");
    }

    #[test]
    fn test_single_command_pattern() {
        let pattern = SingleCommandPattern;
        assert!(pattern.matches("ls -la"));

        let result = pattern.analyze("ls -la");
        assert_eq!(result.pattern, "ls:*");
        assert!(result.persistent_message.contains("ls"));
    }

    #[test]
    fn test_pattern_priorities() {
        assert!(HeredocPattern.priority() > PipelinePattern.priority());
        assert!(PipelinePattern.priority() > CommandChainPattern.priority());
        assert!(CommandChainPattern.priority() > SingleCommandPattern.priority());
    }
}
