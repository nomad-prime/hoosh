use indexmap::IndexSet;

pub struct BashCommandParser;

impl BashCommandParser {
    /// Extract base commands using proper tokenization.
    /// Handles:
    /// - Quotes: 'echo "a | b"' -> ["echo"] (not ["echo", "b"])
    /// - Env Vars: 'Start=1 cargo build' -> ["cargo"]
    /// - Chains: 'git commit && git push' -> ["git"]
    /// - No-space operators: 'cat a|grep b' -> ["cat", "grep"]
    pub fn extract_base_commands(input: &str) -> Vec<String> {
        let mut commands = IndexSet::new();
        for segment in Self::split_subcommands(input) {
            if let Some(cmd) = Self::first_command_token(&segment) {
                commands.insert(cmd);
            }
        }
        commands.into_iter().collect()
    }

    /// Split a command into its top-level subcommands, breaking on `&&`, `||`,
    /// `;` and `|` while respecting quotes and `$( )` / `( )` / backtick
    /// nesting. Operators inside quotes or subshells are NOT split, so
    /// `echo $(a | b)` and `git commit -m 'x | y'` each stay a single segment.
    /// Redirections (`2>&1`, `>`) are left attached to their segment.
    ///
    /// This is a deliberately small, heuristic splitter built on `shlex`-style
    /// scanning rather than a full shell grammar — adequate for hoosh's threat
    /// model. If structural correctness is ever needed, replace this with a real
    /// AST parser (the `brush-parser` crate is the candidate).
    pub fn split_subcommands(input: &str) -> Vec<String> {
        let input_to_parse = if Self::contains_heredoc(input) {
            input.lines().next().unwrap_or("").to_string()
        } else {
            input.to_string()
        };

        let chars: Vec<char> = input_to_parse.chars().collect();
        let mut segments = Vec::new();
        let mut current = String::new();
        let mut i = 0;
        let mut in_single = false;
        let mut in_double = false;
        let mut in_backtick = false;
        let mut depth: i32 = 0;

        while i < chars.len() {
            let c = chars[i];

            if c == '\\' && !in_single {
                current.push(c);
                if i + 1 < chars.len() {
                    current.push(chars[i + 1]);
                    i += 2;
                } else {
                    i += 1;
                }
                continue;
            }

            if in_single {
                current.push(c);
                in_single = c != '\'';
                i += 1;
                continue;
            }
            if in_double {
                current.push(c);
                in_double = c != '"';
                i += 1;
                continue;
            }

            match c {
                '\'' => {
                    in_single = true;
                    current.push(c);
                    i += 1;
                }
                '"' => {
                    in_double = true;
                    current.push(c);
                    i += 1;
                }
                '`' => {
                    in_backtick = !in_backtick;
                    current.push(c);
                    i += 1;
                }
                _ if in_backtick => {
                    current.push(c);
                    i += 1;
                }
                '$' if i + 1 < chars.len() && chars[i + 1] == '(' => {
                    depth += 1;
                    current.push('$');
                    current.push('(');
                    i += 2;
                }
                '(' => {
                    depth += 1;
                    current.push(c);
                    i += 1;
                }
                ')' => {
                    depth = (depth - 1).max(0);
                    current.push(c);
                    i += 1;
                }
                _ if depth > 0 => {
                    current.push(c);
                    i += 1;
                }
                ';' => {
                    Self::push_segment(&mut segments, &mut current);
                    i += 1;
                }
                '|' => {
                    Self::push_segment(&mut segments, &mut current);
                    i += if i + 1 < chars.len() && chars[i + 1] == '|' {
                        2
                    } else {
                        1
                    };
                }
                '&' if i + 1 < chars.len() && chars[i + 1] == '&' => {
                    Self::push_segment(&mut segments, &mut current);
                    i += 2;
                }
                _ => {
                    current.push(c);
                    i += 1;
                }
            }
        }
        Self::push_segment(&mut segments, &mut current);
        segments
    }

