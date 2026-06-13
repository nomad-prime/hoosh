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
    // Braille pixel-art logo lines (left column), taller to fill the box
    let logo_lines = [
        "⣿⠀⠀⠀⠀⣿ ⣴⠟⠛⠛⠻⣦ ⣴⠟⠛⠛⠻⣦ ⣴⠟⠛⠛⠛⠓ ⣿⠀⠀⠀⠀⣿",
        "⣿⠀⠀⠀⠀⣿ ⣿⠀⠀⠀⠀⣿ ⣿⠀⠀⠀⠀⣿ ⠹⣦⣄⡀⠀⠀ ⣿⠀⠀⠀⠀⣿",
        "⣿⠛⠛⠛⠛⣿ ⣿⠀⠀⠀⠀⣿ ⣿⠀⠀⠀⠀⣿ ⠀⠈⠙⠻⣦⡀ ⣿⠛⠛⠛⠛⣿",
        "⣿⠀⠀⠀⠀⣿ ⣿⠀⠀⠀⠀⣿ ⣿⠀⠀⠀⠀⣿ ⠀⠀⠀⠀⠙⣷ ⣿⠀⠀⠀⠀⣿",
        "⣿⠀⠀⠀⠀⣿ ⠻⣦⣀⣀⣴⠟ ⠻⣦⣀⣀⣴⠟ ⠲⣦⣀⣀⣴⠟ ⣿⠀⠀⠀⠀⣿",
    ];

    // Info lines (right column)
    let title = format!("hoosh  v{}", VERSION);
    let agent_info = if let Some(agent) = agent_name {
        format!("Agent: {}", agent)
    } else {
        "Agent: none".to_string()
    };

    let mut info_lines: Vec<(String, &str)> = vec![
        (title, "title"),
        (backend_name.to_string(), "info"),
        (model_name.to_string(), "info"),
        (agent_info, "info"),
        (working_dir.to_string(), "info"),
    ];

    if trusted_project.is_some() {
        info_lines.push(("Project Trusted".to_string(), "trust"));
    }

    let logo_color = palette::HEADER_LOGO;
    let title_color = palette::HEADER_TITLE;
    let info_color = palette::HEADER_INFO;
    let trust_color = palette::HEADER_TRUST;

    // Column width (use char count for display width, not byte length)
    let logo_width = logo_lines[0].chars().count();
    let num_rows = logo_lines.len().max(info_lines.len());

    let mut lines = Vec::new();

    // ─── content rows (no borders) ───────────────────────────────
    for i in 0..num_rows {
        let logo_text: String = if i < logo_lines.len() {
            logo_lines[i].to_string()
        } else {
            " ".repeat(logo_width)
        };

        let (info_text, kind) = if i < info_lines.len() {
            let (ref s, k) = info_lines[i];
            (s.clone(), k)
        } else {
            (String::new(), "info")
        };

        let info_style = match kind {
            "title" => Style::default().fg(title_color),
            "trust" => Style::default().fg(trust_color),
            _ => Style::default().fg(info_color),
        };

        lines.push(Line::from(vec![
            Span::styled(logo_text, Style::default().fg(logo_color)),
            Span::styled("   ", Style::default()),
            Span::styled(info_text, info_style),
        ]));
    }

    lines.push(Line::from(""));

    lines
}
