use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::tui::palette;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn create_header_block(
    backend_name: &str,
    model_name: &str,
    working_dir: &str,
    agent_name: Option<&str>,
    trusted_project: Option<&str>,
) -> Vec<Line<'static>> {
    // ASCII art lines (left side)
    let ascii_lines = [
        " __  __     ______     ______     ______     __  __    ",
        "/\\ \\_\\ \\   /\\  __ \\   /\\  __ \\   /\\  ___\\   /\\ \\_\\ \\   ",
        "\\ \\  __ \\  \\ \\ \\/\\ \\  \\ \\ \\/\\ \\  \\ \\___  \\  \\ \\  __ \\  ",
        " \\ \\_\\ \\_\\  \\ \\_____\\  \\ \\_____\\  \\/\\_____\\  \\ \\_\\ \\_\\ ",
        "  \\/_/\\/_/   \\/_____/   \\/_____/   \\/_____/   \\/_/\\/_/ ",
    ];

    // Info lines (right side)
    let title = format!("hoosh  v{}", VERSION);
    let agent_info = if let Some(agent) = agent_name {
        format!("Agent: {}", agent)
    } else {
        "Agent: none".to_string()
    };

    let mut info_lines = vec![
        title,
        backend_name.to_string(),
        model_name.to_string(),
        agent_info,
        working_dir.to_string(),
    ];

    // Add trust indicator if project is trusted
    if trusted_project.is_some() {
        info_lines.push("🔓 Project Trusted".to_string());
    }

    // Calculate max width needed for the box
    let max_info_width = info_lines.iter().map(|s| s.len()).max().unwrap_or(0);
    let ascii_width = ascii_lines[0].len();
    let total_content_width = ascii_width + 1 + max_info_width;
    let box_width = total_content_width + 4; // 2 for left/right borders + 2 for padding

    // Combine ASCII art with info on the right
    let logo_color = palette::HEADER_LOGO;
    let title_color = palette::HEADER_TITLE;
    let info_color = palette::HEADER_INFO;
    let border_color = palette::HEADER_BORDER;
    let trust_color = palette::HEADER_TRUST;

    let mut lines = vec![
        // Top border
        Line::from(vec![Span::styled(
            format!("┌{}┐", "─".repeat(box_width - 2)),
            Style::default().fg(border_color),
        )]),
    ];

    // Content lines with borders
    let num_lines = ascii_lines.len().max(info_lines.len());
    for i in 0..num_lines {
        let ascii_text: String = if i < ascii_lines.len() {
            ascii_lines[i].to_string()
        } else {
            " ".repeat(ascii_width)
        };

        let info_text = if i < info_lines.len() {
            format!(" {}", info_lines[i])
        } else {
            String::new()
        };

        let padding_needed = max_info_width + 1 - info_text.len();
        let padding = " ".repeat(padding_needed);

        let style = if i == 0 {
            Style::default().fg(title_color)
        } else if i == info_lines.len() - 1 && trusted_project.is_some() {
            // Last line is the trust indicator
            Style::default().fg(trust_color)
        } else {
            Style::default().fg(info_color)
        };

        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(border_color)),
            Span::styled(ascii_text, Style::default().fg(logo_color)),
            Span::styled(info_text, style),
            Span::styled(padding, Style::default()),
            Span::styled(" │", Style::default().fg(border_color)),
        ]));
    }

    // Bottom border
    lines.push(Line::from(vec![Span::styled(
        format!("└{}┘", "─".repeat(box_width - 2)),
        Style::default().fg(border_color),
    )]));

    lines.push(Line::from(""));

    lines
}
