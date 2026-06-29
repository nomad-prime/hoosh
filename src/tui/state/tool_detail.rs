use crate::tui::palette;
use ratatui::{
    style::Style,
    text::{Line, Span},
};

#[derive(Clone, Debug)]
pub struct SubagentStepSummary {
    pub step_number: usize,
    pub action_type: String,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct BashOutputLine {
    pub line_number: usize,
    pub content: String,
    pub stream_type: String, // "stdout" or "stderr"
}

/// Live detail rows shown beneath a tool call: a trailing window over a growing
/// stream, with the boundary marked by `⎿` and the hidden remainder summarized.
/// Both subagent steps and bash output are this shape — they differ only in row
/// styling, window size, and where the overflow marker sits.
pub trait ToolDetail {
    fn detail_lines(&self, expanded: bool) -> Vec<Line<'static>>;
}

#[derive(Clone, Debug, Default)]
pub struct SubagentDetail {
    pub steps: Vec<SubagentStepSummary>,
    pub total_tool_uses: Option<usize>,
    pub total_tokens: Option<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct BashDetail {
    pub lines: Vec<BashOutputLine>,
}

const MAX_SUBAGENT_STEPS: usize = 5;
const BASH_COLLAPSED_LINES: usize = 5;
const BASH_EXPANDED_LINES: usize = 30;
const BASH_LINE_WIDTH: usize = 80;

impl ToolDetail for SubagentDetail {
    fn detail_lines(&self, _expanded: bool) -> Vec<Line<'static>> {
        let style = Style::default().fg(palette::SUBDUED_TEXT);
        let rows = self
            .steps
            .iter()
            .map(|step| DetailRow {
                text: step.description.clone(),
                style,
            })
            .collect();

        trailing_block(rows, MAX_SUBAGENT_STEPS, OverflowPos::Bottom, |_hidden| {
            DetailRow {
                text: "...".into(),
                style,
            }
        })
    }
}

impl ToolDetail for BashDetail {
    fn detail_lines(&self, expanded: bool) -> Vec<Line<'static>> {
        let max = if expanded {
            BASH_EXPANDED_LINES
        } else {
            BASH_COLLAPSED_LINES
        };

        let rows = self
            .lines
            .iter()
            .map(|line| {
                let style = if line.stream_type == "stderr" {
                    Style::default().fg(palette::DESTRUCTIVE)
                } else {
                    Style::default().fg(palette::SECONDARY_TEXT)
                };
                DetailRow {
                    text: truncate(&line.content, BASH_LINE_WIDTH),
                    style,
                }
            })
            .collect();

        // Overflow sits on top, like a terminal scrolled to its tail, and carries
        // the ctrl+o toggle hint.
        trailing_block(rows, max, OverflowPos::Top, move |hidden| {
            let hint = if expanded {
                format!("... (+{hidden} lines · ctrl+o to collapse)")
            } else {
                format!("... (+{hidden} lines · ctrl+o to expand)")
            };
            DetailRow {
                text: hint,
                style: Style::default().fg(palette::SECONDARY_TEXT),
            }
        })
    }
}

pub struct DetailRow {
    pub text: String,
    pub style: Style,
}

pub enum OverflowPos {
    Top,
    Bottom,
}

const BOUNDARY_PREFIX: &str = "  ⎿ ";
const INDENT_PREFIX: &str = "    ";

fn trailing_block(
    rows: Vec<DetailRow>,
    max: usize,
    overflow_pos: OverflowPos,
    overflow: impl FnOnce(usize) -> DetailRow,
) -> Vec<Line<'static>> {
    let max = max.max(1);
    let total = rows.len();
    let shown = total.min(max);
    let hidden = total - shown;

    let overflow_row = (hidden > 0).then(|| overflow(hidden));
    let mut out = Vec::new();
    let mut boundary_used = false;

    match (&overflow_pos, overflow_row) {
        (OverflowPos::Top, Some(row)) => {
            emit(&mut out, row, &mut boundary_used);
            for row in rows.into_iter().skip(total - shown) {
                emit(&mut out, row, &mut boundary_used);
            }
        }
        (OverflowPos::Bottom, Some(row)) => {
            for r in rows.into_iter().skip(total - shown) {
                emit(&mut out, r, &mut boundary_used);
            }
            emit(&mut out, row, &mut boundary_used);
        }
        (_, None) => {
            for row in rows.into_iter().skip(total - shown) {
                emit(&mut out, row, &mut boundary_used);
            }
        }
    }

    out
}

fn emit(out: &mut Vec<Line<'static>>, row: DetailRow, boundary_used: &mut bool) {
    let prefix = if *boundary_used {
        INDENT_PREFIX
    } else {
        BOUNDARY_PREFIX
    };
    *boundary_used = true;
    out.push(Line::from(vec![
        Span::styled(prefix, row.style),
        Span::styled(row.text, row.style),
    ]));
}

fn truncate(content: &str, max: usize) -> String {
    if content.chars().count() > max {
        let head: String = content.chars().take(max.saturating_sub(3)).collect();
        format!("{head}...")
    } else {
        content.to_string()
    }
}

#[cfg(test)]
#[path = "tool_detail_tests.rs"]
mod tests;
