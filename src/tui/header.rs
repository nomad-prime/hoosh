use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Calculate the number of lines the header block will take up
pub fn calculate_header_height(trusted_project: Option<&str>) -> u16 {
    // ASCII art has 5 lines
    // Info lines: title, backend, model, agent, working_dir (5 lines)
    // If trusted_project is Some, add 1 more info line (6 total)
    let info_line_count = if trusted_project.is_some() { 6 } else { 5 };

    // max(5 ascii lines, info_line_count) + 2 borders + 1 blank line
    let content_lines = 5.max(info_line_count);
    content_lines + 2 + 1
}

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
    let logo_color = Color::Rgb(142, 240, 204);
    let title_color = Color::Rgb(255, 255, 255);
    let info_color = Color::Rgb(150, 150, 150);
    let border_color = Color::Rgb(100, 100, 100);
    let trust_color = Color::Rgb(255, 200, 0); // Yellow/Orange for trust indicator

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
