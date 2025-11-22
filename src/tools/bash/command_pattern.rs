use super::BashCommandParser;

#[derive(Debug, Clone, PartialEq)]
pub struct CommandPatternResult {
    pub description: String,
    pub pattern: String,
    pub persistent_message: String,
    pub safe: bool,
}

pub trait BashCommandPattern: Send + Sync {
    fn matches(&self, command: &str) -> bool;
    fn matches_pattern(&self, pattern: &str, command: &str) -> bool;
    fn analyze(&self, command: &str) -> CommandPatternResult;
    fn priority(&self) -> u32;
}

pub struct SubshellPattern;

impl BashCommandPattern for SubshellPattern {
    fn matches(&self, command: &str) -> bool {
        BashCommandParser::contains_subshell(command)
    }

    fn matches_pattern(&self, pattern: &str, command: &str) -> bool {
        // Subshell patterns always use wildcard
        pattern == "*" && BashCommandParser::contains_subshell(command)
    }
    fn analyze(&self, _command: &str) -> CommandPatternResult {
        CommandPatternResult {
            description: "command with subshell execution".to_string(),
            pattern: "*".to_string(), // Too dynamic to pattern match specific commands
            persistent_message: "don't ask me again for complex shell expansions (Be careful! this allows for arbitrary code execution)".to_string(),
            safe: false, // NEVER safe to auto-approve subshells
        }
    }

    fn priority(&self) -> u32 {
        90 // High priority, just below Heredoc
    }
}

pub struct RedirectionPattern;

impl BashCommandPattern for RedirectionPattern {
    fn matches(&self, command: &str) -> bool {
        // Match > or < but not << (heredoc)
        (command.contains('>') || command.contains('<')) && !command.contains("<<")
    }

    fn analyze(&self, command: &str) -> CommandPatternResult {
        let base_commands = BashCommandParser::extract_base_commands(command);
        let cmd = base_commands.first().map(|s| s.as_str()).unwrap_or("*");

        CommandPatternResult {
            description: format!("{} with redirection", cmd),
            pattern: format!("{}:>", cmd),
            persistent_message: format!(
                "don't ask me again for \"{}\" commands with redirection (>, <) in this project",
                cmd
            ),
            safe: false,
        }
    }

    fn priority(&self) -> u32 {
        70
    }

    fn matches_pattern(&self, pattern: &str, command: &str) -> bool {
        if let Some(cmd) = pattern.strip_suffix(":>") {
            command.trim().starts_with(cmd) && command.contains('>')
        } else {
            false
        }
    }
}

pub struct HeredocPattern;

impl BashCommandPattern for HeredocPattern {
    fn matches(&self, command: &str) -> bool {
        BashCommandParser::contains_heredoc(command)
    }

