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
            Box::new(RedirectionPattern),
            Box::new(NetworkReadPattern),
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
            allow_project_wide_trust: true,
        }
    }

    pub fn matches_pattern(&self, pattern: &str, command: &str) -> bool {
        for p in &self.patterns {
            if p.matches(command) {
                return p.matches_pattern(pattern, command);
            }
        }
        false
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
    fn test_redirection_with_command_chain() {
        let registry = BashCommandPatternRegistry::new();
        // This command has both redirection AND command chaining
        let cmd = r#"echo "Project: Demo Script" > data.txt && echo "Status: Active" >> data.txt && cat data.txt"#;
        let result = registry.analyze_command(cmd);

        // RedirectionPattern has priority 70, CommandChainPattern has 60
        // So redirection should win
        assert!(!result.safe, "Redirection commands should not be safe");
        assert!(
            result.pattern.contains(":>"),
            "Pattern should be redirection pattern, got: {}",
            result.pattern
        );
    }

    #[test]
    fn test_redirection_with_command_chain_and_single_command_pattern() {
        let registry = BashCommandPatternRegistry::new();
        // This command has both redirection AND command chaining
        let cmd = r#"echo "Project: Demo Script" > data.txt && echo "Status: Active" >> data.txt && cat data.txt"#;
        let result = registry.matches_pattern("echo:*", cmd);

        assert!(!result, "Should only match simple command patterns")
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

    // ------------------------------------------------------------------
    // Reproducers for ISSUES.md — Tier 2.1 permission leak audit.
    // These tests document *current* behaviour. Failing assertions surface
    // real gaps; passing ones show the case is already handled.
    // ------------------------------------------------------------------

    /// ISSUES.md: "Pipe to file should trigger permissions —
    ///   echo \"Hello, this is a test file created at $(date)\" > test_output.txt && c..
    ///   did not"
    /// Subshell + redirect + chain. Should be flagged unsafe.
    #[test]
    fn issues_pipe_to_file_with_subshell_is_unsafe() {
        let registry = BashCommandPatternRegistry::new();
        let result = registry.analyze_command(
            r#"echo "Hello, this is a test file created at $(date)" > test_output.txt && cat test_output.txt"#,
        );
        assert!(
            !result.safe,
            "Subshell-bearing redirect chain must require approval, got safe=true pattern={}",
            result.pattern
        );
    }

    /// Same shape without the subshell. Should still trigger approval because
    /// the file write needs explicit consent.
    #[test]
    fn issues_plain_redirect_to_file_is_unsafe() {
        let registry = BashCommandPatternRegistry::new();
        let result = registry.analyze_command(r#"echo "hello" > test_output.txt"#);
        assert!(
            !result.safe,
            "Plain redirect to a file must require approval, got safe=true pattern={}",
            result.pattern
        );
    }

    /// Append redirect — same expectation.
    #[test]
    fn issues_append_redirect_is_unsafe() {
        let registry = BashCommandPatternRegistry::new();
        let result = registry.analyze_command(r#"echo "more" >> log.txt"#);
        assert!(
            !result.safe,
            "Append redirect must require approval, got safe=true pattern={}",
            result.pattern
        );
    }

    /// `tee` is a write-to-file disguised as a pipeline. Should not be safe.
    #[test]
    fn issues_tee_pipeline_is_unsafe() {
        let registry = BashCommandPatternRegistry::new();
        let result = registry.analyze_command(r#"echo "hello" | tee output.txt"#);
        assert!(
            !result.safe,
            "tee pipeline must require approval, got safe=true pattern={}",
            result.pattern
        );
    }

    /// ISSUES.md: "bash permission heredoc keeps asking".
    /// After approving `cat:<<` project-wide, a later heredoc with different
    /// content should match the same persistent rule.
    #[test]
    fn issues_heredoc_persistent_rule_matches_subsequent_heredocs() {
        use crate::permissions::{BashPatternMatcher, PatternMatcher};
        let matcher = BashPatternMatcher::new();

        let first = "cat <<EOF\nhello\nEOF";
        let second = "cat <<EOF\nworld\nEOF";
        let third = "cat <<MARKER\nfoo\nbar\nMARKER";

        assert!(
            matcher.matches("cat:<<", first),
            "rule should match the heredoc it was created from"
        );
        assert!(
            matcher.matches("cat:<<", second),
            "rule should match another `cat` heredoc with different content"
        );
        assert!(
            matcher.matches("cat:<<", third),
            "rule should match another `cat` heredoc with a different marker"
        );
    }

    /// `cd` detection happens in BashTool (which has cwd context), not in the
    /// registry. The registry still prompts because `cd` isn't whitelisted —
    /// but the user-facing signal "leaves cwd" comes from the tool layer. See
    /// `cd_outside_cwd_overrides_descriptor` below for the integrated test.
    #[test]
    fn registry_alone_does_not_know_cd_target_vs_cwd() {
        let registry = BashCommandPatternRegistry::new();
        let inside = registry.analyze_command("cd /etc");
        let outside = registry.analyze_command("cd /tmp");
        // From the registry's POV both look identical — neither is safe.
        assert!(!inside.safe && !outside.safe);
        assert!(!inside.description.contains("outside"));
    }

    /// Read-only network reads (curl GET, wget, gh api GET) share the
    /// `net:read` class so one persistent approval covers all of them.
    #[test]
    fn network_read_class_covers_curl_wget_gh_api() {
        let registry = BashCommandPatternRegistry::new();
        for cmd in [
            "curl https://example.com",
            "curl -L https://example.com/redirect",
            "curl -s https://example.com -o /tmp/file", // -o is download, still read
            "wget https://example.com/file",
            "wget -O - https://example.com",
            "gh api repos/foo/bar",
            "gh api -X GET repos/foo/bar",
        ] {
            let r = registry.analyze_command(cmd);
            assert_eq!(r.pattern, "net:read", "expected net:read for `{cmd}`");
            assert!(r.description.contains("read-only network"));
        }
    }

    /// Mutating network calls must NOT be classified as net:read.
    #[test]
    fn network_read_excludes_mutating_calls() {
        let registry = BashCommandPatternRegistry::new();
        for cmd in [
            "curl -X POST https://example.com/api",
            "curl --request POST https://example.com",
            "curl -d 'a=1' https://example.com",
            "curl --data-raw '{}' https://example.com",
            "curl --upload-file local.txt https://example.com",
            "curl -F file=@local.txt https://example.com",
            "wget --post-data='a=1' https://example.com",
            "wget --method=DELETE https://example.com",
            "gh api -X POST repos/foo/bar/issues",
            "gh pr create --title hi", // gh non-api subcommands are out of class
        ] {
            let r = registry.analyze_command(cmd);
            assert_ne!(
                r.pattern, "net:read",
                "mutating call should not be net:read: `{cmd}` got pattern={}",
                r.pattern
            );
        }
    }

    /// A persistent `net:read` rule should match other read-only network calls.
    #[test]
    fn network_read_persistent_rule_matches_across_commands() {
        use crate::permissions::{BashPatternMatcher, PatternMatcher};
        let matcher = BashPatternMatcher::new();
        assert!(matcher.matches("net:read", "curl https://example.com"));
        assert!(matcher.matches("net:read", "wget https://example.com/x.tar.gz"));
        assert!(matcher.matches("net:read", "gh api repos/foo/bar"));
        // Mutating call should not be silently auto-approved by the net:read rule.
        assert!(!matcher.matches("net:read", "curl -X POST https://example.com"));
    }

    /// SubshellPattern used to set pattern="*" — clicking trust-project on a
    /// subshell command then stored an `*` rule that matched ALL future bash
    /// commands. The new behaviour: subshells disallow project-wide trust
    /// entirely (so no rule ever gets persisted from that prompt).
    #[test]
    fn subshell_disallows_project_wide_trust() {
        let registry = BashCommandPatternRegistry::new();
        let r = registry.analyze_command("echo $(date)");
        assert!(!r.safe);
        assert!(
            !r.allow_project_wide_trust,
            "subshells must not offer project-wide trust"
        );
        // Pattern should be a stable marker, not the global wildcard.
        assert_ne!(r.pattern, "*");
    }

    #[test]
    fn non_subshell_commands_still_allow_project_wide_trust() {
        let registry = BashCommandPatternRegistry::new();
        for cmd in [
            "cargo build",
            "ls -la",
            "curl https://example.com",
            "cat file.txt",
            "echo hello > out.txt",
        ] {
            let r = registry.analyze_command(cmd);
            assert!(
                r.allow_project_wide_trust,
                "`{cmd}` should still offer project-wide trust"
            );
        }
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
