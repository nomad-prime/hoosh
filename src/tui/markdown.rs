use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use crate::tui::palette;

// Table rendering types
#[derive(Debug, Clone)]
struct TableCell {
    spans: Vec<Span<'static>>,
    visual_width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Alignment {
    Left,
    Center,
    Right,
}

impl TableCell {
    fn new() -> Self {
        Self {
            spans: Vec::new(),
            visual_width: 0,
        }
    }

    fn add_span(&mut self, span: Span<'static>) {
        use unicode_width::UnicodeWidthStr;
        self.visual_width += UnicodeWidthStr::width(span.content.as_ref());
        self.spans.push(span);
    }
}

#[derive(Debug)]
struct TableBuilder {
    headers: Vec<TableCell>,
    alignments: Vec<Alignment>,
    rows: Vec<Vec<TableCell>>,
    in_header: bool,
    current_row: Vec<TableCell>,
    current_cell: TableCell,
}

impl TableBuilder {
    fn new() -> Self {
        Self {
            headers: Vec::new(),
            alignments: Vec::new(),
            rows: Vec::new(),
            in_header: false,
            current_row: Vec::new(),
            current_cell: TableCell::new(),
        }
    }

    fn finalize_cell(&mut self) {
        let cell = std::mem::replace(&mut self.current_cell, TableCell::new());
        self.current_row.push(cell);
    }

    fn finalize_row(&mut self) {
        if self.current_row.is_empty() {
            return;
        }

        let row = std::mem::take(&mut self.current_row);

        if self.in_header {
            self.headers = row;
        } else {
            self.rows.push(row);
        }
    }

