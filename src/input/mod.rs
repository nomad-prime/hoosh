use anyhow::Result;
use dirs::config_dir;
use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::history::{FileHistory, History};
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{CompletionType, Config, Context, Editor, Helper};
use std::borrow::Cow;
use std::path::PathBuf;

pub struct InputHelper {
    completer: FilenameCompleter,
    highlighter: MatchingBracketHighlighter,
    hinter: HistoryHinter,
    commands: Vec<String>,
}

impl InputHelper {
    pub fn new() -> Self {
        Self {
            completer: FilenameCompleter::new(),
            highlighter: MatchingBracketHighlighter::new(),
            hinter: HistoryHinter::new(),
            commands: vec![
                "/help".to_string(),
                "/tools".to_string(),
                "/history".to_string(),
                "/clear".to_string(),
                "exit".to_string(),
                "quit".to_string(),
            ],
        }
    }

    pub fn add_command(&mut self, command: String) {
        if !self.commands.contains(&command) {
            self.commands.push(command);
        }
    }
}

impl Completer for InputHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        // If line starts with @, use file completion
        if line.starts_with('@') {
            // Remove @ and complete the file path
            let path_part = &line[1..];
            let (file_pos, mut candidates) = self.completer.complete(path_part, pos - 1, ctx)?;

            // Add @ prefix back to candidates
            for candidate in &mut candidates {
                candidate.display = format!("@{}", candidate.display);
                candidate.replacement = format!("@{}", candidate.replacement);
            }

            return Ok((file_pos, candidates));
        }

        // If line starts with /, complete commands
        if line.starts_with('/') {
            let matches: Vec<Pair> = self
                .commands
                .iter()
                .filter(|cmd| cmd.starts_with(line))
                .map(|cmd| Pair {
                    display: cmd.clone(),
                    replacement: cmd.clone(),
                })
                .collect();
            return Ok((0, matches));
        }

        // Default to file completion for other cases
        self.completer.complete(line, pos, ctx)
    }
}

impl Hinter for InputHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

impl Highlighter for InputHelper {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }

    fn highlight_char(&self, line: &str, pos: usize, forced: bool) -> bool {
        self.highlighter.highlight_char(line, pos, forced)
    }
}

impl Validator for InputHelper {
    fn validate(&self, _ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        Ok(ValidationResult::Valid(None))
    }
}

impl Helper for InputHelper {}

/// Interactive input handler with history, autocomplete, and keybindings
pub struct InputHandler {
    editor: Editor<InputHelper, FileHistory>,
    history_path: PathBuf,
}

impl InputHandler {
    pub fn new() -> Result<Self> {
        let config = Config::builder()
            .history_ignore_space(true)
            .completion_type(CompletionType::List)
            .auto_add_history(true)
            .build();

        let helper = InputHelper::new();
        let mut editor = Editor::with_config(config)?;
        editor.set_helper(Some(helper));

        // Setup keybindings (rustyline has these by default)
        // Ctrl+A: beginning of line
        // Ctrl+E: end of line
        // Ctrl+W: delete word backwards
        // Ctrl+K: kill to end of line
        // Ctrl+U: kill to beginning of line
        // Alt+B: move backwards one word
        // Alt+F: move forwards one word

        let history_path = Self::get_history_path()?;

        Ok(Self {
            editor,
            history_path,
        })
    }

    fn get_history_path() -> Result<PathBuf> {
        let config_dir = config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        let hoosh_dir = config_dir.join("hoosh");
        std::fs::create_dir_all(&hoosh_dir)?;
        Ok(hoosh_dir.join("history.txt"))
    }

    /// Load history from disk
    pub fn load_history(&mut self) -> Result<()> {
        if self.history_path.exists() {
            self.editor.load_history(&self.history_path).ok();
        }
        Ok(())
    }

    /// Save history to disk
    pub fn save_history(&mut self) -> Result<()> {
        self.editor.save_history(&self.history_path)?;
        Ok(())
    }

    /// Read a line of input with prompt
    pub fn readline(&mut self, prompt: &str) -> Result<Option<String>> {
        match self.editor.readline(prompt) {
            Ok(line) => Ok(Some(line)),
            Err(ReadlineError::Interrupted) => {
                // Ctrl-C
                Ok(None)
            }
            Err(ReadlineError::Eof) => {
                // Ctrl-D
                Ok(None)
            }
            Err(err) => Err(err.into()),
        }
    }

    /// Add a command to the autocomplete list
    pub fn add_command(&mut self, command: String) {
        if let Some(helper) = self.editor.helper_mut() {
            helper.add_command(command);
        }
    }

    /// Get history entries
    pub fn history(&self) -> Vec<String> {
        self.editor
            .history()
            .iter()
            .map(|entry| entry.to_string())
            .collect()
    }

    /// Clear history
    pub fn clear_history(&mut self) {
        let _ = self.editor.history_mut().clear();
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new().expect("Failed to create input handler")
    }
}