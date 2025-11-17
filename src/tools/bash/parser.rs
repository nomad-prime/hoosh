/// Extracts base commands from bash input for permission checking
pub struct BashCommandParser;

impl BashCommandParser {
    /// Extract base commands from a bash string
    /// Returns a list of unique base command names
    ///
    /// Examples:
    /// - "cargo build" -> ["cargo"]
    /// - "cargo build && cargo test" -> ["cargo"]
    /// - "cat file | grep error | wc -l" -> ["cat", "grep", "wc"]
    /// - "ls -la; pwd; echo done" -> ["ls", "pwd", "echo"]
    pub fn extract_base_commands(input: &str) -> Vec<String> {
        let mut commands = Vec::new();

        // Check if this is a heredoc - if so, only parse the first line
        let lines_to_parse: Vec<&str> = if Self::contains_heredoc(input) {
            // For heredocs, only parse the first line (the actual command)
            input.lines().take(1).collect()
        } else {
            input.lines().collect()
        };

        for statement in lines_to_parse.iter().flat_map(|line| line.split(';')) {
            let trimmed = statement.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            for segment in Self::split_on_operators(trimmed) {
                for cmd in segment.split('|') {
                    if let Some(base) = Self::extract_single_base_command(cmd.trim())
                        && !commands.contains(&base)
                    {
                        commands.push(base);
                    }
                }
            }
        }

        commands
    }

    /// Check if input contains a heredoc
    fn contains_heredoc(input: &str) -> bool {
        input.contains("<<") || input.contains("<<<")
    }

    /// Generate a permission pattern suggestion from base commands
    ///
    /// Examples:
    /// - ["cargo"] -> "cargo:*"
    /// - ["cat", "grep", "wc"] -> "cat:*|grep:*|wc:*"
    /// - ["npm"] -> "npm:*"
    pub fn suggest_pattern(base_commands: &[String]) -> String {
        match base_commands.len() {
            0 => "*".to_string(),
            1 => format!("{}:*", base_commands[0]),
            _ => {
                if base_commands.iter().all(|c| c == &base_commands[0]) {
                    format!("{}:*", base_commands[0])
                } else {
                    base_commands
                        .iter()
                        .map(|cmd| format!("{}:*", cmd))
                        .collect::<Vec<_>>()
                        .join("|")
                }
            }
        }
    }

    /// Extract base command from a single command string
    /// "cargo build --release" -> Some("cargo")
    /// "  ls -la  " -> Some("ls")
    fn extract_single_base_command(cmd: &str) -> Option<String> {
        cmd.split_whitespace()
            .next()
            .filter(|s| !s.is_empty())
            .map(String::from)
    }

    /// Split on && and || operators
    fn split_on_operators(input: &str) -> Vec<&str> {
        let mut segments = Vec::new();
        let mut current = input;

        loop {
            if let Some(pos) = current.find("&&") {
                segments.push(&current[..pos]);
                current = &current[pos + 2..];
            } else if let Some(pos) = current.find("||") {
                segments.push(&current[..pos]);
                current = &current[pos + 2..];
            } else {
                segments.push(current);
                break;
            }
        }

        segments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_command() {
        let cmds = BashCommandParser::extract_base_commands("cargo build");
        assert_eq!(cmds, vec!["cargo"]);
    }

    #[test]
    fn test_extract_command_with_args() {
        let cmds = BashCommandParser::extract_base_commands("cargo build --release");
        assert_eq!(cmds, vec!["cargo"]);
    }

    #[test]
    fn test_extract_command_chain() {
        let cmds = BashCommandParser::extract_base_commands("cargo build && cargo test");
        assert_eq!(cmds, vec!["cargo"]);
    }

    #[test]
    fn test_extract_pipeline() {
        let cmds = BashCommandParser::extract_base_commands("cat file.txt | grep error | wc -l");
        assert_eq!(cmds, vec!["cat", "grep", "wc"]);
    }

    #[test]
    fn test_extract_semicolon_separator() {
        let cmds = BashCommandParser::extract_base_commands("ls -la; pwd; echo done");
        assert_eq!(cmds, vec!["ls", "pwd", "echo"]);
    }

    #[test]
    fn test_extract_mixed_complex() {
        let cmds = BashCommandParser::extract_base_commands(
            "cargo build && cat Cargo.toml | grep version",
        );
        assert_eq!(cmds, vec!["cargo", "cat", "grep"]);
    }

    #[test]
    fn test_extract_with_comments() {
        let cmds = BashCommandParser::extract_base_commands("# comment\ncargo build");
        assert_eq!(cmds, vec!["cargo"]);
    }

    #[test]
    fn test_extract_multiline() {
        let cmds = BashCommandParser::extract_base_commands("cargo build\ncargo test");
        assert_eq!(cmds, vec!["cargo"]);
    }

    #[test]
    fn test_suggest_pattern_single_command() {
        let pattern = BashCommandParser::suggest_pattern(&["cargo".to_string()]);
        assert_eq!(pattern, "cargo:*");
    }

    #[test]
    fn test_suggest_pattern_multiple_same() {
        let pattern =
            BashCommandParser::suggest_pattern(&["cargo".to_string(), "cargo".to_string()]);
        assert_eq!(pattern, "cargo:*");
    }

    #[test]
    fn test_suggest_pattern_multiple_different() {
        let pattern = BashCommandParser::suggest_pattern(&[
            "cat".to_string(),
            "grep".to_string(),
            "wc".to_string(),
        ]);
        assert_eq!(pattern, "cat:*|grep:*|wc:*");
    }

    #[test]
    fn test_suggest_pattern_empty() {
        let pattern = BashCommandParser::suggest_pattern(&[]);
        assert_eq!(pattern, "*");
    }

    #[test]
    fn test_extract_single_base_command() {
        assert_eq!(
            BashCommandParser::extract_single_base_command("cargo build --release"),
            Some("cargo".to_string())
        );
        assert_eq!(
            BashCommandParser::extract_single_base_command("  ls -la  "),
            Some("ls".to_string())
        );
        assert_eq!(BashCommandParser::extract_single_base_command(""), None);
        assert_eq!(BashCommandParser::extract_single_base_command("   "), None);
    }

    #[test]
    fn test_heredoc_only_parses_first_line() {
        let input = "cat <<EOF\nHello from a heredoc!\nThis is a multi-line string.\nYou can use any content here.\nEOF";
        let cmds = BashCommandParser::extract_base_commands(input);
        assert_eq!(cmds, vec!["cat"]);
    }

    #[test]
    fn test_heredoc_with_quotes() {
        let input = "cat <<'EOF'\nHello\nWorld\nEOF";
        let cmds = BashCommandParser::extract_base_commands(input);
        assert_eq!(cmds, vec!["cat"]);
    }

    #[test]
    fn test_herestring() {
        let input = "cat <<< \"some text\"";
        let cmds = BashCommandParser::extract_base_commands(input);
        assert_eq!(cmds, vec!["cat"]);
    }
}