    fn push_segment(segments: &mut Vec<String>, current: &mut String) {
        let trimmed = current.trim();
        if !trimmed.is_empty() {
            segments.push(trimmed.to_string());
        }
        current.clear();
    }

    /// First real command token of a single subcommand: skips `VAR=val`
    /// env-var prefixes. Returns `None` on an empty/unparseable segment.
    fn first_command_token(segment: &str) -> Option<String> {
        let tokens = shlex::split(segment)?;
        tokens
            .into_iter()
            .find(|t| !t.contains('=') || t.starts_with('-'))
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

    /// Regression: shlex tokenizes `a|grep` as one token, so the old parser
    /// missed everything after a space-less operator. The splitter must see it.
    #[test]
    fn extract_base_commands_handles_no_space_operators() {
        assert_eq!(
            BashCommandParser::extract_base_commands("cat a|grep b"),
            vec!["cat", "grep"]
        );
        assert_eq!(
            BashCommandParser::extract_base_commands("cargo build&&cargo test"),
            vec!["cargo"]
        );
        assert_eq!(
            BashCommandParser::extract_base_commands("ls;pwd;echo hi"),
            vec!["ls", "pwd", "echo"]
        );
    }

    #[test]
    fn split_subcommands_spaced_and_unspaced() {
        assert_eq!(
            BashCommandParser::split_subcommands("a && b"),
            vec!["a", "b"]
        );
        assert_eq!(BashCommandParser::split_subcommands("a&&b"), vec!["a", "b"]);
        assert_eq!(BashCommandParser::split_subcommands("a|b"), vec!["a", "b"]);
        assert_eq!(BashCommandParser::split_subcommands("a;b"), vec!["a", "b"]);
    }

    #[test]
    fn split_subcommands_or_does_not_create_empty_segment() {
        assert_eq!(
            BashCommandParser::split_subcommands("ls || echo failed"),
            vec!["ls", "echo failed"]
        );
    }

    #[test]
    fn split_subcommands_respects_quotes() {
        assert_eq!(
            BashCommandParser::split_subcommands("echo \"a && b\""),
            vec!["echo \"a && b\""]
        );
        assert_eq!(
            BashCommandParser::split_subcommands("git commit -m 'x | y'"),
            vec!["git commit -m 'x | y'"]
        );
    }

    #[test]
    fn split_subcommands_respects_subshell_and_backtick_nesting() {
        assert_eq!(
            BashCommandParser::split_subcommands("echo $(a | b)"),
            vec!["echo $(a | b)"]
        );
        assert_eq!(
            BashCommandParser::split_subcommands("echo `a | b`"),
            vec!["echo `a | b`"]
        );
    }

    #[test]
    fn split_subcommands_keeps_redirection_in_segment() {
        assert_eq!(
            BashCommandParser::split_subcommands("npx test 2>&1 | grep y"),
            vec!["npx test 2>&1", "grep y"]
        );
    }

    #[test]
    fn split_subcommands_screenshot_case() {
        assert_eq!(
            BashCommandParser::split_subcommands(
                "cd apps/web && npx playwright test x 2>&1 | grep -A 25 \"image uploads\""
            ),
            vec![
                "cd apps/web",
                "npx playwright test x 2>&1",
                "grep -A 25 \"image uploads\""
            ]
        );
    }

    #[test]
    fn split_subcommands_trims_and_drops_empty() {
        assert_eq!(
            BashCommandParser::split_subcommands("  ls  ;  "),
            vec!["ls"]
        );
        assert!(BashCommandParser::split_subcommands("   ").is_empty());
    }

    #[test]
    fn split_subcommands_ignores_heredoc_body() {
        // Only the command line is split; heredoc body is left out.
        assert_eq!(
            BashCommandParser::split_subcommands("cat <<EOF\nfoo | bar\nEOF"),
            vec!["cat <<EOF"]
        );
    }
}
