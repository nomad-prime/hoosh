use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn create_header_block(
    backend_name: &str,
    model_name: &str,
    working_dir: &str,
    agent_name: Option<&str>,
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

    let info_lines = [
        title,
        backend_name.to_string(),
        model_name.to_string(),
        agent_info,
        working_dir.to_string(),
    ];

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

    let mut lines = vec![
        // Top border
        Line::from(vec![Span::styled(
            format!("┌{}┐", "─".repeat(box_width - 2)),
            Style::default().fg(border_color),
        )]),
    ];

    // Content lines with borders
    for i in 0..5 {
        let info_text = format!(" {}", info_lines[i]);
        let padding_needed = max_info_width + 1 - info_text.len();
        let padding = " ".repeat(padding_needed);

        let style = if i == 0 {
            Style::default().fg(title_color)
        } else {
            Style::default().fg(info_color)
        };

        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(border_color)),
            Span::styled(ascii_lines[i], Style::default().fg(logo_color)),
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
