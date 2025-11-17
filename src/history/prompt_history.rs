use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Manages prompt history for command-line navigation
pub struct PromptHistory {
    entries: Vec<String>,
    current_index: Option<usize>,
    temp_input: Option<String>,
    max_size: usize,
    history_file: Option<PathBuf>,
}

impl PromptHistory {
    /// Creates a new PromptHistory with the specified maximum size
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            current_index: None,
            temp_input: None,
            max_size,
            history_file: None,
        }
    }

    /// Creates a new PromptHistory with persistence enabled
    pub fn with_file<P: AsRef<Path>>(max_size: usize, history_file: P) -> std::io::Result<Self> {
        let history_file = history_file.as_ref().to_path_buf();

        // Create parent directory if it doesn't exist
        if let Some(parent) = history_file.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut history = Self {
            entries: Vec::new(),
            current_index: None,
            temp_input: None,
            max_size,
            history_file: Some(history_file),
        };

        // Load existing history
        history.load()?;

        Ok(history)
    }

    /// Gets the default history file path (~/.hoosh/history)
    pub fn default_history_path() -> Option<PathBuf> {
        dirs::home_dir().map(|home| home.join("../../.hoosh.bak").join("history"))
    }

    /// Loads history from the file
    fn load(&mut self) -> std::io::Result<()> {
        let Some(ref path) = self.history_file else {
            return Ok(());
        };

        if !path.exists() {
            return Ok(());
        }

        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);

        self.entries.clear();
        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                self.entries.push(trimmed.to_string());
            }
        }

        // Trim to max_size if needed
        if self.entries.len() > self.max_size {
            let start = self.entries.len() - self.max_size;
            self.entries = self.entries[start..].to_vec();
        }

        Ok(())
    }

    /// Saves history to the file
    pub fn save(&self) -> std::io::Result<()> {
        let Some(ref path) = self.history_file else {
            return Ok(());
        };

        let mut file = fs::File::create(path)?;
        for entry in &self.entries {
            writeln!(file, "{}", entry)?;
        }

        Ok(())
    }

    /// Adds a new prompt to the history
    /// Skips empty prompts and consecutive duplicates
    pub fn add(&mut self, prompt: String) {
        let trimmed = prompt.trim();
        if trimmed.is_empty() {
            return;
        }

        // Skip if it's the same as the last entry
        if let Some(last) = self.entries.last()
            && last == trimmed
        {
            return;
        }

        self.entries.push(trimmed.to_string());

        // Trim history if it exceeds max size
        if self.entries.len() > self.max_size {
            self.entries.remove(0);
        }

        // Reset navigation state
        self.reset();
    }

    /// Navigate to the previous prompt in history (up arrow)
    /// Returns the previous prompt, or None if at the beginning
    pub fn prev(&mut self, current_input: &str) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }

        match self.current_index {
            None => {
                // First time navigating - save current input and start from end
                self.temp_input = Some(current_input.to_string());
                self.current_index = Some(self.entries.len() - 1);
                Some(self.entries[self.entries.len() - 1].clone())
            }
            Some(idx) => {
                if idx > 0 {
                    self.current_index = Some(idx - 1);
                    Some(self.entries[idx - 1].clone())
                } else {
                    // Already at the oldest entry
                    None
                }
            }
        }
    }

    /// Navigate to the next prompt in history (down arrow)
    /// Returns the next prompt, or the original input if at the end
    pub fn next_entry(&mut self) -> Option<String> {
        match self.current_index {
            None => None,
            Some(idx) => {
                if idx < self.entries.len() - 1 {
                    self.current_index = Some(idx + 1);
                    Some(self.entries[idx + 1].clone())
                } else {
                    // At the newest entry - return to original input
                    let temp = self.temp_input.clone();
                    self.reset();
                    temp
                }
            }
        }
    }

    /// Resets the navigation state
    pub fn reset(&mut self) {
        self.current_index = None;
        self.temp_input = None;
    }

    /// Returns true if currently navigating through history
    pub fn is_navigating(&self) -> bool {
        self.current_index.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_add_and_navigate() {
        let mut history = PromptHistory::new(100);

        history.add("first".to_string());
        history.add("second".to_string());
        history.add("third".to_string());

        assert_eq!(history.prev(""), Some("third".to_string()));
        assert_eq!(history.prev(""), Some("second".to_string()));
        assert_eq!(history.prev(""), Some("first".to_string()));
        assert_eq!(history.prev(""), None); // At the beginning

        assert_eq!(history.next_entry(), Some("second".to_string()));
        assert_eq!(history.next_entry(), Some("third".to_string()));
        assert_eq!(history.next_entry(), Some("".to_string())); // Back to original
    }

    #[test]
    fn test_skip_duplicates() {
        let mut history = PromptHistory::new(100);

        history.add("first".to_string());
        history.add("first".to_string());
        history.add("second".to_string());

        assert_eq!(history.entries.len(), 2);
    }

    #[test]
    fn test_skip_empty() {
        let mut history = PromptHistory::new(100);

        history.add("".to_string());
        history.add("   ".to_string());
        history.add("first".to_string());

        assert_eq!(history.entries.len(), 1);
    }

    #[test]
    fn test_max_size() {
        let mut history = PromptHistory::new(3);

        history.add("first".to_string());
        history.add("second".to_string());
        history.add("third".to_string());
        history.add("fourth".to_string());

        assert_eq!(history.entries.len(), 3);
        assert_eq!(history.entries[0], "second");
        assert_eq!(history.entries[2], "fourth");
    }

    #[test]
    fn test_preserve_current_input() {
        let mut history = PromptHistory::new(100);

        history.add("old command".to_string());

        let current = "new command in progress";
        assert_eq!(history.prev(current), Some("old command".to_string()));
        assert_eq!(history.next_entry(), Some(current.to_string()));
    }

    #[test]
    fn test_persistence() {
        use tempfile::NamedTempFile;

        // Create a temporary file
        let temp_file = NamedTempFile::new().expect("Failed to create temporary file");
        let path = temp_file.path();

        // Create history and add some entries
        {
            let mut history =
                PromptHistory::with_file(100, path).expect("Failed to create history with file");
            history.add("command 1".to_string());
            history.add("command 2".to_string());
            history.add("command 3".to_string());
            history.save().expect("Failed to save history");
        }

        // Load history in a new instance
        let mut history =
            PromptHistory::with_file(100, path).expect("Failed to create history with file");
        assert_eq!(history.entries.len(), 3);
        assert_eq!(history.prev(""), Some("command 3".to_string()));
        assert_eq!(history.prev(""), Some("command 2".to_string()));
        assert_eq!(history.prev(""), Some("command 1".to_string()));
    }

    #[test]
    fn test_persistence_with_max_size() {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new().expect("Failed to create temporary file");
        let path = temp_file.path();

        // Write more entries than max_size
        {
            let mut file = fs::File::create(path).expect("Failed to create file");
            for i in 1..=10 {
                writeln!(file, "command {}", i).expect("Failed to write to file");
            }
        }

        // Load with max_size of 5
        let history =
            PromptHistory::with_file(5, path).expect("Failed to create history with file");
        assert_eq!(history.entries.len(), 5);
        assert_eq!(history.entries[0], "command 6");
        assert_eq!(history.entries[4], "command 10");
    }
}
