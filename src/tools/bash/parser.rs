use std::collections::HashSet;

pub struct BashCommandParser;

impl BashCommandParser {
    /// Extract base commands using proper tokenization.
    /// Handles:
    /// - Quotes: 'echo "a | b"' -> ["echo"] (not ["echo", "b"])
    /// - Env Vars: 'Start=1 cargo build' -> ["cargo"]
    /// - Chains: 'git commit && git push' -> ["git"]
    pub fn extract_base_commands(input: &str) -> Vec<String> {
        // 1. Heredoc Pre-processing
        // We keep your existing logic to only parse the first line if heredoc exists
        // to prevent parsing the content of the heredoc as commands.
        let input_to_parse = if Self::contains_heredoc(input) {
            input.lines().next().unwrap_or("").to_string()
        } else {
            input.to_string()
        };

        // 2. Tokenization via shlex
        // This handles the quoting logic automatically.
        let tokens = match shlex::split(&input_to_parse) {
            Some(t) => t,
            None => return vec![], // Unbalanced quotes or parse error -> Unsafe to run
        };

        let mut commands = HashSet::new();
        let mut expect_command = true;

        for token in tokens {
            if Self::is_control_operator(&token) {
                expect_command = true;
                continue;
            }

            if expect_command {
                // 3. Handle Environment Variable Prefixes (e.g., RUST_LOG=debug cargo run)
                // If it looks like VAR=VAL, it's not the command yet.
                if token.contains('=') && !token.starts_with('-') {
                    // Edge case check: ensure it's actually a variable assignment
                    // and not a command that happens to have an equal sign (rare but possible)
                    continue;
                }

                // This is our command
                commands.insert(token);
                expect_command = false;
            }

            // If expect_command is false, we are processing arguments.
            // We ignore them until we hit a control operator.
        }

        let mut result: Vec<String> = commands.into_iter().collect();
        result.sort();
        result
    }

    pub fn contains_heredoc(input: &str) -> bool {
        input.contains("<<")
    }

    pub fn contains_subshell(input: &str) -> bool {
        // Basic check for $(...) or backticks `...`
        input.contains("$(") || input.contains('`')
    }

    pub fn suggest_pattern(base_commands: &[String]) -> String {
        if base_commands.is_empty() {
            return "*".to_string();
        }

        // Deduplicate for display
        let mut unique = base_commands.to_vec();
        unique.sort();
        unique.dedup();

        if unique.len() == 1 {
            format!("{}:*", unique[0])
        } else {
            unique
                .iter()
                .map(|cmd| format!("{}:*", cmd))
                .collect::<Vec<_>>()
                .join("|")
        }
    }

    fn is_control_operator(token: &str) -> bool {
        matches!(token, "|" | "||" | "&&" | ";")
    }

    pub fn extract_first_command_and_arg(input: &str) -> Option<(String, Option<String>)> {
        let input_to_parse = if Self::contains_heredoc(input) {
            input.lines().next().unwrap_or("").to_string()
        } else {
            input.to_string()
        };

        let tokens = shlex::split(&input_to_parse)?;

        // Skip env vars (RUST_LOG=1 ...)
        let mut cmd_iter = tokens
            .into_iter()
            .skip_while(|t| t.contains('=') && !t.starts_with('-'));

        let command = cmd_iter.next()?;

        // If the next token is a control operator, there is no argument
        let next_token = cmd_iter.next();
        let argument = next_token.filter(|arg| !Self::is_control_operator(arg));
        Some((command, argument))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quote_protection() {
        // The old parser would fail here and see "grep" as a command
        let cmds =
            BashCommandParser::extract_base_commands("git commit -m 'Fixed bug | grep issue'");
        assert_eq!(cmds, vec!["git"]);
    }

    #[test]
    fn test_env_vars() {
        // The old parser would fail here and think "RUST_LOG=debug" is the command
        let cmds = BashCommandParser::extract_base_commands("RUST_LOG=debug cargo run");
        assert_eq!(cmds, vec!["cargo"]);
    }

    #[test]
    fn test_complex_chain() {
        let cmds = BashCommandParser::extract_base_commands(
            "cd /tmp && RUST_BACKTRACE=1 ./app | grep error",
        );
        assert_eq!(cmds, vec!["./app", "cd", "grep"]);
    }
}
