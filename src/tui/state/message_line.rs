use ratatui::text::Line;

#[derive(Clone)]
pub enum MessageLine {
    Plain(String),
    Styled(Line<'static>),
    Markdown(String),
    Thinking(String),
}
