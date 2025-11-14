use super::app_state::{AppState, MessageLine};
use super::markdown::MarkdownRenderer;
use crate::tui::terminal::HooshTerminal;
use anyhow::Result;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Paragraph, Widget};

/// Handles rendering of chat messages in the TUI
///
/// The MessageRenderer is responsible for:
/// - Wrapping text to fit terminal width
/// - Preserving indentation for continuation lines
/// - Handling both plain and styled messages
///
/// # Example
/// ```no_run
/// # use hoosh::tui::MessageRenderer;
/// let renderer = MessageRenderer::new();
/// // renderer.render_pending_messages(app, &mut terminal)?;
/// ```
pub struct MessageRenderer {
    markdown_renderer: MarkdownRenderer,
}

impl MessageRenderer {
    pub fn new() -> Self {
        Self {
            markdown_renderer: MarkdownRenderer::new(),
        }
    }

    pub fn render_pending_messages(
        &self,
        app: &mut AppState,
        terminal: &mut HooshTerminal,
    ) -> Result<()> {
        if !app.has_pending_messages() {
            return Ok(());
        }

        let terminal_width = terminal.size()?.width as usize;

        for message in app.drain_pending_messages() {
            self.render_single_message(message, terminal_width, terminal)?;
        }

        Ok(())
    }

    fn render_single_message(
        &self,
        message: MessageLine,
        terminal_width: usize,
        terminal: &mut HooshTerminal,
    ) -> Result<()> {
        match message {
            MessageLine::Plain(text) => self.render_plain_message(text, terminal_width, terminal),
            MessageLine::Styled(line) => self.render_styled_message(line, terminal),
            MessageLine::Markdown(markdown) => {
                self.render_markdown_message(markdown, terminal_width, terminal)
            }
        }
    }

    fn render_plain_message(
        &self,
        text: String,
        terminal_width: usize,
        terminal: &mut HooshTerminal,
    ) -> Result<()> {
        let wrapped_lines = self.wrap_plain_text(&text, terminal_width);
        let line_count = wrapped_lines.len() as u16;

        terminal.insert_before(line_count, |buf| {
            Paragraph::new(Text::from(wrapped_lines)).render(buf.area, buf);
        })?;

        Ok(())
    }

    fn render_styled_message(
        &self,
        line: Line<'static>,
        terminal: &mut HooshTerminal,
    ) -> Result<()> {
        let terminal_width = terminal.size()?.width as usize;
        let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        let wrapped_line_count = if line_text.is_empty() {
            1
        } else {
            textwrap::wrap(&line_text, terminal_width).len()
        };

        terminal.insert_before(wrapped_line_count as u16, |buf| {
            Paragraph::new(line).render(buf.area, buf);
        })?;

        Ok(())
    }

    fn render_markdown_message(
        &self,
        markdown: String,
        _terminal_width: usize,
        terminal: &mut HooshTerminal,
    ) -> Result<()> {
        let rendered_lines = self.markdown_renderer.render(&markdown);
        let line_count = rendered_lines.len() as u16;

        terminal.insert_before(line_count, |buf| {
            Paragraph::new(Text::from(rendered_lines)).render(buf.area, buf);
        })?;

        Ok(())
    }

    fn wrap_plain_text(&self, text: &str, terminal_width: usize) -> Vec<Line<'static>> {
        let mut wrapped_lines = Vec::new();

        if text.is_empty() {
            wrapped_lines.push(Line::from(""));
            return wrapped_lines;
        }

        for line in text.lines() {
            if line.is_empty() {
                wrapped_lines.push(Line::from(""));
                continue;
            }

            let wrapped = self.wrap_single_line(line, terminal_width);
            wrapped_lines.extend(wrapped);
        }

        wrapped_lines
    }

    fn wrap_single_line(&self, line: &str, max_width: usize) -> Vec<Line<'static>> {
        // Detect indentation
        let indent_len = line.len() - line.trim_start().len();
        let indent = " ".repeat(indent_len);

        // Use textwrap with preserved indentation for continuation lines
        let options = textwrap::Options::new(max_width)
            .initial_indent("")
            .subsequent_indent(&indent);

        let wrapped = textwrap::wrap(line, &options);
        wrapped
            .into_iter()
            .map(|w| Line::from(w.to_string()))
            .collect()
    }
}

impl Default for MessageRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_plain_text_fits_on_one_line() {
        let renderer = MessageRenderer::new();
        let text = "Short message";
        let lines = renderer.wrap_plain_text(text, 80);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].to_string(), "Short message");
    }

    #[test]
    fn test_wrap_plain_text_needs_wrapping() {
        let renderer = MessageRenderer::new();
        let text = "This is a very long message that definitely needs to be wrapped because it exceeds the terminal width";
        let lines = renderer.wrap_plain_text(text, 40);

        assert!(lines.len() > 1);
    }

    #[test]
    fn test_wrap_preserves_indentation() {
        let renderer = MessageRenderer::new();
        let text = "  Indented message that is very long and needs wrapping";
        let lines = renderer.wrap_plain_text(text, 30);

        assert!(lines.len() > 1);
        // First line should have original indentation
        assert!(lines[0].to_string().starts_with("  "));
        // Continuation lines should also have same indentation
        assert!(lines[1].to_string().starts_with("  "));
    }

    #[test]
    fn test_empty_text() {
        let renderer = MessageRenderer::new();
        let lines = renderer.wrap_plain_text("", 80);

        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_multiline_text() {
        let renderer = MessageRenderer::new();
        let text = "Line 1\nLine 2\nLine 3";
        let lines = renderer.wrap_plain_text(text, 80);

        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_empty_lines_preserved() {
        let renderer = MessageRenderer::new();
        let text = "Line 1\n\nLine 3";
        let lines = renderer.wrap_plain_text(text, 80);

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1].to_string(), "");
    }

    #[test]
    fn test_wrap_single_line_with_indentation() {
        let renderer = MessageRenderer::new();
        let line = "  This is an indented line that is quite long";
        let wrapped = renderer.wrap_single_line(line, 30);

        assert!(wrapped.len() > 1);
        // All lines should preserve indentation
        for wrapped_line in &wrapped {
            assert!(wrapped_line.to_string().starts_with("  "));
        }
    }

    #[test]
    fn test_wrap_single_line_no_wrap_needed() {
        let renderer = MessageRenderer::new();
        let line = "Short";
        let wrapped = renderer.wrap_single_line(line, 80);

        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0].to_string(), "Short");
    }

    #[test]
    fn test_very_narrow_terminal() {
        let renderer = MessageRenderer::new();
        let text = "word";
        let lines = renderer.wrap_plain_text(text, 2);

        // Should still wrap, even if narrower than word
        assert!(!lines.is_empty());
    }
}
