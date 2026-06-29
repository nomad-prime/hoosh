use crate::tui::component::Component;
use crate::tui::palette;
use crate::tui::state::AppState;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

/// Trailing lines of live bash output shown when collapsed (default) and when
/// the user has expanded with `ctrl+o`.
const COLLAPSED_LINES: usize = 5;
const EXPANDED_LINES: usize = 30;

pub struct BashResultsComponent;

impl Component for BashResultsComponent {
    type State = AppState;

    fn render(&self, state: &Self::State, area: Rect, buf: &mut Buffer) {
        // Only render if there are active bash streaming tasks
        let bash_tasks: Vec<_> = state
            .tools
            .active
            .iter()
            .filter(|tc| tc.is_bash_streaming)
            .collect();

        if bash_tasks.is_empty() {
            return;
        }

        let mut lines = Vec::new();
        let max_lines = if state.tools.expanded {
            EXPANDED_LINES
        } else {
            COLLAPSED_LINES
        };

        for tool_call in bash_tasks {
            if tool_call.bash_output_lines.is_empty() {
                continue;
            }

            let total_lines = tool_call.bash_output_lines.len();
            let has_more = total_lines > max_lines;

            // Show ellipsis at the top if there are more lines (like terminal
            // scrolling down), carrying the ctrl+o toggle hint.
            if has_more {
                let hidden = total_lines - max_lines;
                let hint = if state.tools.expanded {
                    format!("... (+{hidden} lines · ctrl+o to collapse)")
                } else {
                    format!("... (+{hidden} lines · ctrl+o to expand)")
                };
                lines.push(Line::from(vec![
                    Span::styled("  ⎿ ", Style::default().fg(palette::SECONDARY_TEXT)),
                    Span::styled(hint, Style::default().fg(palette::SECONDARY_TEXT)),
                ]));
            }

            // Get the last N lines
            let start = total_lines.saturating_sub(max_lines);
            let recent_lines = &tool_call.bash_output_lines[start..];

            // Show each recent line
            for (i, line) in recent_lines.iter().enumerate() {
                let (prefix_text, style) = if line.stream_type == "stderr" {
                    if i == 0 && !has_more {
                        ("  ⎿  ", Style::default().fg(palette::DESTRUCTIVE))
                    } else {
                        ("     ", Style::default().fg(palette::DESTRUCTIVE))
                    }
                } else if i == 0 && !has_more {
                    ("  ⎿ ", Style::default().fg(palette::SECONDARY_TEXT))
                } else {
                    ("    ", Style::default().fg(palette::SECONDARY_TEXT))
                };

                // Truncate long lines on char boundaries to avoid splitting
                // multi-byte UTF-8 sequences.
                let content = if line.content.chars().count() > 80 {
                    let truncated: String = line.content.chars().take(77).collect();
                    format!("{}...", truncated)
                } else {
                    line.content.clone()
                };

                lines.push(Line::from(vec![
                    Span::styled(prefix_text, style),
                    Span::styled(content, style),
                ]));
            }
        }

        if !lines.is_empty() {
            let paragraph = Paragraph::new(lines);
            paragraph.render(area, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{ToolRender, phrasing};
    use crate::tui::state::{ActiveToolCall, BashOutputLine, ToolCallStatus};
    use ratatui::layout::Rect;
    use std::time::Instant;

    fn bash_call(num_lines: usize) -> ActiveToolCall {
        let bash_output_lines = (0..num_lines)
            .map(|i| BashOutputLine {
                line_number: i + 1,
                content: format!("line {i}"),
                stream_type: "stdout".into(),
            })
            .collect();
        ActiveToolCall {
            tool_call_id: "id".into(),
            display_name: "Bash(cargo build)".into(),
            render: ToolRender::Standard,
            phrasing: phrasing::RUN,
            status: ToolCallStatus::Executing,
            preview: None,
            result_summary: None,
            subagent_steps: Vec::new(),
            is_subagent_task: false,
            bash_output_lines,
            is_bash_streaming: true,
            start_time: Instant::now(),
            budget_pct: None,
            total_tool_uses: None,
            total_tokens: None,
        }
    }

    fn render_to_string(state: &AppState, height: u16) -> String {
        let area = Rect::new(0, 0, 80, height);
        let mut buf = Buffer::empty(area);
        BashResultsComponent.render(state, area, &mut buf);
        let mut out = String::new();
        for y in 0..height {
            for x in 0..80 {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn collapsed_shows_five_lines_with_expand_hint() {
        let mut app = AppState::new();
        app.tools.active = vec![bash_call(8)];
        app.tools.expanded = false;

        let out = render_to_string(&app, 10);
        assert!(
            out.contains("ctrl+o to expand"),
            "missing expand hint:\n{out}"
        );
        assert!(out.contains("(+3 lines"), "wrong hidden count:\n{out}");
        // Newest 5 of 8 are lines 3..=7; line 2 must be hidden.
        assert!(out.contains("line 7"));
        assert!(!out.contains("line 1\n") && !out.contains("line 2\n"));
    }

    #[test]
    fn expanded_shows_more_lines_with_collapse_hint() {
        let mut app = AppState::new();
        app.tools.active = vec![bash_call(35)];
        app.tools.expanded = true;

        let out = render_to_string(&app, 40);
        assert!(
            out.contains("ctrl+o to collapse"),
            "missing collapse hint:\n{out}"
        );
        assert!(out.contains("(+5 lines"), "wrong hidden count:\n{out}");
    }

    #[test]
    fn no_hint_when_output_fits() {
        let mut app = AppState::new();
        app.tools.active = vec![bash_call(3)];
        app.tools.expanded = false;

        let out = render_to_string(&app, 10);
        assert!(!out.contains("ctrl+o"), "unexpected hint:\n{out}");
    }
}
