use crate::tui::component::Component;
use crate::tui::state::{AppState, ToolCallStatus, continuation_line};
use crate::tui::tool_phrase::{aggregate_phrase, completed_phrase, target_basenames};
use crate::tui::{glyphs, palette};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

pub struct ActiveToolCallsComponent;

impl Component for ActiveToolCallsComponent {
    type State = AppState;

    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer) {
        if state.tools.active.is_empty() {
            if !state.pending_exploration.is_empty() {
                self.render_pending_exploration(state, area, buf);
            }
            return;
        }

        if state.tool_calls_collapsed() {
            self.render_collapsed(state, area, buf);
            return;
        }

        let mut lines = Vec::new();

        for tool_call in &state.tools.active {
            // Static glyphs are padded to three cells so the tool-name column
            // stays aligned with the animated executing rows.
            let status_indicator = match &tool_call.status {
                ToolCallStatus::Starting => Span::styled(
                    glyphs::TOOL_STARTING,
                    Style::default().fg(palette::TOOL_STATUS_STARTING),
                ),
                ToolCallStatus::AwaitingApproval => Span::styled(
                    glyphs::TOOL_AWAITING,
                    Style::default().fg(palette::TOOL_STATUS_RUNNING),
                ),
                ToolCallStatus::Executing => {
                    let sweep = glyphs::TOOL_EXECUTING_SWEEP;
                    let frame = sweep[state.animation.frame % sweep.len()];
                    Span::styled(frame, Style::default().fg(palette::TOOL_STATUS_EXECUTING))
                }
                ToolCallStatus::Completed => Span::styled(
                    glyphs::TOOL_COMPLETED,
                    Style::default().fg(palette::TOOL_STATUS_COMPLETED),
                ),
                ToolCallStatus::Error(_) => Span::styled(
                    glyphs::TOOL_ERROR,
                    Style::default().fg(palette::TOOL_STATUS_ERROR),
                ),
            };

            let timer = tool_call.elapsed_time();

            let meta_info = match tool_call.budget_pct {
                Some(pct) => format!(" {} • {:.0}% budget", timer, pct),
                None => format!(" {}", timer),
            };

            let mut spans = vec![
                status_indicator,
                Span::raw(" "),
                Span::raw(&tool_call.display_name),
                Span::styled(meta_info, Style::default().fg(palette::SUBDUED_TEXT)),
            ];

            match &tool_call.status {
                ToolCallStatus::AwaitingApproval => {
                    spans.push(Span::styled(
                        " [Awaiting Approval]",
                        Style::default()
                            .fg(palette::WARNING)
                            .add_modifier(Modifier::ITALIC),
                    ));
                }
                ToolCallStatus::Error(err) => {
                    spans.push(Span::styled(
                        format!(" [Error: {}]", err),
                        Style::default().fg(palette::DESTRUCTIVE),
                    ));
                }
                _ => {}
            }

            lines.push(Line::from(spans));

            if let Some(summary) = &tool_call.result_summary {
                lines.push(continuation_line(summary.clone()));
            }
        }

        let paragraph = Paragraph::new(lines);
        paragraph.render(area, buf);
    }
}

impl ActiveToolCallsComponent {
    fn render_pending_exploration(&self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let calls = &state.pending_exploration;
        let sweep = glyphs::TOOL_EXECUTING_SWEEP;
        let frame = sweep[state.animation.frame % sweep.len()];

        let summary = Line::from(vec![
            Span::styled(frame, Style::default().fg(palette::TOOL_STATUS_EXECUTING)),
            Span::raw(" "),
            Span::raw(format!("{}… ", completed_phrase(calls))),
        ]);

        let targets = truncate_targets(&target_basenames(calls), area.width as usize);
        let detail = continuation_line(targets);

        Paragraph::new(vec![summary, detail, Line::from("")]).render(area, buf);
    }

    fn render_collapsed(&self, state: &AppState, area: Rect, buf: &mut Buffer) {
        let calls = &state.tools.active;
        let all_done = calls.iter().all(|tc| {
            matches!(
                tc.status,
                ToolCallStatus::Completed | ToolCallStatus::Error(_)
            )
        });

        let indicator = if all_done {
            Span::styled(
                glyphs::TOOL_COMPLETED,
                Style::default().fg(palette::TOOL_STATUS_COMPLETED),
            )
        } else {
            let sweep = glyphs::TOOL_EXECUTING_SWEEP;
            let frame = sweep[state.animation.frame % sweep.len()];
            Span::styled(frame, Style::default().fg(palette::TOOL_STATUS_EXECUTING))
        };

        let timer = calls
            .first()
            .map(|tc| tc.elapsed_time())
            .unwrap_or_default();

        let phrase = if all_done {
            format!("{} ", completed_phrase(calls))
        } else {
            format!("{}… ", aggregate_phrase(calls))
        };

        let summary = Line::from(vec![
            indicator,
            Span::raw(" "),
            Span::raw(phrase),
            Span::styled(
                format!("({})", timer),
                Style::default().fg(palette::SUBDUED_TEXT),
            ),
            Span::styled(
                " (ctrl+o to expand)",
                Style::default().fg(palette::DIMMED_TEXT),
            ),
        ]);

        let targets = truncate_targets(&target_basenames(calls), area.width as usize);
        let detail = continuation_line(targets);

        Paragraph::new(vec![summary, detail]).render(area, buf);
    }
}

fn truncate_targets(basenames: &[String], max_width: usize) -> String {
    const PREFIX_WIDTH: usize = 4;
    let budget = max_width.saturating_sub(PREFIX_WIDTH).max(8);
    let joined = basenames.join(", ");
    if joined.chars().count() <= budget {
        return joined;
    }

    let mut shown = Vec::new();
    let mut width = 0;
    for (i, name) in basenames.iter().enumerate() {
        let sep = if i == 0 { 0 } else { 2 };
        if width + sep + name.chars().count() > budget {
            break;
        }
        width += sep + name.chars().count();
        shown.push(name.clone());
    }

    let remaining = basenames.len() - shown.len();
    if shown.is_empty() {
        return format!("{} files", basenames.len());
    }
    format!("{} +{} more", shown.join(", "), remaining)
}
