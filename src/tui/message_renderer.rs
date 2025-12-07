use super::app_state::{AppState, MessageLine};
use super::markdown::MarkdownRenderer;
use crate::tui::terminal::HooshTerminal;
use anyhow::Result;
use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
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
        terminal_width: usize,
        terminal: &mut HooshTerminal,
    ) -> Result<()> {
        let rendered_lines = self.markdown_renderer.render(&markdown);
        let wrapped_lines = self.wrap_styled_lines(rendered_lines, terminal_width);
        let line_count = wrapped_lines.len() as u16;

        terminal.insert_before(line_count, |buf| {
            Paragraph::new(Text::from(wrapped_lines)).render(buf.area, buf);
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

    /// Wraps styled lines to fit within terminal width while preserving formatting
    fn wrap_styled_lines(
        &self,
        lines: Vec<Line<'static>>,
        terminal_width: usize,
    ) -> Vec<Line<'static>> {
        let mut wrapped_lines = Vec::new();

        for line in lines {
            // Calculate the actual text width (without ANSI codes)
            let text_width: usize = line.spans.iter().map(|s| s.content.len()).sum();

            // If the line fits within terminal width, keep it as-is
            if text_width <= terminal_width {
                wrapped_lines.push(line);
                continue;
            }

            // Need to wrap this line - preserve indentation from first span if it's whitespace
            let mut indent = String::new();
            if let Some(first_span) = line.spans.first() {
                let content = first_span.content.as_ref();
                let trimmed = content.trim_start();
                if trimmed.len() < content.len() {
                    indent = " ".repeat(content.len() - trimmed.len());
                }
            }

            // Flatten spans into styled characters for precise wrapping
            let mut chars_with_style: Vec<(char, Style)> = Vec::new();
            for span in &line.spans {
                for ch in span.content.chars() {
                    chars_with_style.push((ch, span.style));
                }
            }

            // Wrap by building new lines respecting word boundaries
            let mut current_line_chars: Vec<(char, Style)> = Vec::new();
            let mut current_width = 0;
            let mut word_buffer: Vec<(char, Style)> = Vec::new();
            let mut word_width = 0;

            for (ch, style) in chars_with_style {
                if ch.is_whitespace() {
                    // Flush word buffer to current line
                    if current_width + word_width <= terminal_width {
                        current_line_chars.append(&mut word_buffer);
                        current_width += word_width;
                        word_width = 0;

                        // Add the whitespace if it fits
                        if current_width < terminal_width {
                            current_line_chars.push((ch, style));
                            current_width += 1;
                        } else {
                            // Start new line, add indent
                            wrapped_lines.push(self.chars_to_line(current_line_chars));
                            current_line_chars = Vec::new();
                            for indent_ch in indent.chars() {
                                current_line_chars.push((indent_ch, Style::default()));
                            }
                            current_width = indent.len();
                        }
                    } else {
                        // Word doesn't fit, start new line
                        if !current_line_chars.is_empty() {
                            wrapped_lines.push(self.chars_to_line(current_line_chars));
                            current_line_chars = Vec::new();
                        }
                        // Add indent
                        for indent_ch in indent.chars() {
                            current_line_chars.push((indent_ch, Style::default()));
                        }
                        current_width = indent.len();

                        // Add the word
                        current_line_chars.append(&mut word_buffer);
                        current_width += word_width;
                        word_width = 0;

                        // Add whitespace if it fits
                        if current_width < terminal_width {
                            current_line_chars.push((ch, style));
                            current_width += 1;
                        }
                    }
                } else {
                    // Add to word buffer
                    word_buffer.push((ch, style));
                    word_width += 1;
                }
            }

            // Flush remaining word buffer
            if !word_buffer.is_empty() {
                if current_width + word_width <= terminal_width {
                    current_line_chars.extend(word_buffer);
                } else {
                    // Start new line for the word
                    if !current_line_chars.is_empty() {
                        wrapped_lines.push(self.chars_to_line(current_line_chars));
                        current_line_chars = Vec::new();
                    }
                    // Add indent
                    for indent_ch in indent.chars() {
                        current_line_chars.push((indent_ch, Style::default()));
                    }
                    current_line_chars.extend(word_buffer);
                }
            }

            // Flush current line
            if !current_line_chars.is_empty() {
                wrapped_lines.push(self.chars_to_line(current_line_chars));
            }
        }

        wrapped_lines
    }

    /// Converts a vector of (char, Style) tuples back into a styled Line
    fn chars_to_line(&self, chars: Vec<(char, Style)>) -> Line<'static> {
        if chars.is_empty() {
            return Line::from("");
        }

        let mut spans = Vec::new();
        let mut current_text = String::new();
        let mut current_style = chars[0].1;

        for (ch, style) in chars {
            if style == current_style {
                current_text.push(ch);
            } else {
                // Style changed, flush current span
                if !current_text.is_empty() {
                    spans.push(Span::styled(current_text.clone(), current_style));
                    current_text.clear();
                }
                current_style = style;
                current_text.push(ch);
            }
        }

        // Flush final span
        if !current_text.is_empty() {
            spans.push(Span::styled(current_text, current_style));
        }

        Line::from(spans)
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

    #[test]
    fn test_wrap_styled_lines_short_line() {
        let renderer = MessageRenderer::new();
        let lines = vec![Line::from(vec![
            Span::raw("Hello "),
            Span::styled("world", Style::default().fg(ratatui::style::Color::Red)),
        ])];

        let wrapped = renderer.wrap_styled_lines(lines, 80);
        assert_eq!(wrapped.len(), 1);
    }

    #[test]
    fn test_wrap_styled_lines_long_line() {
        let renderer = MessageRenderer::new();
        let lines = vec![Line::from(vec![Span::raw(
            "This is a very long message that definitely needs to be wrapped because it exceeds the terminal width and should be split across multiple lines",
        )])];

        let wrapped = renderer.wrap_styled_lines(lines, 50);
        assert!(wrapped.len() > 1);
    }

    #[test]
    fn test_wrap_styled_lines_preserves_formatting() {
        let renderer = MessageRenderer::new();
        let red_style = Style::default().fg(ratatui::style::Color::Red);
        let lines = vec![Line::from(vec![
            Span::raw("Normal text "),
            Span::styled(
                "red colored text that is quite long and needs wrapping",
                red_style,
            ),
        ])];

        let wrapped = renderer.wrap_styled_lines(lines, 30);
        assert!(wrapped.len() > 1);

        // Check that red styling is preserved in wrapped lines
        let has_red_spans = wrapped.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.style.fg == Some(ratatui::style::Color::Red))
        });
        assert!(
            has_red_spans,
            "Red styling should be preserved after wrapping"
        );
    }

    #[test]
    fn test_wrap_styled_lines_with_indentation() {
        let renderer = MessageRenderer::new();
        let lines = vec![Line::from(vec![Span::raw(
            "  Indented text that is very long and needs to be wrapped to fit within the terminal width",
        )])];

        let wrapped = renderer.wrap_styled_lines(lines, 40);
        assert!(wrapped.len() > 1);

        // All wrapped lines should preserve indentation
        for line in &wrapped {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(
                text.starts_with("  "),
                "Wrapped line should preserve indentation: '{}'",
                text
            );
        }
    }

    #[test]
    fn test_chars_to_line_single_style() {
        let renderer = MessageRenderer::new();
        let chars = vec![('H', Style::default()), ('i', Style::default())];

        let line = renderer.chars_to_line(chars);
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].content, "Hi");
    }

    #[test]
    fn test_chars_to_line_multiple_styles() {
        let renderer = MessageRenderer::new();
        let style1 = Style::default();
        let style2 = Style::default().fg(ratatui::style::Color::Red);

        let chars = vec![('H', style1), ('i', style1), (' ', style2), ('!', style2)];

        let line = renderer.chars_to_line(chars);
        assert_eq!(line.spans.len(), 2);
        assert_eq!(line.spans[0].content, "Hi");
        assert_eq!(line.spans[1].content, " !");
    }

    #[test]
    fn test_chars_to_line_empty() {
        let renderer = MessageRenderer::new();
        let line = renderer.chars_to_line(vec![]);
        assert_eq!(line.spans.len(), 0);
    }

    #[test]
    fn test_wrap_styled_lines_preserves_heading_colors() {
        let renderer = MessageRenderer::new();
        let heading_style = Style::default()
            .fg(ratatui::style::Color::Magenta)
            .add_modifier(ratatui::style::Modifier::BOLD);

        // Short heading that doesn't need wrapping
        let short_heading = vec![Line::from(vec![Span::styled(
            "Introduction",
            heading_style,
        )])];
        let wrapped = renderer.wrap_styled_lines(short_heading, 80);

        assert_eq!(wrapped.len(), 1);
        assert_eq!(
            wrapped[0].spans[0].style.fg,
            Some(ratatui::style::Color::Magenta)
        );
        assert!(
            wrapped[0].spans[0]
                .style
                .add_modifier
                .contains(ratatui::style::Modifier::BOLD)
        );

        // Long heading that needs wrapping
        let long_heading = vec![Line::from(vec![Span::styled(
            "A Comprehensive Guide to Markdown with Very Long Heading Text",
            heading_style,
        )])];
        let wrapped = renderer.wrap_styled_lines(long_heading, 30);

        assert!(wrapped.len() > 1);
        // Check that all wrapped lines preserve the heading style
        for line in &wrapped {
            for span in &line.spans {
                if !span.content.trim().is_empty() {
                    assert_eq!(
                        span.style.fg,
                        Some(ratatui::style::Color::Magenta),
                        "Heading color should be preserved in wrapped line: '{}'",
                        span.content
                    );
                    assert!(
                        span.style
                            .add_modifier
                            .contains(ratatui::style::Modifier::BOLD),
                        "Heading bold modifier should be preserved in wrapped line: '{}'",
                        span.content
                    );
                }
            }
        }
    }
}
