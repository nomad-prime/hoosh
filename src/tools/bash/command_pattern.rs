use super::BashCommandParser;

#[derive(Debug, Clone, PartialEq)]
pub struct CommandPatternResult {
    pub description: String,
    /// One-line, plain-English gloss of what the command does, shown in the
    /// permission dialog so the user understands the request at a glance.
    pub summary: String,
    pub pattern: String,
    pub persistent_message: String,
    pub safe: bool,
    /// When false, the permission dialog must NOT offer the "trust project"
    /// option for this command — typically because the pattern is too dynamic
    /// for blanket trust (e.g. arbitrary subshells).
    pub allow_project_wide_trust: bool,
}

impl Default for CommandPatternResult {
    fn default() -> Self {
        Self {
            description: String::new(),
            summary: String::new(),
            pattern: "*".to_string(),
            persistent_message: String::new(),
            safe: false,
            allow_project_wide_trust: true,
        }
    }
}

/// Render a list of base command names as a friendly inline list:
/// `["git"]` -> "git", `["npm", "cp"]` -> "npm and cp",
/// `["a", "b", "c"]` -> "a, b and c".
fn join_commands(commands: &[String]) -> String {
    match commands {
        [] => "a command".to_string(),
        [one] => one.clone(),
        [head @ .., last] => format!("{} and {}", head.join(", "), last),
    }
}

/// Quote each item, then join into a friendly inline list:
/// `["npx playwright", "docker compose"]` -> `"npx playwright" and "docker compose"`.
fn quoted_join(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|i| format!("\"{i}\"")).collect();
    join_commands(&quoted)
}

/// The commands in a compound pipeline/chain that actually warrant a rule —
/// the read-only/whitelisted subcommands and `cd` are dropped as noise, so a
/// rule keys on the meaningful work (e.g. `npx playwright` out of
/// `cd web && npx playwright test … | grep …`).
///
/// Meaningful commands are returned as 2-word prefixes (`cmd subcmd`) so rules
/// stay reusably scoped. If *every* subcommand is read-only/`cd`, there is no
/// noise to strip and we fall back to the bare base-command names.
fn meaningful_prefixes(command: &str) -> Vec<String> {
    let mut meaningful: Vec<String> = Vec::new();
    let mut bare_all: Vec<String> = Vec::new();

    for sub in BashCommandParser::split_subcommands(command) {
        let Some((cmd, arg)) = BashCommandParser::extract_first_command_and_arg(&sub) else {
            continue;
        };
        if !bare_all.contains(&cmd) {
            bare_all.push(cmd.clone());
        }
        if cmd == "cd" || SingleCommandPattern::is_whitelisted(&cmd, &sub) {
            continue;
        }
        let prefix = match arg {
            Some(a) => format!("{cmd} {a}"),
            None => cmd.clone(),
        };
        if !meaningful.contains(&prefix) {
            meaningful.push(prefix);
        }
    }

    if meaningful.is_empty() {
        bare_all
    } else {
        meaningful
    }
}

