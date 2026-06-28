use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use crate::tui::palette;

const DEFAULT_TABLE_WIDTH: usize = 120;

fn middle_elide(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    if max <= 1 {
        return "…".to_string();
    }
    let keep = max - 1;
    let head = keep.div_ceil(2);
    let tail = keep - head;
    let head_str: String = chars[..head].iter().collect();
    let tail_str: String = chars[chars.len() - tail..].iter().collect();
    format!("{head_str}…{tail_str}")
}

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
        self.render_with_indent(markdown, "  ", DEFAULT_TABLE_WIDTH)
    }

    pub fn render_with_indent(
        &self,
        markdown: &str,
        indent: &str,
        max_table_width: usize,
    ) -> Vec<Line<'static>> {
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
                            let table_lines = self.render_table(table, max_table_width);
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
                    let span = Span::styled(format!("`{}`", code), style);
                    if let Some(ref mut table) = current_table {
                        table.current_cell.add_span(span);
                    } else {
                        current_line_spans.push(span);
                    }
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

        const MIN_COL: usize = 5;
        let mut available = max_width.saturating_sub(borders_width);
        let mut order: Vec<usize> = (0..col_count).collect();
        order.sort_by_key(|&i| widths[i]);

        let mut remaining_cols = col_count;
        for &i in &order {
            let fair = (available / remaining_cols).max(MIN_COL);
            if widths[i] > fair {
                widths[i] = fair;
            }
            available = available.saturating_sub(widths[i]);
            remaining_cols -= 1;
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

    fn border_line(&self, widths: &[usize], left: char, mid: char, right: char) -> Line<'static> {
        let mut content = String::new();
        content.push(left);
        for (i, &width) in widths.iter().enumerate() {
            if i > 0 {
                content.push(mid);
            }
            content.push_str(&"─".repeat(width));
        }
        content.push(right);
        Line::from(Span::styled(
            content,
            Style::default().fg(palette::MARKDOWN_RULE),
        ))
    }

    fn render_row(
        &self,
        cells: &[TableCell],
        widths: &[usize],
        alignments: &[Alignment],
    ) -> Line<'static> {
        use unicode_width::UnicodeWidthStr;

        let bar = || Span::styled("│", Style::default().fg(palette::MARKDOWN_RULE));
        let mut spans = vec![bar()];

        for (i, &width) in widths.iter().enumerate() {
            let alignment = alignments.get(i).copied().unwrap_or(Alignment::Left);
            let content_budget = width.saturating_sub(2);

            let fitted = cells
                .get(i)
                .map(|cell| self.fit_cell(cell, content_budget))
                .unwrap_or_default();
            let fitted_width: usize = fitted
                .iter()
                .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
                .sum();

            let (padding_left, padding_right) =
                self.apply_padding_with_alignment(fitted_width, width, alignment);

            spans.push(Span::raw(" ".repeat(padding_left)));
            spans.extend(fitted);
            spans.push(Span::raw(" ".repeat(padding_right)));
            spans.push(bar());
        }

        Line::from(spans)
    }

    fn fit_cell(&self, cell: &TableCell, budget: usize) -> Vec<Span<'static>> {
        use unicode_width::UnicodeWidthStr;

        if cell.visual_width <= budget {
            return cell.spans.clone();
        }

        let joined: String = cell.spans.iter().map(|s| s.content.as_ref()).collect();
        if joined.contains('/')
            && let Some(span) = cell.spans.first()
            && cell.spans.len() == 1
        {
            return vec![Span::styled(middle_elide(&joined, budget), span.style)];
        }

        let mut out = Vec::new();
        let target = budget.saturating_sub(1);
        let mut acc = 0;
        for span in &cell.spans {
            let w = UnicodeWidthStr::width(span.content.as_ref());
            if acc + w <= target {
                out.push(span.clone());
                acc += w;
            } else {
                let remaining = target.saturating_sub(acc);
                if remaining > 0 {
                    let truncated: String = span.content.chars().take(remaining).collect();
                    out.push(Span::styled(truncated, span.style));
                }
                break;
            }
        }
        out.push(Span::raw("…"));
        out
    }

    fn render_table(&self, table: TableBuilder, max_width: usize) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        if table.headers.is_empty() {
            return lines;
        }

        let widths = self.calculate_column_widths(&table, max_width);

        lines.push(self.border_line(&widths, '┌', '┬', '┐'));
        lines.push(self.render_row(&table.headers, &widths, &table.alignments));
        lines.push(self.border_line(&widths, '├', '┼', '┤'));
        for row in &table.rows {
            lines.push(self.render_row(row, &widths, &table.alignments));
        }
        lines.push(self.border_line(&widths, '└', '┴', '┘'));

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

    fn line_text(line: &Line<'static>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn test_render_simple_table() {
        let renderer = MarkdownRenderer::new();
        let markdown = "| Header1 | Header2 |\n|---------|----------|\n| Data1   | Data2   |";
        let texts: Vec<String> = renderer.render(markdown).iter().map(line_text).collect();
        let starts = |t: &String, c: char| t.trim_start().starts_with(c);

        assert!(texts.iter().any(|t| starts(t, '┌')), "top border");
        assert!(texts.iter().any(|t| starts(t, '├')), "header rule");
        assert!(texts.iter().any(|t| starts(t, '└')), "bottom border");
        assert!(
            texts
                .iter()
                .any(|t| t.contains("Header1") && t.contains("Header2")),
            "header row"
        );
        assert!(
            texts
                .iter()
                .any(|t| t.contains("Data1") && t.contains("Data2")),
            "data row"
        );
    }

    #[test]
    fn test_inline_code_stays_in_table_cell() {
        let renderer = MarkdownRenderer::new();
        let markdown = "| Doc | Repo |\n|-----|------|\n| `a/b.md` | `peyk` |";

        let data_line = renderer
            .render(markdown)
            .iter()
            .map(line_text)
            .find(|t| t.contains("a/b.md"))
            .expect("data row rendered");

        assert!(
            data_line.contains('│') && data_line.contains("peyk"),
            "inline-code cells must render inside the table row, got: {data_line}"
        );
    }

    #[test]
    fn test_header_separator() {
        let renderer = MarkdownRenderer::new();
        let markdown = "| Col1 | Col2 | Col3 |\n|------|------|------|\n| A    | B    | C    |";
        let texts: Vec<String> = renderer.render(markdown).iter().map(line_text).collect();

        let rule = texts
            .iter()
            .find(|t| t.trim_start().starts_with('├'))
            .expect("header rule line");
        assert!(rule.contains('─'), "rule should contain horizontal bars");
        assert_eq!(
            rule.matches('┼').count(),
            2,
            "three columns yield two interior junctions"
        );
    }

    #[test]
    fn test_empty_cells_no_collapse() {
        let renderer = MarkdownRenderer::new();
        let markdown = "| Name  | Value |\n|-------|-------|\n| Item1 |       |\n|       | Item2 |";
        let texts: Vec<String> = renderer.render(markdown).iter().map(line_text).collect();

        let row1 = texts.iter().find(|t| t.contains("Item1")).unwrap();
        let row2 = texts.iter().find(|t| t.contains("Item2")).unwrap();

        let bars = |t: &str| t.chars().filter(|&c| c == '│').count();
        assert_eq!(
            bars(row1),
            bars(row2),
            "empty cells keep the same column separators"
        );
        assert!(bars(row1) >= 3, "two columns yield three bars");
    }

    fn row_lines(lines: &[Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .map(line_text)
            .filter(|t| t.trim_start().starts_with('│'))
            .collect()
    }

    #[test]
    fn test_truncate_wide_table() {
        let renderer = MarkdownRenderer::new();
        let long = "VeryLongContentThatExceedsTerminalWidth".repeat(5);
        let markdown = format!("| H1 | H2 |\n|----|----|\n| {long} | {long} |");
        let texts: Vec<String> = renderer.render(&markdown).iter().map(line_text).collect();

        assert!(
            texts.iter().all(|t| t.chars().count() <= 122),
            "rows stay within the table width budget"
        );
        assert!(
            row_lines(&renderer.render(&markdown))
                .iter()
                .any(|t| t.contains('…')),
            "overlong cells are elided"
        );
    }

    #[test]
    fn test_path_cells_middle_elide() {
        let renderer = MarkdownRenderer::new();
        let a = format!("src/{}alpha/00-overview.md", "deep/".repeat(30));
        let b = format!("src/{}beta/01-backend.md", "deep/".repeat(30));
        let markdown = format!("| Doc |\n|-----|\n| `{a}` |\n| `{b}` |");

        let rows = row_lines(&renderer.render(&markdown));
        let elided: Vec<&String> = rows.iter().filter(|t| t.contains('…')).collect();
        assert_eq!(elided.len(), 2, "both path rows elide: {rows:?}");
        assert!(elided.iter().any(|t| t.contains("overview.md")));
        assert!(elided.iter().any(|t| t.contains("backend.md")));
    }

    #[test]
    fn test_formatted_cells_preserve_style() {
        let renderer = MarkdownRenderer::new();
        let markdown = "| Name | Status |\n|------|--------|\n| **Bold** | *Italic* |";
        let lines = renderer.render(markdown);

        let row = lines
            .iter()
            .find(|l| line_text(l).contains("Bold"))
            .expect("data row");
        assert!(
            row.spans
                .iter()
                .any(|s| s.style.add_modifier != ratatui::style::Modifier::empty()),
            "data rows preserve bold/italic"
        );
    }

    #[test]
    fn test_consistent_column_separators() {
        let renderer = MarkdownRenderer::new();
        let markdown = "| Short | Medium | Long Header |\n|-------|--------|-------------|\n| A | B | C |\n| Longer | X | Y |";
        let bars: Vec<usize> = row_lines(&renderer.render(markdown))
            .iter()
            .map(|t| t.chars().filter(|&c| c == '│').count())
            .collect();

        assert!(!bars.is_empty());
        assert!(
            bars.windows(2).all(|w| w[0] == w[1]),
            "every row has the same column bars: {bars:?}"
        );
    }

    #[test]
    fn test_left_alignment_pads_trailing() {
        let renderer = MarkdownRenderer::new();
        let markdown = "| Name | Age |\n|:-----|:----|\n| Alice | 30 |";
        let row = row_lines(&renderer.render(markdown))
            .into_iter()
            .find(|t| t.contains("Alice"))
            .expect("data row");
        let first_col = row.split('│').nth(1).unwrap();
        assert!(first_col.starts_with(' '), "left pad before content");
    }

    #[test]
    fn test_right_alignment_pads_leading() {
        let renderer = MarkdownRenderer::new();
        let markdown = "| Name | Amount |\n|------|-------:|\n| Item1 | 1000 |";
        let row = row_lines(&renderer.render(markdown))
            .into_iter()
            .find(|t| t.contains("1000"))
            .expect("data row");
        let amount_col = row.split('│').nth(2).unwrap();
        let leading = amount_col.len() - amount_col.trim_start().len();
        assert!(leading > 1, "right-aligned column has leading padding");
    }
}
