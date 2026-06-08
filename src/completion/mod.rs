mod command_completer;
mod file_completer;

pub use command_completer::CommandCompleter;
pub use file_completer::FileCompleter;

use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Completer: Send + Sync {
    fn trigger_key(&self) -> char;
    async fn get_completions(&self, query: &str) -> Result<Vec<String>>;

    /// Whether the trigger key should activate this completer given the
    /// text already in the input buffer (before the trigger key is inserted).
    /// Defaults to always-on; completers like slash-commands override this
    /// to only fire at the start of the prompt.
    fn should_trigger(&self, _input_before_trigger: &str) -> bool {
        true
    }

    fn format_completion(&self, item: &str) -> String {
        item.to_string()
    }

    /// Find the position of the trigger character in the input text
    /// Returns the byte position of the trigger character, or None if not found
    fn find_trigger_position(&self, input: &str) -> Option<usize> {
        input.rfind(self.trigger_key())
    }

    /// Apply the selected completion to the input text
    /// Returns the new input text with the completion applied
    fn apply_completion(&self, input: &str, trigger_pos: usize, completion: &str) -> String {
        format!("{}{}", &input[..=trigger_pos], completion)
    }
}
