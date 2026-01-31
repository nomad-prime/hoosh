use colored::*;
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Renders markdown to terminal output using ANSI escape codes
pub struct TerminalMarkdownRenderer {}

impl TerminalMarkdownRenderer {
    pub fn new() -> Self {
        Self {}
    }

    /// Render markdown text to a string with ANSI color codes
    pub fn render(&self, markdown: &str) -> String {
        let mut output = String::new();
        let parser = Parser::new_ext(markdown, Options::all());

        let mut in_code_block = false;
        let mut code_buffer = String::new();
        let mut code_language: Option<String> = None;
        let mut list_depth: usize = 0;
        let mut list_stack: Vec<Option<usize>> = Vec::new();
        let mut heading_level = HeadingLevel::H1;
        let mut in_emphasis = false;
        let mut in_strong = false;
        let mut in_strikethrough = false;
        let mut current_line = String::new();

        for event in parser {
            match event {
                Event::Start(tag) => match tag {
                    Tag::CodeBlock(kind) => {
                        if !current_line.is_empty() {
                            output.push_str(&current_line);
                            output.push('\n');
                            current_line.clear();
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
                        if !current_line.is_empty() {
                            output.push_str(&current_line);
                            output.push('\n');
                            current_line.clear();
                        }
                        if !output.is_empty() {
                            output.push('\n');
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
                        if !current_line.is_empty() {
                            output.push_str(&current_line);
                            output.push('\n');
                            current_line.clear();
                        }
                        list_depth += 1;
                        list_stack.push(start_number.map(|n| n as usize));
                    }
                    Tag::Item => {
                        if !current_line.is_empty() {
                            output.push_str(&current_line);
                            output.push('\n');
                            current_line.clear();
                        }
                        let indent = "  ".repeat(list_depth.saturating_sub(1));

                        if let Some(counter_opt) = list_stack.last_mut() {
                            if let Some(counter) = counter_opt {
                                current_line.push_str(&format!("{}{}. ", indent, counter));
                                *counter += 1;
                            } else {
                                current_line.push_str(&format!("{}• ", indent));
                            }
                        }
                    }
                    Tag::Paragraph => {
                        if !current_line.is_empty() {
                            output.push_str(&current_line);
                            output.push('\n');
                            current_line.clear();
                        }
                    }
                    Tag::BlockQuote(_) => {
                        if !current_line.is_empty() {
                            output.push_str(&current_line);
                            output.push('\n');
                            current_line.clear();
                        }
                        current_line.push_str(&format!("{} ", "│".bright_black()));
                    }
                    Tag::Table(_) => {
                        if !current_line.is_empty() {
                            output.push_str(&current_line);
                            output.push('\n');
                            current_line.clear();
                        }
                    }
                    _ => {}
                },
                Event::End(tag_end) => match tag_end {
                    TagEnd::CodeBlock => {
                        let rendered = self.render_code_block(
                            code_language.as_deref().unwrap_or(""),
                            &code_buffer,
                        );
                        output.push_str(&rendered);
                        output.push('\n');
                        in_code_block = false;
                    }
                    TagEnd::Heading { .. } => {
                        if !current_line.is_empty() {
                            let styled = self.style_heading(&current_line, heading_level);
                            output.push_str(&styled);
                            output.push('\n');
                            current_line.clear();
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
                        list_stack.pop();
                        if !current_line.is_empty() {
                            output.push_str(&current_line);
                            output.push('\n');
                            current_line.clear();
                        }
                    }
                    TagEnd::Item => {
                        if !current_line.is_empty() {
                            output.push_str(&current_line);
                            output.push('\n');
                            current_line.clear();
                        }
                    }
                    TagEnd::Paragraph => {
                        if !current_line.is_empty() {
                            output.push_str(&current_line);
                            output.push('\n');
                            current_line.clear();
                        }
                    }
                    TagEnd::BlockQuote => {
                        if !current_line.is_empty() {
                            output.push_str(&current_line);
                            output.push('\n');
                            current_line.clear();
                        }
                    }
                    _ => {}
                },
                Event::Text(text) => {
                    if in_code_block {
                        code_buffer.push_str(&text);
                    } else {
                        let styled = self.style_text(&text, in_emphasis, in_strong, in_strikethrough);
                        current_line.push_str(&styled);
                    }
                }
                Event::Code(code) => {
                    let styled = format!("`{}`", code).on_black().bright_white().bold();
                    current_line.push_str(&styled.to_string());
                }
                Event::SoftBreak => {
                    current_line.push(' ');
                }
                Event::HardBreak => {
                    output.push_str(&current_line);
                    output.push('\n');
                    current_line.clear();
                }
                Event::Rule => {
                    if !current_line.is_empty() {
                        output.push_str(&current_line);
                        output.push('\n');
                        current_line.clear();
                    }
                    output.push_str(&format!("{}\n", "─".repeat(80).bright_black()));
                }
                Event::Html(html) | Event::InlineHtml(html) => {
                    current_line.push_str(&html.bright_black().to_string());
                }
                Event::FootnoteReference(name) => {
                    current_line.push_str(&format!("[^{}]", name).blue().to_string());
                }
                Event::TaskListMarker(checked) => {
                    let marker = if checked { "[✓] " } else { "[ ] " };
                    current_line.push_str(&marker.green().to_string());
                }
                _ => {}
            }
        }

        if !current_line.is_empty() {
            output.push_str(&current_line);
            output.push('\n');
        }

        output
    }

    fn render_code_block(&self, language: &str, code: &str) -> String {
        let mut output = String::new();

        let header = if !language.is_empty() {
            format!("┌─ {} ", language)
        } else {
            "┌─ code ".to_string()
        };
        output.push_str(&header.bright_black().to_string());
        output.push('\n');

        for (line_num, line) in code.lines().enumerate() {
            let line_prefix = format!("│ {:3} ", line_num + 1);
            output.push_str(&line_prefix.bright_black().to_string());
            output.push_str(line);
            output.push('\n');
        }

        output.push_str(&"└─".bright_black().to_string());
        output.push('\n');

        output
    }

    fn style_heading(&self, text: &str, level: HeadingLevel) -> String {
        match level {
            HeadingLevel::H1 => text.cyan().bold().to_string(),
            HeadingLevel::H2 => text.blue().bold().to_string(),
            HeadingLevel::H3 => text.green().bold().to_string(),
            HeadingLevel::H4 => text.yellow().bold().to_string(),
            HeadingLevel::H5 => text.magenta().bold().to_string(),
            HeadingLevel::H6 => text.red().bold().to_string(),
        }
    }

    fn style_text(&self, text: &str, emphasis: bool, strong: bool, strikethrough: bool) -> String {
        let mut result = text.normal();

        if emphasis {
            result = result.italic();
        }
        if strong {
            result = result.bold();
        }
        if strikethrough {
            result = result.strikethrough();
        }

        result.to_string()
    }
}

impl Default for TerminalMarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_plain_text() {
        let renderer = TerminalMarkdownRenderer::new();
        let output = renderer.render("Hello, world!");
        assert!(output.contains("Hello, world!"));
    }

    #[test]
    fn test_render_heading() {
        let renderer = TerminalMarkdownRenderer::new();
        let output = renderer.render("# Heading 1");
        assert!(!output.is_empty());
    }

    #[test]
    fn test_render_code_block() {
        let renderer = TerminalMarkdownRenderer::new();
        let output = renderer.render("```rust\nfn main() {}\n```");
        assert!(output.contains("rust"));
        assert!(output.contains("fn main()"));
    }

    #[test]
    fn test_render_inline_code() {
        let renderer = TerminalMarkdownRenderer::new();
        let output = renderer.render("This is `inline code` text.");
        assert!(output.contains("inline code"));
    }

    #[test]
    fn test_render_list() {
        let renderer = TerminalMarkdownRenderer::new();
        let output = renderer.render("- Item 1\n- Item 2");
        assert!(output.contains("•"));
    }
}
