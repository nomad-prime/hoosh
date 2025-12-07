use ratatui::style::Color;

pub mod palette {
    use super::*;

    // Primary colors
    pub const PRIMARY_BORDER: Color = Color::Cyan;
    pub const SELECTED_BG: Color = Color::Cyan;
    pub const SELECTED_FG: Color = Color::Black;
    pub const DIALOG_BG: Color = Color::Black;

    // Semantic colors
    pub const DESTRUCTIVE: Color = Color::Red;
    pub const WARNING: Color = Color::Yellow;
    pub const SUCCESS: Color = Color::Green;
    pub const INFO: Color = Color::Cyan;

    // Text colors
    pub const PRIMARY_TEXT: Color = Color::White;
    pub const SECONDARY_TEXT: Color = Color::Gray;
    pub const DIMMED_TEXT: Color = Color::DarkGray;
    pub const SUBDUED_TEXT: Color = Color::Rgb(100, 100, 100);

    // Markdown specific colors
    pub const MARKDOWN_HEADING: Color = Color::Magenta;
    pub const MARKDOWN_LINK: Color = Color::Blue;
    pub const MARKDOWN_CODE_FG: Color = Color::Cyan;
    pub const MARKDOWN_CODE_BG: Color = Color::Rgb(40, 44, 52);
    pub const MARKDOWN_QUOTE: Color = Color::DarkGray;
    pub const MARKDOWN_HTML: Color = Color::Gray;
    pub const MARKDOWN_TASK_MARKER: Color = Color::Green;
    pub const MARKDOWN_RULE: Color = Color::DarkGray;

    // Header colors (RGB for custom styling)
    pub const HEADER_LOGO: Color = Color::Rgb(142, 240, 204);
    pub const HEADER_TITLE: Color = Color::Rgb(255, 255, 255);
    pub const HEADER_INFO: Color = Color::Rgb(150, 150, 150);
    pub const HEADER_BORDER: Color = Color::Rgb(100, 100, 100);
    pub const HEADER_TRUST: Color = Color::Rgb(255, 200, 0);

    // Status colors
    pub const STATUS_IDLE: Color = Color::Rgb(142, 240, 204);
    pub const STATUS_PROCESSING: Color = Color::Rgb(142, 240, 204);
    pub const STATUS_WAITING: Color = Color::Yellow;
    pub const STATUS_TODOS: Color = Color::Yellow;

    // Tool call status colors
    pub const TOOL_STATUS_STARTING: Color = Color::Gray;
    pub const TOOL_STATUS_RUNNING: Color = Color::Yellow;
    pub const TOOL_STATUS_EXECUTING: Color = Color::Cyan;
    pub const TOOL_STATUS_COMPLETED: Color = Color::Green;
    pub const TOOL_STATUS_ERROR: Color = Color::Red;

    pub const PLACEHOLDER: Color = Color::Gray;
}
