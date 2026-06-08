use indexmap::IndexSet;

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

        let mut commands = IndexSet::new();
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

        commands.into_iter().collect()
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

    /// Find every `cd <path>` invocation in the command and return the
    /// target arg. Useful for sandbox checks. `cd` with no arg returns
    /// an empty string (which the caller should treat as "go to $HOME",
    /// almost always outside cwd).
    ///
    /// Walks the shlex-tokenized stream, resets on control operators so
    /// that `cd /tmp && cd /var` returns both targets.
    pub fn extract_cd_targets(input: &str) -> Vec<String> {
        let input_to_parse = if Self::contains_heredoc(input) {
            input.lines().next().unwrap_or("").to_string()
        } else {
            input.to_string()
        };
        let tokens = match shlex::split(&input_to_parse) {
            Some(t) => t,
            None => return vec![],
        };

        let mut targets = Vec::new();
        let mut expect_command = true;
        let mut iter = tokens.into_iter().peekable();
        while let Some(token) = iter.next() {
            if Self::is_control_operator(&token) {
                expect_command = true;
                continue;
            }
            if expect_command {
                // skip env var prefixes
                if token.contains('=') && !token.starts_with('-') {
                    continue;
                }
                if token == "cd" {
                    // Grab the next non-flag token as the target; `cd -` is a
                    // valid target (previous dir), not a flag. `cd` with no
                    // arg means $HOME.
                    let next = loop {
                        match iter.peek() {
                            Some(t) if Self::is_control_operator(t) => break String::new(),
                            // `-` alone is a target, not a flag.
                            Some(t) if t == "-" => break iter.next().unwrap(),
                            Some(t) if t.starts_with('-') => {
                                iter.next();
                                continue;
                            }
                            Some(_) => break iter.next().unwrap(),
                            None => break String::new(),
                        }
                    };
                    targets.push(next);
                }
                expect_command = false;
            }
        }
        targets
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
        assert_eq!(cmds, vec!["cd", "./app", "grep"]);
    }

    #[test]
    fn extract_cd_targets_basic() {
        assert_eq!(
            BashCommandParser::extract_cd_targets("cd /tmp && ls"),
            vec!["/tmp"]
        );
        assert_eq!(
            BashCommandParser::extract_cd_targets("cd /tmp && cd /var"),
            vec!["/tmp", "/var"]
        );
        assert_eq!(
            BashCommandParser::extract_cd_targets("ls -la"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn extract_cd_targets_handles_no_arg_and_dash() {
        // `cd` alone → empty string sentinel ($HOME).
        assert_eq!(
            BashCommandParser::extract_cd_targets("cd"),
            vec![String::new()]
        );
        assert_eq!(BashCommandParser::extract_cd_targets("cd -"), vec!["-"]);
    }

    #[test]
    fn extract_cd_targets_skips_flags() {
        // hypothetical `cd -P` — should still find the path
        assert_eq!(
            BashCommandParser::extract_cd_targets("cd -P /tmp"),
            vec!["/tmp"]
        );
    }

    #[test]
    fn test_echo_cat_command() {
        let cmds = BashCommandParser::extract_base_commands(
            "echo \"Hello! This is a test message written using bash.\" > test_message.txt && cat test_message.txt ",
        );
        assert_eq!(cmds, vec!["echo", "cat"]);
    }
}