/// Shared matcher for pipeline/chain rules. A stored rule trusts a set of
/// command prefixes; a command matches only when *every* meaningful command it
/// runs is covered by that set (subset-of-trust, the safe direction). A stored
/// 2-word prefix (`npx playwright`) matches that exact prefix; a stored bare
/// command (`cargo`) matches any of its subcommands.
fn compound_matches(pattern: &str, command: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let stored: Vec<String> = pattern
        .split(['|', '&'])
        .map(|p| {
            p.trim()
                .trim_end_matches(":*")
                .trim_end_matches('*')
                .trim()
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect();
    if stored.is_empty() {
        return false;
    }

    let incoming = meaningful_prefixes(command);
    if incoming.is_empty() {
        return false;
    }

    incoming.iter().all(|prefix| {
        let base = prefix.split(' ').next().unwrap_or(prefix);
        stored.iter().any(|s| s == prefix || s == base)
    })
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
            summary: "Runs a command that executes another command inline (subshell)".to_string(),
            // A subshell rule used to be stored as "*" — that silently
            // matched every future bash command and granted blanket trust.
            // Keep the pattern stable but mark trust-project as disallowed
            // so no rule ever gets persisted from this prompt.
            pattern: "subshell:none".to_string(),
            persistent_message:
                "subshells are evaluated per-call and cannot be pre-approved project-wide"
                    .to_string(),
            safe: false, // NEVER safe to auto-approve subshells
            allow_project_wide_trust: false,
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
            summary: format!("Runs `{}` and redirects its input/output to a file", cmd),
            pattern: format!("{}:>", cmd),
            persistent_message: format!(
                "don't ask me again for \"{}\" commands with redirection (>, <) in this project",
                cmd
            ),
            safe: false,
            allow_project_wide_trust: true,
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
            summary: format!("Feeds an inline document (heredoc) into `{}`", cmd),
            pattern: format!("{}:<<", cmd),
            persistent_message: format!(
                "don't ask me again for \"{}\" commands with heredoc (<<) in this project",
                cmd
            ),
            safe: false,
            allow_project_wide_trust: true,
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
        compound_matches(pattern, command)
    }

    fn analyze(&self, command: &str) -> CommandPatternResult {
        let base_commands = BashCommandParser::extract_base_commands(command);

        if base_commands.is_empty() {
            return CommandPatternResult {
                description: "pipeline".to_string(),
                summary: "Pipes output between commands".to_string(),
                pattern: "*".to_string(),
                persistent_message: "don't ask me again for bash in this project".to_string(),
                safe: false,
                allow_project_wide_trust: true,
            };
        }

        if base_commands.iter().all(|c| c == &base_commands[0]) {
            return CommandPatternResult {
                description: base_commands[0].clone(),
                summary: format!("Pipes data through `{}`", base_commands[0]),
                pattern: format!("{}:*", base_commands[0]),
                persistent_message: format!(
                    "don't ask me again for \"{}\" commands in this project",
                    base_commands[0]
                ),
                safe: false,
                allow_project_wide_trust: true,
            };
        }

        let prefixes = meaningful_prefixes(command);
        if prefixes.len() == 1 {
            let p = &prefixes[0];
            CommandPatternResult {
                description: p.clone(),
                summary: format!("Runs `{p}` and pipes its output"),
                pattern: format!("{p}:*"),
                persistent_message: format!(
                    "don't ask me again for \"{p}\" commands in this project"
                ),
                safe: false,
                allow_project_wide_trust: true,
            }
        } else {
            let pattern = prefixes
                .iter()
                .map(|p| format!("{p}:*"))
                .collect::<Vec<_>>()
                .join("|");
            CommandPatternResult {
                description: prefixes.join(", "),
                summary: format!("Pipes output through {}", join_commands(&prefixes)),
                pattern,
                persistent_message: format!(
                    "don't ask me again for {} commands in this project",
                    quoted_join(&prefixes)
                ),
                safe: false,
                allow_project_wide_trust: true,
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
        compound_matches(pattern, command)
    }

    fn analyze(&self, command: &str) -> CommandPatternResult {
        let base_commands = BashCommandParser::extract_base_commands(command);

        if base_commands.is_empty() {
            return CommandPatternResult {
                description: "command chain".to_string(),
                summary: "Runs several commands in sequence".to_string(),
                pattern: "*".to_string(),
                persistent_message: "don't ask me again for bash in this project".to_string(),
                safe: false,
                allow_project_wide_trust: true,
            };
        }

        if base_commands.iter().all(|c| c == &base_commands[0]) {
            return CommandPatternResult {
                description: base_commands[0].clone(),
                summary: format!("Runs several `{}` commands in sequence", base_commands[0]),
                pattern: format!("{}:*", base_commands[0]),
                persistent_message: format!(
                    "don't ask me again for \"{}\" commands in this project",
                    base_commands[0]
                ),
                safe: false,
                allow_project_wide_trust: true,
            };
        }

        let prefixes = meaningful_prefixes(command);
        if prefixes.len() == 1 {
            let p = &prefixes[0];
            CommandPatternResult {
                description: p.clone(),
                summary: format!("Runs `{p}`"),
                pattern: format!("{p}:*"),
                persistent_message: format!(
                    "don't ask me again for \"{p}\" commands in this project"
                ),
                safe: false,
                allow_project_wide_trust: true,
            }
        } else {
            let pattern = prefixes
                .iter()
                .map(|p| format!("{p}:*"))
                .collect::<Vec<_>>()
                .join("&");
            CommandPatternResult {
                description: prefixes.join(", "),
                summary: format!("Runs {} in sequence", join_commands(&prefixes)),
                pattern,
                persistent_message: format!(
                    "don't ask me again for {} commands in this project",
                    quoted_join(&prefixes)
                ),
                safe: false,
                allow_project_wide_trust: true,
            }
        }
    }

    fn priority(&self) -> u32 {
        60
    }
}

/// Read-only network commands: `curl <URL>`, `wget <URL>`, `gh api <...>` and
/// the like — anything that fetches but does not write or post.
///
/// One persistent approval ("trust project") creates a stable `net:read` rule
/// that covers all future read-only network calls in this project — so web
/// fetches don't get permission-prompt friction.
pub struct NetworkReadPattern;

impl NetworkReadPattern {
    const NETWORK_COMMANDS: &'static [&'static str] = &["curl", "wget", "gh"];

    /// Flags whose presence (with any value, or with a non-GET method) means
    /// the call mutates state. Matched by exact token OR by `flag=...` prefix.
    const MUTATING_FLAGS_TAKING_VALUE: &'static [&'static str] = &[
        "-d",
        "--data",
        "--data-raw",
        "--data-binary",
        "--data-urlencode",
        "--form",
        "-F",
        "--upload-file",
        "-T",
        "--post-data",
        "--post-file",
    ];

    /// Method flags: only allow when the value is GET.
    const METHOD_FLAGS: &'static [&'static str] = &["-X", "--request", "--method"];

    fn token_starts_with_flag(token: &str, flag: &str) -> bool {
        // Matches "--flag=value" form. Exact `--flag` form handled separately.
        token.starts_with(&format!("{flag}="))
    }

    fn is_read_only_network_command(command: &str) -> bool {
        // Pipelines and chains are handled by their own patterns; this one is
        // for a single read-only network call.
        if command.contains('|') || command.contains("&&") || command.contains("||") {
            return false;
        }
        if BashCommandParser::contains_subshell(command) {
            return false;
        }
        // Redirects mean we're writing fetched content to a file — let
        // RedirectionPattern require explicit consent for the write target.
        if command.contains('>') {
            return false;
        }

        let Some(first) = BashCommandParser::extract_base_commands(command)
            .into_iter()
            .next()
        else {
            return false;
        };
        if !Self::NETWORK_COMMANDS.contains(&first.as_str()) {
            return false;
        }

        let tokens = match shlex::split(command) {
            Some(t) => t,
            None => return false,
        };

        let mut iter = tokens.iter().peekable();
        while let Some(token) = iter.next() {
            // Mutating flags with required values.
            if Self::MUTATING_FLAGS_TAKING_VALUE.contains(&token.as_str()) {
                return false;
            }
            for flag in Self::MUTATING_FLAGS_TAKING_VALUE {
                if Self::token_starts_with_flag(token, flag) {
                    return false;
                }
            }

            // Method flags: need to inspect value (separate token or =value).
            for mflag in Self::METHOD_FLAGS {
                if token == mflag {
                    // value is the next token
                    let method = iter.peek().map(|s| s.as_str()).unwrap_or("");
                    if !method.eq_ignore_ascii_case("get") {
                        return false;
                    }
                } else if let Some(rest) = token.strip_prefix(&format!("{mflag}="))
                    && !rest.eq_ignore_ascii_case("get")
                {
                    return false;
                }
            }
            // `-XPOST` (no space) form.
            if let Some(rest) = token.strip_prefix("-X")
                && !rest.is_empty()
                && !rest.eq_ignore_ascii_case("get")
            {
                return false;
            }
        }

        // `gh` is broad: only `gh api ...` is in scope. Other subcommands
        // (`gh pr create`, `gh issue close`, etc.) are state-changing.
        if first == "gh" {
            let after_gh = tokens.iter().skip_while(|t| *t != "gh").nth(1);
            if after_gh.map(String::as_str) != Some("api") {
                return false;
            }
        }

        true
    }
}

