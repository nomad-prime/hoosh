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

    // Combine ASCII art with info on the right
    let logo_color = Color::Rgb(142, 240, 204);
    let title_color = Color::Rgb(255, 255, 255);
    let info_color = Color::Rgb(150, 150, 150);

    vec![
        Line::from(vec![
            Span::styled(ascii_lines[0], Style::default().fg(logo_color)),
            Span::styled(
                format!(" {}", info_lines[0]),
                Style::default().fg(title_color),
            ),
        ]),
        Line::from(vec![
            Span::styled(ascii_lines[1], Style::default().fg(logo_color)),
            Span::styled(
                format!(" {}", info_lines[1]),
                Style::default().fg(info_color),
            ),
        ]),
        Line::from(vec![
            Span::styled(ascii_lines[2], Style::default().fg(logo_color)),
            Span::styled(
                format!(" {}", info_lines[2]),
                Style::default().fg(info_color),
            ),
        ]),
        Line::from(vec![
            Span::styled(ascii_lines[3], Style::default().fg(logo_color)),
            Span::styled(
                format!(" {}", info_lines[3]),
                Style::default().fg(info_color),
            ),
        ]),
        Line::from(vec![
            Span::styled(ascii_lines[4], Style::default().fg(logo_color)),
            Span::styled(
                format!(" {}", info_lines[4]),
                Style::default().fg(info_color),
            ),
        ]),
        Line::from(""),
    ]
}