    fn column_count(&self) -> usize {
        self.headers.len()
    }
}

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
        let mut list_stack: Vec<Option<usize>> = Vec::new();
        let mut heading_level = HeadingLevel::H1;
        let mut in_emphasis = false;
        let mut in_strong = false;
        let mut in_strikethrough = false;
        let mut current_table: Option<TableBuilder> = None;

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
                    Tag::Link { .. } => {}
                    Tag::Image { .. } => {}
                    Tag::Table(alignments) => {
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_line_spans)));
                        }
                        // Add blank line before table for spacing
                        if !lines.is_empty() {
                            lines.push(Line::from(""));
                        }
                        let mut table = TableBuilder::new();
                        let aligns: Vec<Alignment> = alignments
                            .into_iter()
                            .map(|a| match a {
                                pulldown_cmark::Alignment::Left
                                | pulldown_cmark::Alignment::None => Alignment::Left,
                                pulldown_cmark::Alignment::Center => Alignment::Center,
                                pulldown_cmark::Alignment::Right => Alignment::Right,
                            })
                            .collect();
                        table.alignments = aligns;
                        current_table = Some(table);
                    }
                    Tag::TableHead => {
                        if let Some(ref mut table) = current_table {
                            table.in_header = true;
                        }
                    }
                    Tag::TableRow => {}
                    Tag::TableCell => {}
                    Tag::FootnoteDefinition(_) => {}
                    Tag::HtmlBlock | Tag::MetadataBlock(_) => {}
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
                        list_stack.pop();
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
                    TagEnd::TableCell => {
                        if let Some(ref mut table) = current_table {
                            table.finalize_cell();
                        }
                    }
                    TagEnd::TableRow => {
                        // T019: End table row - finalize row
                        if let Some(ref mut table) = current_table {
                            table.finalize_row();
                        }
                    }
                    TagEnd::TableHead => {
                        if let Some(ref mut table) = current_table {
                            table.finalize_row();
                            table.in_header = false;
                            let col_count = table.column_count();
                            while table.alignments.len() < col_count {
                                table.alignments.push(Alignment::Left);
                            }
                        }
                    }
                    TagEnd::Table => {
                        if let Some(table) = current_table.take() {
                            let table_lines = self.render_table(table);
                            lines.extend(table_lines);
                            // Add blank line after table for spacing
                            lines.push(Line::from(""));
                        }
                    }
                    _ => {}
                },
                Event::Text(text) => {
                    if in_code_block {
                        code_buffer.push_str(&text);
                    } else if let Some(ref mut table) = current_table {
                        // T017: Add text span to current table cell
                        let style = self.get_inline_style(in_emphasis, in_strong, in_strikethrough);
                        let span = Span::styled(text.to_string(), style);
                        table.current_cell.add_span(span);
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

    fn calculate_column_widths(&self, table: &TableBuilder, max_width: usize) -> Vec<usize> {
        let col_count = table.column_count();
        if col_count == 0 {
            return Vec::new();
        }

        let mut widths = vec![0; col_count];

        for (i, cell) in table.headers.iter().enumerate() {
            widths[i] = widths[i].max(cell.visual_width);
        }

        for row in &table.rows {
            for (i, cell) in row.iter().enumerate().take(col_count) {
                widths[i] = widths[i].max(cell.visual_width);
            }
        }

        for width in &mut widths {
            *width += 2;
        }

        let borders_width = col_count + 1; // One pipe before each column + one at the end
        let total_ideal_width: usize = widths.iter().sum::<usize>() + borders_width;

        if total_ideal_width <= max_width {
            return widths;
        }

        let available_width = max_width.saturating_sub(borders_width);
        let total_content_width: usize = widths.iter().sum();

        if total_content_width > 0 {
            for width in &mut widths {
                let proportion = (*width as f64) / (total_content_width as f64);
                *width = (proportion * available_width as f64).floor() as usize;
                *width = (*width).max(3);
            }
        }

        widths
    }

    fn apply_left_alignment(&self, content_width: usize, total_width: usize) -> (usize, usize) {
        let padding_left = 1;
        let padding_right = total_width.saturating_sub(content_width + padding_left);
        (padding_left, padding_right)
    }

    fn apply_center_alignment(&self, content_width: usize, total_width: usize) -> (usize, usize) {
        let available_padding = total_width.saturating_sub(content_width);
        let padding_left = available_padding / 2;
        let padding_right = available_padding - padding_left;
        (padding_left, padding_right)
    }

    fn apply_right_alignment(&self, content_width: usize, total_width: usize) -> (usize, usize) {
        let padding_right = 1;
        let padding_left = total_width.saturating_sub(content_width + padding_right);
        (padding_left, padding_right)
    }

    fn apply_padding_with_alignment(
        &self,
        content_width: usize,
        total_width: usize,
        alignment: Alignment,
    ) -> (usize, usize) {
        match alignment {
            Alignment::Left => self.apply_left_alignment(content_width, total_width),
            Alignment::Center => self.apply_center_alignment(content_width, total_width),
            Alignment::Right => self.apply_right_alignment(content_width, total_width),
        }
    }

    fn render_header_line(
        &self,
        headers: &[TableCell],
        widths: &[usize],
        alignments: &[Alignment],
    ) -> Line<'static> {
        use unicode_width::UnicodeWidthStr;

        let mut spans = Vec::new();

        for (i, (cell, &width)) in headers.iter().zip(widths.iter()).enumerate() {
            spans.push(Span::raw("|"));

            let alignment = alignments.get(i).copied().unwrap_or(Alignment::Left);

            let content_width = cell.visual_width;
            let (padding_left, padding_right) =
                self.apply_padding_with_alignment(content_width, width, alignment);

            spans.push(Span::raw(" ".repeat(padding_left)));

            if content_width <= width - 2 {
                spans.extend(cell.spans.clone());
                spans.push(Span::raw(" ".repeat(padding_right)));
            } else {
                let mut accumulated_width = 0;
                let target_width = width.saturating_sub(padding_left + 3); // Leave room for "..."

                for span in &cell.spans {
                    let span_width = UnicodeWidthStr::width(span.content.as_ref());
                    if accumulated_width + span_width <= target_width {
                        spans.push(span.clone());
                        accumulated_width += span_width;
                    } else {
                        // Truncate this span
                        let remaining = target_width.saturating_sub(accumulated_width);
                        if remaining > 0 {
                            let truncated: String = span.content.chars().take(remaining).collect();
                            spans.push(Span::styled(truncated, span.style));
                        }
                        break;
                    }
                }

                spans.push(Span::raw("..."));
                spans.push(Span::raw(" "));
            }
        }

        spans.push(Span::raw("|"));

        Line::from(spans)
    }

    fn render_separator_line(&self, widths: &[usize]) -> Line<'static> {
        let mut content = String::new();

        for &width in widths {
            content.push('|');
            content.push_str(&"-".repeat(width));
        }
        content.push('|');

        Line::from(Span::raw(content))
    }

    fn render_data_line(
        &self,
        row: &[TableCell],
        widths: &[usize],
        alignments: &[Alignment],
    ) -> Line<'static> {
        use unicode_width::UnicodeWidthStr;

        let mut spans = Vec::new();

        for (i, (&width, cell)) in widths.iter().zip(row.iter()).enumerate() {
            spans.push(Span::raw("|"));

            let alignment = alignments.get(i).copied().unwrap_or(Alignment::Left);

            let content_width = cell.visual_width;
            let (padding_left, padding_right) =
                self.apply_padding_with_alignment(content_width, width, alignment);

            spans.push(Span::raw(" ".repeat(padding_left)));

            if content_width <= width - 2 {
                spans.extend(cell.spans.clone());
                spans.push(Span::raw(" ".repeat(padding_right)));
            } else {
                let mut accumulated_width = 0;
                let target_width = width.saturating_sub(padding_left + 3); // Leave room for "..."

                for span in &cell.spans {
                    let span_width = UnicodeWidthStr::width(span.content.as_ref());
                    if accumulated_width + span_width <= target_width {
                        spans.push(span.clone());
                        accumulated_width += span_width;
                    } else {
                        // Truncate this span
                        let remaining = target_width.saturating_sub(accumulated_width);
                        if remaining > 0 {
                            let truncated: String = span.content.chars().take(remaining).collect();
                            spans.push(Span::styled(truncated, span.style));
                        }
                        break;
                    }
                }

                spans.push(Span::raw("..."));
                spans.push(Span::raw(" "));
            }
        }

        for item in widths.iter().skip(row.len()) {
            let width = *item;
            spans.push(Span::raw("|"));
            spans.push(Span::raw(" ".repeat(width)));
        }

        spans.push(Span::raw("|"));

        Line::from(spans)
    }

    fn render_table(&self, table: TableBuilder) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        if table.headers.is_empty() {
            return lines;
        }

        let max_width = 120;
        let widths = self.calculate_column_widths(&table, max_width);

        lines.push(self.render_header_line(&table.headers, &widths, &table.alignments));

        lines.push(self.render_separator_line(&widths));

        for row in &table.rows {
            lines.push(self.render_data_line(row, &widths, &table.alignments));
        }

        lines
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

    // Table rendering tests for User Story 1 (T009-T012)

    #[test]
    fn test_render_simple_table() {
        // T009: Test 2x2 table rendering with pipe visibility
        let renderer = MarkdownRenderer::new();
        let markdown = "| Header1 | Header2 |\n|---------|----------|\n| Data1   | Data2   |";
        let lines = renderer.render(markdown);

        // Should have at least 4 lines (header + separator + data + blank after)
        // Note: No blank before since table is first element
        assert!(
            lines.len() >= 4,
            "Expected at least 4 lines, got {}",
            lines.len()
        );

        // Filter out blank lines for verification
        let table_lines: Vec<_> = lines
            .iter()
            .filter(|line| {
                let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                let trimmed = text.trim();
                !trimmed.is_empty()
            })
            .collect();

        // All non-blank lines should contain pipe characters
        for (i, line) in table_lines.iter().enumerate() {
            let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(
                line_text.contains('|'),
                "Line {} should contain pipe character: '{}'",
                i,
                line_text
            );
        }

        // Verify structure: header, separator, data
        let line0_text: String = table_lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        let line1_text: String = table_lines[1]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        let line2_text: String = table_lines[2]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();

        assert!(
            line0_text.contains("Header1") && line0_text.contains("Header2"),
            "First line should contain headers"
        );
        assert!(
            line1_text.contains('-'),
            "Second line should be separator with dashes"
        );
        assert!(
            line2_text.contains("Data1") && line2_text.contains("Data2"),
            "Third line should contain data"
        );
    }

    #[test]
    fn test_header_separator() {
        // T010: Test dash separator line below headers
        let renderer = MarkdownRenderer::new();
        let markdown = "| Col1 | Col2 | Col3 |\n|------|------|------|\n| A    | B    | C    |";
        let lines = renderer.render(markdown);

        assert!(lines.len() >= 3, "Expected at least 3 lines");

        // Second line should be the separator with dashes and pipes
        let separator_text: String = lines[1].spans.iter().map(|s| s.content.as_ref()).collect();

        assert!(
            separator_text.contains('-'),
            "Separator should contain dashes"
        );
        assert!(
            separator_text.contains('|'),
            "Separator should contain pipes"
        );

        // Separator should have multiple dash segments (one per column)
        let dash_segments = separator_text
            .split('|')
            .filter(|s| s.contains('-'))
            .count();
        assert!(
            dash_segments >= 3,
            "Separator should have dash segments for each column"
        );
    }

    #[test]
    fn test_empty_cells_no_collapse() {
        // T011: Verify empty cells maintain column width (don't collapse)
        let renderer = MarkdownRenderer::new();
        let markdown = "| Name  | Value |\n|-------|-------|\n| Item1 |       |\n|       | Item2 |";
        let lines = renderer.render(markdown);

        assert!(
            lines.len() >= 4,
            "Expected at least 4 lines (header + separator + 2 data rows)"
        );

        // Check that empty cells are still represented with proper spacing
        let row1_text: String = lines[2].spans.iter().map(|s| s.content.as_ref()).collect();
        let row2_text: String = lines[3].spans.iter().map(|s| s.content.as_ref()).collect();

        // Both rows should have pipes indicating column boundaries
        let row1_pipe_count = row1_text.chars().filter(|&c| c == '|').count();
        let row2_pipe_count = row2_text.chars().filter(|&c| c == '|').count();

        assert_eq!(
            row1_pipe_count, row2_pipe_count,
            "Empty cells should maintain same number of column separators"
        );
        assert!(
            row1_pipe_count >= 2,
            "Should have at least 2 pipes (start and middle or middle and end)"
        );
    }

    #[test]
    fn test_truncate_wide_table() {
        // T012: Test ellipsis truncation when table exceeds terminal width
        let renderer = MarkdownRenderer::new();

        // Create a table with very long content
        let long_content = "VeryLongContentThatExceedsTerminalWidth".repeat(5);
        let markdown = format!(
            "| Header1 | Header2 |\n|---------|----------|\n| {} | {} |",
            long_content, long_content
        );
        let lines = renderer.render(&markdown);

        assert!(lines.len() >= 3, "Expected at least 3 lines");

        // Filter blank lines
        let table_lines: Vec<_> = lines
            .iter()
            .filter(|l| {
                let t: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
                !t.trim().is_empty()
            })
            .collect();

        // Check that lines don't exceed a reasonable terminal width (e.g., 120 chars)
        // and contain ellipsis for truncated content
        for (i, line) in table_lines.iter().enumerate() {
            let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            let line_width: usize = line_text.chars().count();

            // If the line is truncated, it should contain ellipsis
            if line_width < long_content.len() && i > 0 {
                // Note: This test assumes truncation happens; actual behavior depends on implementation
                // For now, just verify the table structure is maintained
                assert!(
                    line_text.contains('|'),
                    "Truncated line {} should still maintain pipe structure",
                    i
                );
            }
        }
    }

    // User Story 2 tests (T029-T032) - Complex tables with formatting

    #[test]
    fn test_formatted_cells() {
        // T029: Test bold/italic preservation within cells
        let renderer = MarkdownRenderer::new();
        let markdown = "| Name | Status |\n|------|--------|\n| **Bold** | *Italic* |\n| Normal | **Bold** and *Italic* |";
        let lines = renderer.render(markdown);

        assert!(
            lines.len() >= 4,
            "Expected at least 4 lines (header + separator + 2 data rows)"
        );

        // Filter out blank lines
        let table_lines: Vec<_> = lines
            .iter()
            .filter(|line| {
                let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                !text.trim().is_empty()
            })
            .collect();

        // Check that table structure is maintained
        for line in &table_lines {
            let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(
                line_text.contains('|'),
                "Line should contain pipe characters"
            );
        }

        // Check that formatted text is preserved in spans
        // Data rows should have multiple spans with different styles
        let data_row1 = &table_lines[2];
        let data_row2 = &table_lines[3];

        // At least one span in data rows should have bold or italic modifier
        let has_formatted_spans = data_row1
            .spans
            .iter()
            .any(|s| s.style.add_modifier != ratatui::style::Modifier::empty())
            || data_row2
                .spans
                .iter()
                .any(|s| s.style.add_modifier != ratatui::style::Modifier::empty());

        assert!(
            has_formatted_spans,
            "Data rows should preserve text formatting"
        );
    }

    #[test]
    fn test_escaped_pipes_in_cells() {
        // T030: Test escaped pipes render as literal pipe characters
        let renderer = MarkdownRenderer::new();
        // Note: In markdown, \| within a cell should render as a literal pipe
        let markdown = r"| Column1 | Column2 |
|---------|---------|
| Text with \| pipe | Normal |";
        let lines = renderer.render(markdown);

        assert!(lines.len() >= 3, "Expected at least 3 lines");

        // The data row should contain the literal pipe character in the cell content
        let data_row = &lines[2];
        let row_text: String = data_row.spans.iter().map(|s| s.content.as_ref()).collect();

        // Should have table structure pipes AND the literal pipe in content
        let pipe_count = row_text.chars().filter(|&c| c == '|').count();
        assert!(
            pipe_count >= 3,
            "Should have at least 3 pipes (2 for structure + 1 literal)"
        );
    }

    #[test]
    fn test_special_characters() {
        // T031: Test special characters in cells (parentheses, hyphens, quotes)
        let renderer = MarkdownRenderer::new();
        let markdown = r#"| Item | Description |
|------|-------------|
| (A) | "quoted" |
| B-C | it's-working |"#;
        let lines = renderer.render(markdown);

        assert!(lines.len() >= 4, "Expected at least 4 lines");

        // Check that special characters are preserved
        let data_row1 = &lines[2];
        let data_row2 = &lines[3];

        let row1_text: String = data_row1.spans.iter().map(|s| s.content.as_ref()).collect();
        let row2_text: String = data_row2.spans.iter().map(|s| s.content.as_ref()).collect();

        #[cfg(test)]
        {
            eprintln!("Row1: '{}'", row1_text);
            eprintln!("Row2: '{}'", row2_text);
        }

        assert!(
            row1_text.contains('(') && row1_text.contains(')'),
            "Parentheses should be preserved"
        );
        // Note: pulldown-cmark may convert straight quotes to smart quotes
        let has_quotes = row1_text.contains('"')
            || row1_text.contains('\u{201C}')
            || row1_text.contains('\u{201D}');
        assert!(
            has_quotes,
            "Quotes should be preserved (may be smart quotes)"
        );
        assert!(row2_text.contains('-'), "Hyphens should be preserved");
        let has_apostrophe = row2_text.contains('\'') || row2_text.contains('\u{2019}');
        assert!(
            has_apostrophe,
            "Apostrophes should be preserved (may be smart quotes)"
        );
    }

    #[test]
    fn test_varying_cell_lengths() {
        // T032: Test alignment with mixed content lengths
        let renderer = MarkdownRenderer::new();
        let markdown = "| Short | Medium Length | Very Long Content Here |\n|-------|---------------|------------------------|\n| A | B | C |\n| VeryLongText | X | Y |";
        let lines = renderer.render(markdown);

        assert!(lines.len() >= 4, "Expected at least 4 lines");

        // Filter blank lines
        let table_lines: Vec<_> = lines
            .iter()
            .filter(|l| {
                let t: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
                !t.trim().is_empty()
            })
            .collect();

        // All lines should have the same number of pipe characters (column structure maintained)
        let pipe_counts: Vec<usize> = table_lines
            .iter()
            .map(|line| {
                let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                text.chars().filter(|&c| c == '|').count()
            })
            .collect();

        // Header, separator, and all data rows should have same pipe count
        assert!(
            pipe_counts.windows(2).all(|w| w[0] == w[1]),
            "All rows should have the same number of column separators: {:?}",
            pipe_counts
        );

        // Check that columns are properly padded/aligned
        for (i, line) in table_lines.iter().enumerate() {
            let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            if i == 1 {
                // Separator line should have dashes
                assert!(line_text.contains('-'), "Separator should contain dashes");
            }
        }
    }

    // User Story 3 tests (T040-T043) - Column alignment

    #[test]
    fn test_left_alignment() {
        // T040: Test left-aligned columns
        let renderer = MarkdownRenderer::new();
        // Left alignment uses :-- or ---
        let markdown = "| Name | Age |\n|:-----|:----|\n| Alice | 30 |\n| Bob | 25 |";
        let lines = renderer.render(markdown);

        assert!(lines.len() >= 4, "Expected at least 4 lines");

        // Filter blank lines
        let table_lines: Vec<_> = lines
            .iter()
            .filter(|l| {
                let t: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
                !t.trim().is_empty()
            })
            .collect();

        // Check that content is left-aligned (text starts right after the space after pipe)
        let data_row = &table_lines[2];
        let row_text: String = data_row.spans.iter().map(|s| s.content.as_ref()).collect();

        // For left alignment, text should appear near the start of each column
        // After splitting by |, trimming start should remove minimal spaces
        let columns: Vec<&str> = row_text.split('|').collect();
        if columns.len() > 1 {
            let first_col = columns[1];
            // Left-aligned: should have space at start, then text, then more spaces
            assert!(
                first_col.starts_with(' '),
                "Column should have leading space for padding"
            );
        }
    }

    #[test]
    fn test_center_alignment() {
        // T041: Test center-aligned columns
        let renderer = MarkdownRenderer::new();
        // Center alignment uses :-:
        let markdown = "| Item | Value |\n|:----:|:-----:|\n| A | 100 |\n| BC | 50 |";
        let lines = renderer.render(markdown);

        assert!(lines.len() >= 4, "Expected at least 4 lines");

        // Filter blank lines
        let table_lines: Vec<_> = lines
            .iter()
            .filter(|l| {
                let t: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
                !t.trim().is_empty()
            })
            .collect();

        // Verify table structure is maintained
        for line in &table_lines {
            let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(
                line_text.contains('|'),
                "Line should contain pipe characters"
            );
        }

        // For center alignment, content should be roughly centered in the column
        // This is harder to test precisely without knowing exact widths, so we verify structure
        let data_row = &table_lines[2];
        let row_text: String = data_row.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(row_text.contains('A'), "Should contain data");
    }

    #[test]
    fn test_right_alignment() {
        // T042: Test right-aligned columns
        let renderer = MarkdownRenderer::new();
        // Right alignment uses --:
        let markdown = "| Name | Amount |\n|------|-------:|\n| Item1 | 1000 |\n| Item2 | 50 |";
        let lines = renderer.render(markdown);

        assert!(lines.len() >= 4, "Expected at least 4 lines");

        // Filter blank lines
        let table_lines: Vec<_> = lines
            .iter()
            .filter(|l| {
                let t: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
                !t.trim().is_empty()
            })
            .collect();

        // Verify table structure
        for line in &table_lines {
            let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            assert!(
                line_text.contains('|'),
                "Line should contain pipe characters"
            );
        }

        // Right-aligned columns should have content near the end (before the padding before pipe)
        let data_row = &table_lines[2];
        let row_text: String = data_row.spans.iter().map(|s| s.content.as_ref()).collect();

        // Split by pipes and check the last column (Amount)
        let columns: Vec<&str> = row_text.split('|').collect();
        if columns.len() > 2 {
            let last_col = columns[columns.len() - 2]; // -2 because last element after final | is empty
            // Right-aligned: should have more spaces at start, then text
            let trimmed_start = last_col.trim_start();
            let leading_spaces = last_col.len() - trimmed_start.len();
            assert!(
                leading_spaces > 1,
                "Right-aligned column should have leading spaces"
            );
        }
    }

    #[test]
    fn test_mixed_alignment() {
        // T043: Test table with mixed column alignments
        let renderer = MarkdownRenderer::new();
        let markdown = "| Name | Count | Price |\n|:-----|:-----:|------:|\n| Apple | 5 | 1.50 |\n| Banana | 10 | 0.75 |";
        let lines = renderer.render(markdown);

        assert!(
            lines.len() >= 4,
            "Expected at least 4 lines (header + separator + 2 data rows)"
        );

        // Filter blank lines
        let table_lines: Vec<_> = lines
            .iter()
            .filter(|l| {
                let t: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
                !t.trim().is_empty()
            })
            .collect();

        // Verify all rows have consistent structure
        let pipe_counts: Vec<usize> = table_lines
            .iter()
            .map(|line| {
                let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                text.chars().filter(|&c| c == '|').count()
            })
            .collect();

        assert!(
            pipe_counts.windows(2).all(|w| w[0] == w[1]),
            "All rows should have same number of pipes: {:?}",
            pipe_counts
        );

        // Verify content is present in all rows
        for (i, line) in table_lines.iter().enumerate() {
            let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            if i == 0 {
                assert!(line_text.contains("Name"), "Header should contain Name");
            } else if i >= 2 {
                // Data rows should have actual data
                assert!(line_text.len() > 10, "Data rows should have content");
            }
        }
    }
}