impl BashCommandPattern for NetworkReadPattern {
    fn matches(&self, command: &str) -> bool {
        Self::is_read_only_network_command(command)
    }

    fn matches_pattern(&self, pattern: &str, command: &str) -> bool {
        pattern == "net:read" && Self::is_read_only_network_command(command)
    }

    fn analyze(&self, _command: &str) -> CommandPatternResult {
        CommandPatternResult {
            description: "read-only network request".to_string(),
            summary: "Makes a read-only network request (no data is uploaded)".to_string(),
            pattern: "net:read".to_string(),
            persistent_message:
                "don't ask me again for read-only network requests (curl/wget/gh api) in this project"
                    .to_string(),
            safe: false,
            allow_project_wide_trust: true,
        }
    }

    fn priority(&self) -> u32 {
        // Above PipelinePattern(80) is tempting but a pipeline like
        // `curl URL | jq` should be evaluated as a pipeline so all its parts
        // are considered. Keep below Pipeline(80) and below Redirection(70)
        // so file writes still take precedence. Sits above CommandChain(60).
        65
    }
}

pub struct SingleCommandPattern;

impl SingleCommandPattern {
    fn is_whitelisted(cmd: &str, full_command: &str) -> bool {
        match cmd {
            // Always safe (information only)
            "ls" | "pwd" | "whoami" | "date" | "echo" | "which" | "type" | "hostname" => {
                !full_command.contains('>')
            }

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
        // Single command patterns should NOT match complex commands
        // Reject if command contains redirections, pipes, chains, or subshells
        if command.contains('>') || command.contains('<') {
            return false;
        }
        if command.contains('|') {
            return false;
        }
        if command.contains("&&") || command.contains("||") || command.contains(';') {
            return false;
        }
        if BashCommandParser::contains_subshell(command) {
            return false;
        }

        if let Some(prefix) = pattern.strip_suffix(":*") {
            // Match on parsed tokens, not the raw string, so a pattern stays
            // symmetric with the way it was generated. This is what makes
            // env-var-prefixed commands re-match: `RUST_LOG=debug cargo run`
            // tokenizes to (`cargo`, `run`) just like `cargo run` does, so the
            // stored `cargo run:*` rule keeps matching it.
            let Some((cmd, arg)) = BashCommandParser::extract_first_command_and_arg(command) else {
                return false;
            };
            // A bare-command prefix (`cargo:*`) matches any invocation of that
            // command; a command+arg prefix (`cargo run:*`) matches only when
            // the first argument lines up too.
            if prefix == cmd {
                return true;
            }
            match arg {
                Some(arg) => prefix == format!("{} {}", cmd, arg),
                None => false,
            }
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

            let summary = if safe {
                format!("Runs `{}` (read-only)", description)
            } else {
                format!("Runs `{}`", description)
            };

            CommandPatternResult {
                description: description.clone(),
                summary,
                pattern,
                persistent_message: format!(
                    "don't ask me again for \"{}\" commands in this project",
                    description
                ),
                safe,
                allow_project_wide_trust: true,
            }
        } else {
            // Fallback
            CommandPatternResult {
                description: "bash command".to_string(),
                summary: "Runs a bash command".to_string(),
                pattern: "*".to_string(),
                persistent_message: "don't ask me again for bash".to_string(),
                safe: false,
                allow_project_wide_trust: true,
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
        // All read-only: nothing to strip, so the rule keys on bare names with
        // clean wording (no "pipe combination of ...").
        let result = pattern.analyze("cat file | grep error | wc -l");
        assert_eq!(result.pattern, "cat:*|grep:*|wc:*");
        assert!(!result.persistent_message.contains("pipe combination"));
        assert!(result.persistent_message.contains("\"cat\""));
    }

    /// The motivating case: read-only/`cd` noise is stripped so the rule keys on
    /// the one meaningful command, with clean single-command wording.
    #[test]
    fn test_pipeline_strips_noise_to_single_meaningful_command() {
        let pattern = PipelinePattern;
        let result =
            pattern.analyze("cd apps/web && npx playwright test x 2>&1 | grep -A 25 \"image\"");
        assert_eq!(result.pattern, "npx playwright:*");
        assert!(!result.safe);
        assert!(result.persistent_message.contains("npx playwright"));
        assert!(!result.persistent_message.contains("pipe combination"));
        assert!(!result.persistent_message.contains("grep"));
        // And the generated rule re-matches the command it came from.
        assert!(pattern.matches_pattern(
            &result.pattern,
            "cd apps/web && npx playwright test x 2>&1 | grep -A 25 \"image\""
        ));
    }

    #[test]
    fn test_compound_keys_on_meaningful_command() {
        // read-only prefix dropped
        assert_eq!(
            CommandChainPattern.analyze("ls && cargo build").pattern,
            "cargo build:*"
        );
        // cd prefix dropped
        assert_eq!(
            CommandChainPattern.analyze("cd foo && npm test").pattern,
            "npm test:*"
        );
    }

    #[test]
    fn test_compound_patterns_round_trip_through_matcher() {
        for (pat, cmd) in [
            (
                &PipelinePattern as &dyn BashCommandPattern,
                "cd web && npx playwright test x | grep y",
            ),
            (&CommandChainPattern, "ls && cargo build --release"),
            (&CommandChainPattern, "cd foo && npm test -- --watch"),
            (&PipelinePattern, "cat a | grep b | wc -l"),
            (&CommandChainPattern, "cargo build && docker compose up"),
        ] {
            let result = pat.analyze(cmd);
            assert!(
                pat.matches_pattern(&result.pattern, cmd),
                "generated pattern `{}` failed to re-match `{cmd}`",
                result.pattern
            );
        }
    }

    #[test]
    fn test_pipeline_two_meaningful_commands_clean_wording() {
        let pattern = CommandChainPattern;
        let result = pattern.analyze("cargo build && docker compose up");
        assert_eq!(result.pattern, "cargo build:*&docker compose:*");
        assert!(result.persistent_message.contains("\"cargo build\""));
        assert!(result.persistent_message.contains("\"docker compose\""));
        assert!(!result.persistent_message.contains("combination"));
        assert!(pattern.matches_pattern(&result.pattern, "cargo build && docker compose up"));
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
    fn test_pipeline_matches_pattern_subset_is_allowed() {
        let pattern = PipelinePattern;
        // A command using fewer commands than the rule trusts is within trust.
        assert!(pattern.matches_pattern("cat:*|grep:*|wc:*", "cat file | wc -l"));
        assert!(pattern.matches_pattern("cat:*|grep:*", "cat file | grep x"));
    }

    #[test]
    fn test_pipeline_matches_pattern_uncovered_command_rejected() {
        let pattern = PipelinePattern;
        // `rm` is meaningful and not in the trusted set → must NOT match.
        assert!(!pattern.matches_pattern("cat:*|grep:*", "cat file | grep x | rm -rf y"));
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

    /// Regression: an env-var prefix used to break re-matching. The pattern is
    /// generated from the real command (`cargo run`), so the stored rule must
    /// keep matching the same invocation even with a `VAR=val` prefix —
    /// otherwise the identical command prompts again every time.
    #[test]
    fn test_single_command_matches_pattern_ignores_env_prefix() {
        let pattern = SingleCommandPattern;
        assert!(pattern.matches_pattern("cargo run:*", "RUST_LOG=debug cargo run"));
        assert!(pattern.matches_pattern("cargo:*", "RUST_LOG=debug cargo run"));
        assert!(pattern.matches_pattern("make:*", "FOO=bar make build"));
        // A different command behind the env prefix must still NOT match.
        assert!(!pattern.matches_pattern("cargo run:*", "RUST_LOG=debug cargo test"));
    }

    /// The descriptor a command produces must re-match the command it came
    /// from — a stored "trust project" rule is worthless if it can't.
    #[test]
    fn test_analyze_pattern_round_trips_through_matcher() {
        let pattern = SingleCommandPattern;
        for cmd in [
            "cargo build",
            "RUST_LOG=debug cargo run",
            "git status",
            "ls -la",
        ] {
            let result = pattern.analyze(cmd);
            assert!(
                pattern.matches_pattern(&result.pattern, cmd),
                "generated pattern `{}` failed to re-match `{cmd}`",
                result.pattern
            );
        }
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
        // Subset of the trusted set is fine; an uncovered meaningful command is not.
        assert!(registry.matches_pattern("cat:*|grep:*|wc:*", "cat file | grep pattern"));
        assert!(!registry.matches_pattern("cat:*|grep:*", "cat file | grep p | rm x"));
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
