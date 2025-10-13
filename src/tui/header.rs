use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

pub fn create_header_block(backend_name: &str, agent_name: Option<&str>) -> Vec<Line<'static>> {
    let backend_info = format!("Backend: {}", backend_name);
    let agent_info = if let Some(name) = agent_name {
        format!("Agent: {}", name)
    } else {
        "Agent: none".to_string()
    };

    vec![
        Line::from(vec![Span::styled(
            " __  __     ______     ______     ______     __  __    ",
            Style::default().fg(Color::Rgb(142, 240, 204)),
        )]),
        Line::from(vec![Span::styled(
            "/\\ \\_\\ \\   /\\  __ \\   /\\  __ \\   /\\  ___\\   /\\ \\_\\ \\   ",
            Style::default().fg(Color::Rgb(142, 240, 204)),
        )]),
        Line::from(vec![Span::styled(
            "\\ \\  __ \\  \\ \\ \\/\\ \\  \\ \\ \\/\\ \\  \\ \\___  \\  \\ \\  __ \\  ",
            Style::default().fg(Color::Rgb(142, 240, 204)),
        )]),
        Line::from(vec![Span::styled(
            " \\ \\_\\ \\_\\  \\ \\_____\\  \\ \\_____\\  \\/\\_____\\  \\ \\_\\ \\_\\ ",
            Style::default().fg(Color::Rgb(142, 240, 204)),
        )]),
        Line::from(vec![Span::styled(
            "  \\/_/\\/_/   \\/_____/   \\/_____/   \\/_____/   \\/_/\\/_/ ",
            Style::default().fg(Color::Rgb(142, 240, 204)),
        )]),
        Line::from(vec![Span::styled(
            "                                                       ",
            Style::default().fg(Color::Rgb(142, 240, 204)),
        )]),
        Line::from(vec![Span::styled(
            backend_info.clone(),
            Style::default().fg(Color::Rgb(150, 150, 150)),
        )]),
        Line::from(vec![Span::styled(
            agent_info.clone(),
            Style::default().fg(Color::Rgb(150, 150, 150)),
        )]),
        Line::from(""),
    ]
}
