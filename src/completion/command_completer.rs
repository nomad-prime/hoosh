use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use super::Completer;
use crate::commands::CommandRegistry;

pub struct CommandCompleter {
    registry: Arc<CommandRegistry>,
}

impl CommandCompleter {
    pub fn new(registry: Arc<CommandRegistry>) -> Self {
        Self { registry }
    }

    fn fuzzy_match(pattern: &str, target: &str) -> bool {
        if pattern.is_empty() {
            return true;
        }

        let pattern_lower = pattern.to_lowercase();
        let target_lower = target.to_lowercase();

        let mut pattern_chars = pattern_lower.chars();
        let mut current_pattern_char = pattern_chars.next();

        for target_char in target_lower.chars() {
            if let Some(pc) = current_pattern_char
                && pc == target_char
            {
                current_pattern_char = pattern_chars.next();
            }
        }

        current_pattern_char.is_none()
    }

    fn score_match(pattern: &str, target: &str) -> i32 {
        let pattern_lower = pattern.to_lowercase();
        let target_lower = target.to_lowercase();

        let mut score = 0;

        // Exact match gets highest score
        if target_lower == pattern_lower {
            return 1000;
        }

        // Prefix match gets high score
        if target_lower.starts_with(&pattern_lower) {
            score += 500;
            // Bonus for matching more of the target word
            let ratio = (pattern_lower.len() as f32 / target_lower.len() as f32 * 100.0) as i32;
            score += ratio;
        } else if target_lower.contains(&pattern_lower) {
            // Contains match gets lower score
            score += 300;
        } else if Self::fuzzy_match(pattern, target) {
            // Fuzzy match gets lowest score
            score += 100;
        }

        // Penalize longer targets (prefer shorter command names)
        score -= target.len() as i32;

        score
    }
}

#[async_trait]
impl Completer for CommandCompleter {
    fn trigger_key(&self) -> char {
        '/'
    }

    async fn get_completions(&self, query: &str) -> Result<Vec<String>> {
        let commands = self.registry.list_commands();

        let mut matches: Vec<(String, i32)> = commands
            .iter()
            .filter_map(|(name, desc)| {
                if Self::fuzzy_match(query, name) {
                    let score = Self::score_match(query, name);
                    Some((format!("{} - {}", name, desc), score))
                } else {
                    None
                }
            })
            .collect();

        matches.sort_by(|a, b| b.1.cmp(&a.1));

        Ok(matches.into_iter().map(|(text, _)| text).collect())
    }

    fn format_completion(&self, item: &str) -> String {
        item.to_string()
    }

    fn apply_completion(&self, input: &str, trigger_pos: usize, completion: &str) -> String {
        // Extract just the command name before " - " from the completion
        let command_name = completion.split(" - ").next().unwrap_or(completion);
        format!("{}{}", &input[..=trigger_pos], command_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_match() {
        assert!(CommandCompleter::fuzzy_match("hlp", "help"));
        assert!(CommandCompleter::fuzzy_match("clr", "clear"));
        assert!(CommandCompleter::fuzzy_match("", "anything"));
        assert!(!CommandCompleter::fuzzy_match("xyz", "abc"));
    }

    #[test]
    fn test_score_match() {
        // Exact match should score higher than prefix match
        assert!(
            CommandCompleter::score_match("help", "help")
                > CommandCompleter::score_match("help", "helper")
        );

        // Prefix match should score higher than fuzzy match
        assert!(
            CommandCompleter::score_match("cl", "clear")
                > CommandCompleter::score_match("cr", "clear")
        );

        // Shorter targets with same prefix score equal or better
        assert!(
            CommandCompleter::score_match("ex", "exit")
                >= CommandCompleter::score_match("ex", "export")
        );
    }
}
