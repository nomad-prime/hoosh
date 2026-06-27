use crate::tui::app_state::AppState;
use crate::tui::component::Component;
use crate::tui::markdown::MarkdownRenderer;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

const FADE_CHARS: usize = 28;
const FADE_DIM: u8 = 60;
const FADE_FULL: u8 = 190;

pub fn streaming_markdown_lines(app: &AppState) -> Vec<Line<'static>> {
    let Some(text) = app.visible_streaming_text() else {
        return Vec::new();
    };
    let mut lines = fade_tail(MarkdownRenderer::new().render(text));
    lines.insert(0, Line::from(""));
    lines
}

fn fade_tail(lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    let line_lens: Vec<usize> = lines
        .iter()
        .map(|l| l.spans.iter().map(|s| s.content.chars().count()).sum())
        .collect();

    let mut suffix = 0usize;
    let mut faded = Vec::with_capacity(lines.len());
    for (line, len) in lines.into_iter().zip(line_lens).rev() {
        if suffix >= FADE_CHARS || len == 0 {
            suffix += len;
            faded.push(line);
            continue;
        }

        let mut out: Vec<(char, Style)> = Vec::new();
        let mut idx = 0usize;
        for span in &line.spans {
            for ch in span.content.chars() {
                let dist = (len - 1 - idx) + suffix;
                let style = if dist < FADE_CHARS {
                    let t = dist as f32 / FADE_CHARS as f32;
                    let v = (FADE_DIM as f32 + (FADE_FULL - FADE_DIM) as f32 * t) as u8;
                    span.style.fg(Color::Rgb(v, v, v))
                } else {
                    span.style
                };
                out.push((ch, style));
                idx += 1;
            }
        }

        faded.push(chars_to_line(out));
        suffix += len;
    }

    faded.reverse();
    faded
}

fn chars_to_line(chars: Vec<(char, Style)>) -> Line<'static> {
    let mut spans = Vec::new();
    let mut text = String::new();
    let mut current = chars.first().map(|(_, s)| *s).unwrap_or_default();

    for (ch, style) in chars {
        if style == current {
            text.push(ch);
        } else {
            spans.push(Span::styled(std::mem::take(&mut text), current));
            current = style;
            text.push(ch);
        }
    }
    if !text.is_empty() {
        spans.push(Span::styled(text, current));
    }
    Line::from(spans)
}

pub struct StreamingResponseComponent;

impl Component for StreamingResponseComponent {
    type State = AppState;

    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer) {
        let Some(line) = state.streaming_live_line(area.width) else {
            return;
        };
        Paragraph::new(fade_tail(vec![line])).render(area, buf);
    }
}
