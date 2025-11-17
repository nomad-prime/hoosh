use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use crate::tui::palette;

pub struct MarkdownRenderer {
    syntax_set: SyntaxSet,
    theme: Theme,
}

impl MarkdownRenderer {
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let theme = theme_set.themes["base16-ocean.dark"].clone();

        Self { syntax_set, theme }
    }

    pub fn render(&self, markdown: &str) -> Vec<Line<'static>> {
        self.render_with_indent(markdown, "  ")
    }

    pub fn render_with_indent(&self, markdown: &str, indent: &str) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let parser = Parser::new_ext(markdown, Options::all());

        let mut current_line_spans: Vec<Span<'static>> = Vec::new();
        let mut in_code_block = false;
        let mut code_buffer = String::new();
        let mut code_language: Option<String> = None;
        let mut list_depth: usize = 0;
        let mut list_stack: Vec<Option<usize>> = Vec::new(); // Stack of list counters (None = unordered)
        let mut heading_level = HeadingLevel::H1;
        let mut in_emphasis = false;
        let mut in_strong = false;
        let mut in_strikethrough = false;

        for event in parser {
            match event {
                Event::Start(tag) => match tag {
                    Tag::CodeBlock(kind) => {
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_line_spans)));
                        }
                        in_code_block = true;
                        code_buffer.clear();
                        code_language = match kind {
                            CodeBlockKind::Fenced(lang) => {
                                if lang.is_empty() {
                                    None
                                } else {
                                    Some(lang.to_string())
                                }
                            }
                            CodeBlockKind::Indented => None,
                        };
                    }
                    Tag::Heading { level, .. } => {
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_line_spans)));
                        }
                        // Add blank line before headings (except at the very start)
                        if !lines.is_empty() {
                            lines.push(Line::from(""));
                        }
                        heading_level = level;
                    }
                    Tag::Emphasis => {
                        in_emphasis = true;
                    }
                    Tag::Strong => {
                        in_strong = true;
                    }
                    Tag::Strikethrough => {
                        in_strikethrough = true;
                    }
                    Tag::List(start_number) => {
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_line_spans)));
                        }
                        list_depth += 1;
                        // Push counter for this list level (None = bullet list, Some(n) = numbered list)
                        list_stack.push(start_number.map(|n| n as usize));
                    }
                    Tag::Item => {
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_line_spans)));
                        }
                        let indent = "  ".repeat(list_depth.saturating_sub(1));

                        // Get the current list's counter
                        if let Some(counter_opt) = list_stack.last_mut() {
                            if let Some(counter) = counter_opt {
                                current_line_spans
                                    .push(Span::raw(format!("{}{}. ", indent, counter)));
                                *counter += 1;
                            } else {
                                current_line_spans.push(Span::raw(format!("{}• ", indent)));
                            }
                        }
                    }
                    Tag::Paragraph => {
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_line_spans)));
                        }
                    }
                    Tag::BlockQuote(_) => {
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_line_spans)));
                        }
                        current_line_spans.push(Span::styled(
                            "│ ",
                            Style::default().fg(palette::MARKDOWN_QUOTE),
                        ));
                    }
                    Tag::Link { .. } => {
                        // Links are handled by their text content
                    }
                    Tag::Image { .. } => {
                        // Images - just handle the alt text
                    }
                    Tag::Table(_) | Tag::TableHead | Tag::TableRow | Tag::TableCell => {
                        // Tables - basic support
                    }
                    Tag::FootnoteDefinition(_) => {
                        // Footnote definitions
                    }
                    Tag::HtmlBlock | Tag::MetadataBlock(_) => {
                        // HTML/Metadata blocks
                    }
                },
                Event::End(tag_end) => match tag_end {
                    TagEnd::CodeBlock => {
                        let highlighted = self.render_code_block(
                            code_language.as_deref().unwrap_or(""),
                            &code_buffer,
                        );
                        lines.extend(highlighted);
                        in_code_block = false;
                    }
                    TagEnd::Heading { .. } => {
                        if !current_line_spans.is_empty() {
                            let styled_spans: Vec<Span> = current_line_spans
                                .drain(..)
                                .map(|span| {
                                    let style = self.get_heading_style(heading_level);
                                    Span::styled(span.content, style)
                                })
                                .collect();
                            lines.push(Line::from(styled_spans));
                        }
                    }
                    TagEnd::Emphasis => {
                        in_emphasis = false;
                    }
                    TagEnd::Strong => {
                        in_strong = false;
                    }
                    TagEnd::Strikethrough => {
                        in_strikethrough = false;
                    }
                    TagEnd::List(_) => {
                        list_depth = list_depth.saturating_sub(1);
                        list_stack.pop(); // Pop the counter for this list level
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_line_spans)));
                        }
                    }
                    TagEnd::Item => {
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_line_spans)));
                        }
                    }
                    TagEnd::Paragraph => {
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_line_spans)));
                        }
                    }
                    TagEnd::BlockQuote => {
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_line_spans)));
                        }
                    }
                    _ => {}
                },
                Event::Text(text) => {
                    if in_code_block {
                        code_buffer.push_str(&text);
                    } else {
                        let style = self.get_inline_style(in_emphasis, in_strong, in_strikethrough);
                        current_line_spans.push(Span::styled(text.to_string(), style));
                    }
                }
                Event::Code(code) => {
                    let style = Style::default()
                        .fg(palette::MARKDOWN_CODE_FG)
                        .bg(palette::MARKDOWN_CODE_BG)
                        .add_modifier(Modifier::BOLD);
                    current_line_spans.push(Span::styled(format!("`{}`", code), style));
                }
                Event::SoftBreak => {
                    current_line_spans.push(Span::raw(" "));
                }
                Event::HardBreak => {
                    lines.push(Line::from(std::mem::take(&mut current_line_spans)));
                }
                Event::Rule => {
                    if !current_line_spans.is_empty() {
                        lines.push(Line::from(std::mem::take(&mut current_line_spans)));
                    }
                    lines.push(Line::styled(
                        "─".repeat(80),
                        Style::default().fg(palette::MARKDOWN_RULE),
                    ));
                    lines.push(Line::from(""));
                }
                Event::Html(html) => {
                    current_line_spans.push(Span::styled(
                        html.to_string(),
                        Style::default().fg(palette::MARKDOWN_HTML),
                    ));
                }
                Event::InlineHtml(html) => {
                    current_line_spans.push(Span::styled(
                        html.to_string(),
                        Style::default().fg(palette::MARKDOWN_HTML),
                    ));
                }
                Event::FootnoteReference(name) => {
                    current_line_spans.push(Span::styled(
                        format!("[^{}]", name),
                        Style::default().fg(palette::MARKDOWN_LINK),
                    ));
                }
                Event::TaskListMarker(checked) => {
                    let marker = if checked { "[✓] " } else { "[ ] " };
                    current_line_spans.push(Span::styled(
                        marker,
                        Style::default().fg(palette::MARKDOWN_TASK_MARKER),
                    ));
                }
                _ => {}
            }
        }

        if !current_line_spans.is_empty() {
            lines.push(Line::from(current_line_spans));
        }

        if indent.is_empty() {
            lines
        } else {
            lines
                .into_iter()
                .map(|line| {
                    let mut spans = vec![Span::raw(indent.to_string())];
                    spans.extend(line.spans);
                    Line::from(spans)
                })
                .collect()
        }
    }

    fn render_code_block(&self, language: &str, code: &str) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let syntax = if language.is_empty() {
            self.syntax_set.find_syntax_plain_text()
        } else {
            self.syntax_set
                .find_syntax_by_token(language)
                .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text())
        };

        let mut highlighter = HighlightLines::new(syntax, &self.theme);
        let code_bg = palette::MARKDOWN_CODE_BG;

        let header = if !language.is_empty() {
            format!("┌─ {} ", language)
        } else {
            "┌─ code ".to_string()
        };
        lines.push(Line::styled(
            header,
            Style::default().fg(palette::MARKDOWN_QUOTE).bg(code_bg),
        ));

        for (line_num, line) in LinesWithEndings::from(code).enumerate() {
            let highlighted = highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();

            let mut spans = vec![Span::styled(
                format!("│ {:3} ", line_num + 1),
                Style::default().fg(palette::MARKDOWN_QUOTE).bg(code_bg),
            )];

            for (style, text) in highlighted {
                let fg_color =
                    Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                spans.push(Span::styled(
                    text.to_string(),
                    Style::default().fg(fg_color).bg(code_bg),
                ));
            }

            lines.push(Line::from(spans));
        }

        lines.push(Line::styled(
            "└─",
            Style::default().fg(palette::MARKDOWN_QUOTE).bg(code_bg),
        ));
        lines.push(Line::from(""));

        lines
    }

    fn get_heading_style(&self, level: HeadingLevel) -> Style {
        match level {
            HeadingLevel::H1 => Style::default()
                .fg(palette::INFO)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H2 => Style::default()
                .fg(palette::MARKDOWN_LINK)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H3 => Style::default()
                .fg(palette::SUCCESS)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H4 => Style::default()
                .fg(palette::WARNING)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H5 => Style::default()
                .fg(palette::MARKDOWN_HEADING)
                .add_modifier(Modifier::BOLD),
            HeadingLevel::H6 => Style::default()
                .fg(palette::DESTRUCTIVE)
                .add_modifier(Modifier::BOLD),
        }
    }

    fn get_inline_style(&self, emphasis: bool, strong: bool, strikethrough: bool) -> Style {
        let mut style = Style::default();

        if emphasis {
            style = style.add_modifier(Modifier::ITALIC);
        }
        if strong {
            style = style.add_modifier(Modifier::BOLD);
        }
        if strikethrough {
            style = style.add_modifier(Modifier::CROSSED_OUT);
        }

        style
    }
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_plain_text() {
        let renderer = MarkdownRenderer::new();
        let markdown = "Hello, world!";
        let lines = renderer.render(markdown);

        assert!(!lines.is_empty());
    }

    #[test]
    fn test_heading_spacing() {
        let renderer = MarkdownRenderer::new();
        let markdown = "Intro paragraph\n\n## Heading 1\n\nContent\n\n## Heading 2";
        let lines = renderer.render(markdown);

        // Should have: intro, blank, heading1, blank after heading, content, blank, blank before heading2, heading2, blank after heading
        // At minimum we should have more than just 4 lines
        assert!(lines.len() > 4);

        // Check that there are some empty lines (spacing)
        let empty_lines = lines
            .iter()
            .filter(|l| l.spans.is_empty() || l.spans.iter().all(|s| s.content.trim().is_empty()))
            .count();
        assert!(empty_lines > 0);
    }

    #[test]
    fn test_render_code_block() {
        let renderer = MarkdownRenderer::new();
        let markdown = "```rust\nfn main() {\n    println!(\"Hello\");\n}\n```";
        let lines = renderer.render(markdown);

        assert!(lines.len() > 3);
    }

    #[test]
    fn test_render_inline_code() {
        let renderer = MarkdownRenderer::new();
        let markdown = "This is `inline code` in text.";
        let lines = renderer.render(markdown);

        assert!(!lines.is_empty());
    }

    #[test]
    fn test_render_bold() {
        let renderer = MarkdownRenderer::new();
        let markdown = "This is **bold text**.";
        let lines = renderer.render(markdown);

        assert!(!lines.is_empty());
    }

    #[test]
    fn test_render_italic() {
        let renderer = MarkdownRenderer::new();
        let markdown = "This is *italic text*.";
        let lines = renderer.render(markdown);

        assert!(!lines.is_empty());
    }

    #[test]
    fn test_render_heading() {
        let renderer = MarkdownRenderer::new();
        let markdown = "# Heading 1\n## Heading 2";
        let lines = renderer.render(markdown);

        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_render_list() {
        let renderer = MarkdownRenderer::new();
        let markdown = "- Item 1\n- Item 2\n- Item 3";
        let lines = renderer.render(markdown);

        assert!(lines.len() >= 3);
    }

    #[test]
    fn test_render_ordered_list() {
        let renderer = MarkdownRenderer::new();
        let markdown = "1. First\n2. Second\n3. Third";
        let lines = renderer.render(markdown);

        assert!(lines.len() >= 3);
    }

    #[test]
    fn test_render_blockquote() {
        let renderer = MarkdownRenderer::new();
        let markdown = "> This is a quote";
        let lines = renderer.render(markdown);

        assert!(!lines.is_empty());
    }

    #[test]
    fn test_render_horizontal_rule() {
        let renderer = MarkdownRenderer::new();
        let markdown = "Before\n\n---\n\nAfter";
        let lines = renderer.render(markdown);

        assert!(lines.len() > 2);
    }

    #[test]
    fn test_render_mixed_formatting() {
        let renderer = MarkdownRenderer::new();
        let markdown = "**Bold** and *italic* and `code`";
        let lines = renderer.render(markdown);

        assert!(!lines.is_empty());
    }

    #[test]
    fn test_render_empty_markdown() {
        let renderer = MarkdownRenderer::new();
        let markdown = "";
        let lines = renderer.render(markdown);

        assert!(lines.is_empty() || lines.iter().all(|l| l.spans.is_empty()));
    }

    #[test]
    fn test_code_block_without_language() {
        let renderer = MarkdownRenderer::new();
        let markdown = "```\nplain code\n```";
        let lines = renderer.render(markdown);

        assert!(lines.len() > 2);
    }

    #[test]
    fn test_nested_lists() {
        let renderer = MarkdownRenderer::new();
        let markdown = "- Item 1\n  - Nested 1\n  - Nested 2\n- Item 2";
        let lines = renderer.render(markdown);

        assert!(lines.len() >= 4);
    }
}