    fn matches_pattern(&self, pattern: &str, command: &str) -> bool {
        if let Some(prefix) = pattern.strip_suffix(":<<") {
            if prefix == "*" {
                return command.contains("<<");
            }
            let clean_target = command.trim();
            if let Some(rest) = clean_target.strip_prefix(prefix) {
                let valid_word_boundary = rest.is_empty() || rest.starts_with(' ');
                return valid_word_boundary && command.contains("<<");
            }
            false
        } else {
            false
        }
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
            safe: false,
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

    fn matches_pattern(&self, pattern: &str, command: &str) -> bool {
        if !pattern.contains('|') {
            return false;
        }

        let pattern_commands: Vec<&str> = pattern
            .split('|')
            .map(|p| p.trim().trim_end_matches(":*").trim_end_matches('*'))
            .filter(|s| !s.is_empty())
            .collect();

        let target_commands = BashCommandParser::extract_base_commands(command);

        pattern_commands.iter().all(|pattern_cmd| {
            target_commands
                .iter()
                .any(|target_cmd| target_cmd == pattern_cmd)
        })
    }

    fn analyze(&self, command: &str) -> CommandPatternResult {
        let base_commands = BashCommandParser::extract_base_commands(command);

        if base_commands.is_empty() {
            return CommandPatternResult {
                description: "pipeline".to_string(),
                pattern: "*".to_string(),
                persistent_message: "don't ask me again for bash in this project".to_string(),
                safe: false,
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
                safe: false,
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
                safe: false,
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

    fn matches_pattern(&self, pattern: &str, command: &str) -> bool {
        // Chain patterns that collapse to single command use ":*" suffix
        if let Some(prefix) = pattern.strip_suffix(":*") {
            let clean_target = command.trim();
            if let Some(rest) = clean_target.strip_prefix(prefix) {
                return rest.is_empty() || rest.starts_with(' ');
            }
            false
        } else {
            // Wildcard patterns
            pattern == "*"
        }
    }

    fn analyze(&self, command: &str) -> CommandPatternResult {
        let base_commands = BashCommandParser::extract_base_commands(command);

        if base_commands.is_empty() {
            return CommandPatternResult {
                description: "command chain".to_string(),
                pattern: "*".to_string(),
                persistent_message: "don't ask me again for bash in this project".to_string(),
                safe: false,
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
                safe: false,
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
                safe: false,
            }
        }
    }

    fn priority(&self) -> u32 {
        60
    }
}

pub struct SingleCommandPattern;

impl SingleCommandPattern {
    fn is_whitelisted(cmd: &str, full_command: &str) -> bool {
        match cmd {
            // Always safe (information only)
            "ls" | "pwd" | "whoami" | "date" | "echo" | "which" | "type" | "hostname" => {
                !full_command.contains('>')
            },

            // Safe read-only text processing (unless redirecting output)
            "cat" | "head" | "tail" | "less" | "more" | "grep" | "wc" | "sort" | "uniq"
            | "diff" => {
                // PREVENT: cat file.txt > overwritten_file.txt
                !full_command.contains('>')
            }

            // Find is DANGEROUS if used with exec/delete
            "find" => {
                !full_command.contains("-exec")
                    && !full_command.contains("-delete")
                    && !full_command.contains("-ok")
            }

            // Everything else is assumed unsafe for auto-approval
            _ => false,
        }
    }
}

impl BashCommandPattern for SingleCommandPattern {
    fn matches(&self, _command: &str) -> bool {
        true
    }

    fn matches_pattern(&self, pattern: &str, command: &str) -> bool {
        if let Some(prefix) = pattern.strip_suffix(":*") {
            let clean_target = command.trim();
            if let Some(rest) = clean_target.strip_prefix(prefix) {
                return rest.is_empty() || rest.starts_with(' ');
            }
            false
        } else {
            pattern == command
        }
    }

    fn analyze(&self, command: &str) -> CommandPatternResult {
        // Use the new helper to get the subcommand
        if let Some((cmd, arg_opt)) = BashCommandParser::extract_first_command_and_arg(command) {
            // Determine safety (existing logic)
            let safe = Self::is_whitelisted(&cmd, command);

            // Format the pattern: "cargo build:*" vs "ls:*"
            let pattern = if let Some(arg) = &arg_opt {
                format!("{} {}:*", cmd, arg)
            } else {
                format!("{}:*", cmd)
            };

            let description = if let Some(arg) = &arg_opt {
                format!("{} {}", cmd, arg)
            } else {
                cmd.clone()
            };

            CommandPatternResult {
                description: description.clone(),
                pattern,
                persistent_message: format!(
                    "don't ask me again for \"{}\" commands in this project",
                    description
                ),
                safe,
            }
        } else {
            // Fallback
            CommandPatternResult {
                description: "bash command".to_string(),
                pattern: "*".to_string(),
                persistent_message: "don't ask me again for bash".to_string(),
                safe: false,
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
        assert!(!result.safe);
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
    fn test_single_command_pattern_whitelisted() {
        let pattern = SingleCommandPattern;
        assert!(pattern.matches("ls -la"));

        let result = pattern.analyze("ls -la");
        assert_eq!(result.pattern, "ls -la:*");
        assert!(result.persistent_message.contains("ls"));
        assert!(result.safe);
    }

    #[test]
    fn test_single_command_pattern_not_whitelisted() {
        let pattern = SingleCommandPattern;
        let result = pattern.analyze("cargo build");
        assert_eq!(result.pattern, "cargo build:*");
        assert!(!result.safe);
    }

    #[test]
    fn test_whitelist_coverage() {
        let safe_commands = vec![
            "ls", "pwd", "cat", "head", "tail", "find", "grep", "wc", "sort", "echo", "which",
            "date",
        ];

        for cmd in safe_commands {
            assert!(SingleCommandPattern::is_whitelisted(cmd, cmd));
        }
    }

    #[test]
    fn test_non_whitelisted_commands() {
        let unsafe_commands = vec!["cargo", "sed", "rm", "xargs", "docker"];

        for cmd in unsafe_commands {
            assert!(!SingleCommandPattern::is_whitelisted(cmd, cmd));
        }
    }

    #[test]
    fn test_pattern_priorities() {
        assert!(HeredocPattern.priority() > PipelinePattern.priority());
        assert!(PipelinePattern.priority() > CommandChainPattern.priority());
        assert!(CommandChainPattern.priority() > SingleCommandPattern.priority());
    }

    // ===========================================
    // SubshellPattern matches_pattern tests
    // ===========================================

    #[test]
    fn test_subshell_matches_pattern_wildcard() {
        let pattern = SubshellPattern;
        // Subshell patterns only match wildcard "*"
        assert!(pattern.matches_pattern("*", "echo $(whoami)"));
        assert!(pattern.matches_pattern("*", "cat `pwd`"));
        assert!(!pattern.matches_pattern("*", "echo hello")); // No subshell
    }

    #[test]
    fn test_subshell_matches_pattern_non_wildcard() {
        let pattern = SubshellPattern;
        // Non-wildcard patterns should not match
        assert!(!pattern.matches_pattern("echo:*", "echo $(whoami)"));
        assert!(!pattern.matches_pattern("cat:*", "cat `pwd`"));
    }

    // ===========================================
    // RedirectionPattern matches_pattern tests
    // ===========================================

    #[test]
    fn test_redirection_matches_pattern_basic() {
        let pattern = RedirectionPattern;
        assert!(pattern.matches_pattern("echo:>", "echo test > file.txt"));
        assert!(pattern.matches_pattern("echo:>", "echo test >> file.txt"));
        assert!(pattern.matches_pattern("cat:>", "cat input.txt > output.txt"));
    }

    #[test]
    fn test_redirection_matches_pattern_no_redirection() {
        let pattern = RedirectionPattern;
        assert!(!pattern.matches_pattern("echo:>", "echo test"));
        assert!(!pattern.matches_pattern("cat:>", "cat file.txt"));
    }

    #[test]
    fn test_redirection_matches_pattern_wrong_command() {
        let pattern = RedirectionPattern;
        assert!(!pattern.matches_pattern("cat:>", "echo test > file.txt"));
        assert!(!pattern.matches_pattern("echo:>", "cat input > output"));
    }

    #[test]
    fn test_redirection_matches_pattern_invalid_pattern() {
        let pattern = RedirectionPattern;
        // Pattern must end with ":>"
        assert!(!pattern.matches_pattern("echo", "echo test > file.txt"));
        assert!(!pattern.matches_pattern("echo:*", "echo test > file.txt"));
    }

    // ===========================================
    // HeredocPattern matches_pattern tests
    // ===========================================

    #[test]
    fn test_heredoc_matches_pattern_specific_command() {
        let pattern = HeredocPattern;
        assert!(pattern.matches_pattern("cat:<<", "cat <<EOF\ntest\nEOF"));
        assert!(pattern.matches_pattern("cat:<<", "cat <<'EOF'\ntest\nEOF"));
        assert!(pattern.matches_pattern("mysql:<<", "mysql -u root <<EOF\nSELECT 1;\nEOF"));
    }

    #[test]
    fn test_heredoc_matches_pattern_wildcard() {
        let pattern = HeredocPattern;
        assert!(pattern.matches_pattern("*:<<", "cat <<EOF\ntest\nEOF"));
        assert!(pattern.matches_pattern("*:<<", "mysql <<EOF\nquery\nEOF"));
    }

    #[test]
    fn test_heredoc_matches_pattern_no_heredoc() {
        let pattern = HeredocPattern;
        assert!(!pattern.matches_pattern("cat:<<", "cat file.txt"));
        assert!(!pattern.matches_pattern("*:<<", "cat file.txt"));
    }

    #[test]
    fn test_heredoc_matches_pattern_wrong_command() {
        let pattern = HeredocPattern;
        assert!(!pattern.matches_pattern("cat:<<", "grep <<EOF\npattern\nEOF"));
        assert!(!pattern.matches_pattern("mysql:<<", "cat <<EOF\ntest\nEOF"));
    }

    #[test]
    fn test_heredoc_matches_pattern_word_boundary() {
        let pattern = HeredocPattern;
        // "cat" should not match "catch" or "catalog"
        assert!(!pattern.matches_pattern("cat:<<", "catch <<EOF\ntest\nEOF"));
        assert!(pattern.matches_pattern("cat:<<", "cat <<EOF\ntest\nEOF"));
    }

    // ===========================================
    // PipelinePattern matches_pattern tests
    // ===========================================

    #[test]
    fn test_pipeline_matches_pattern_all_commands_present() {
        let pattern = PipelinePattern;
        assert!(pattern.matches_pattern("cat:*|grep:*|wc:*", "cat file | grep error | wc -l"));
        assert!(pattern.matches_pattern("cat:*|grep:*", "cat file | grep pattern"));
    }

    #[test]
    fn test_pipeline_matches_pattern_missing_command() {
        let pattern = PipelinePattern;
        // Pattern requires cat, grep, wc but command only has cat and wc
        assert!(!pattern.matches_pattern("cat:*|grep:*|wc:*", "cat file | wc -l"));
        assert!(!pattern.matches_pattern("cat:*|grep:*", "cat file | wc -l"));
    }

    #[test]
    fn test_pipeline_matches_pattern_order_independent() {
        let pattern = PipelinePattern;
        // Order in the command doesn't need to match order in pattern
        assert!(pattern.matches_pattern("cat:*|grep:*|wc:*", "wc -l | cat file | grep error"));
    }

    #[test]
    fn test_pipeline_matches_pattern_non_pipe_pattern() {
        let pattern = PipelinePattern;
        // Pattern without pipe should not match
        assert!(!pattern.matches_pattern("cat:*", "cat file | grep error"));
        assert!(!pattern.matches_pattern("grep:*", "cat file | grep error"));
    }

    #[test]
    fn test_pipeline_matches_pattern_with_spaces() {
        let pattern = PipelinePattern;
        assert!(pattern.matches_pattern("cat:* | grep:*", "cat file | grep pattern"));
    }

    // ===========================================
    // CommandChainPattern matches_pattern tests
    // ===========================================

    #[test]
    fn test_command_chain_matches_pattern_prefix() {
        let pattern = CommandChainPattern;
        assert!(pattern.matches_pattern("cargo:*", "cargo build && cargo test"));
        assert!(pattern.matches_pattern("git:*", "git add . && git commit -m 'msg'"));
    }

    #[test]
    fn test_command_chain_matches_pattern_wildcard() {
        let pattern = CommandChainPattern;
        assert!(pattern.matches_pattern("*", "cargo build && npm install"));
        assert!(pattern.matches_pattern("*", "ls; pwd; echo done"));
    }

    #[test]
    fn test_command_chain_matches_pattern_word_boundary() {
        let pattern = CommandChainPattern;
        // "cargo" should not match "cargoship"
        assert!(!pattern.matches_pattern("cargo:*", "cargoship && test"));
        assert!(pattern.matches_pattern("cargo:*", "cargo build"));
    }

    #[test]
    fn test_command_chain_matches_pattern_with_args() {
        let pattern = CommandChainPattern;
        assert!(pattern.matches_pattern("cargo:*", "cargo build --release && cargo test"));
    }

    // ===========================================
    // SingleCommandPattern matches_pattern tests
    // ===========================================

    #[test]
    fn test_single_command_matches_pattern_prefix() {
        let pattern = SingleCommandPattern;
        assert!(pattern.matches_pattern("cargo:*", "cargo build"));
        assert!(pattern.matches_pattern("cargo:*", "cargo test --release"));
        assert!(pattern.matches_pattern("ls:*", "ls -la"));
    }

    #[test]
    fn test_single_command_matches_pattern_exact() {
        let pattern = SingleCommandPattern;
        assert!(pattern.matches_pattern("echo hello", "echo hello"));
        assert!(!pattern.matches_pattern("echo hello", "echo world"));
    }

    #[test]
    fn test_single_command_matches_pattern_word_boundary() {
        let pattern = SingleCommandPattern;
        // "cargo" should not match "cargoship"
        assert!(!pattern.matches_pattern("cargo:*", "cargoship"));
        assert!(!pattern.matches_pattern("ls:*", "lsof"));
        assert!(!pattern.matches_pattern("cat:*", "catch"));
    }

    #[test]
    fn test_single_command_matches_pattern_empty_args() {
        let pattern = SingleCommandPattern;
        assert!(pattern.matches_pattern("ls:*", "ls"));
        assert!(pattern.matches_pattern("pwd:*", "pwd"));
    }

    #[test]
    fn test_single_command_should_not_match_redirections() {
        let pattern = SingleCommandPattern;
        // A simple "echo:*" permission should NOT match commands with redirection
        assert!(
            !pattern.matches_pattern("echo:*", "echo test > file.txt"),
            "echo:* should not match redirection commands"
        );
        assert!(
            !pattern.matches_pattern("cat:*", "cat file > output.txt"),
            "cat:* should not match redirection commands"
        );
    }

    #[test]
    fn test_single_command_should_not_match_chains() {
        let pattern = SingleCommandPattern;
        // A simple "echo:*" permission should NOT match command chains
        assert!(
            !pattern.matches_pattern("echo:*", "echo hello && rm -rf /"),
            "echo:* should not match command chains"
        );
        assert!(
            !pattern.matches_pattern("ls:*", "ls; rm -rf /"),
            "ls:* should not match command chains"
        );
    }

    #[test]
    fn test_single_command_should_not_match_pipes() {
        let pattern = SingleCommandPattern;
        // A simple "cat:*" permission should NOT match pipelines
        assert!(
            !pattern.matches_pattern("cat:*", "cat file | rm -rf /"),
            "cat:* should not match pipelines"
        );
    }

    #[test]
    fn test_single_command_should_not_match_subshells() {
        let pattern = SingleCommandPattern;
        // A simple "echo:*" permission should NOT match subshell commands
        assert!(
            !pattern.matches_pattern("echo:*", "echo $(rm -rf /)"),
            "echo:* should not match subshell commands"
        );
    }

    // ===========================================
    // RedirectionPattern analyze tests
    // ===========================================

    #[test]
    fn test_redirection_pattern_analyze() {
        let pattern = RedirectionPattern;
        let result = pattern.analyze("echo test > file.txt");
        assert_eq!(result.pattern, "echo:>");
        assert!(result.persistent_message.contains("redirection"));
        assert!(!result.safe);
    }

    #[test]
    fn test_redirection_pattern_matches_basic() {
        let pattern = RedirectionPattern;
        assert!(pattern.matches("echo test > file.txt"));
        assert!(pattern.matches("cat < input.txt"));
        assert!(!pattern.matches("cat <<EOF")); // Heredoc, not simple redirection
        assert!(!pattern.matches("echo hello"));
    }

    // ===========================================
    // Integration tests with registry
    // ===========================================

    #[test]
    fn test_registry_matches_pattern_redirection() {
        use super::super::BashCommandPatternRegistry;
        let registry = BashCommandPatternRegistry::new();

        assert!(registry.matches_pattern("echo:>", "echo test > file.txt"));
        assert!(!registry.matches_pattern("echo:>", "echo test"));
    }

    #[test]
    fn test_registry_matches_pattern_heredoc() {
        use super::super::BashCommandPatternRegistry;
        let registry = BashCommandPatternRegistry::new();

        assert!(registry.matches_pattern("cat:<<", "cat <<EOF\ntest\nEOF"));
        assert!(registry.matches_pattern("*:<<", "mysql <<EOF\nquery\nEOF"));
    }

    #[test]
    fn test_registry_matches_pattern_pipeline() {
        use super::super::BashCommandPatternRegistry;
        let registry = BashCommandPatternRegistry::new();

        assert!(registry.matches_pattern("cat:*|grep:*", "cat file | grep pattern"));
        assert!(!registry.matches_pattern("cat:*|grep:*|wc:*", "cat file | grep pattern"));
    }

    #[test]
    fn test_registry_matches_pattern_single_command() {
        use super::super::BashCommandPatternRegistry;
        let registry = BashCommandPatternRegistry::new();

        assert!(registry.matches_pattern("cargo:*", "cargo build"));
        assert!(registry.matches_pattern("cargo:*", "cargo test --release"));
        assert!(!registry.matches_pattern("cargo:*", "npm install"));
    }
}
