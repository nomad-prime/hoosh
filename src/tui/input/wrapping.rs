use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, PartialEq)]
pub struct WrappedLine {
    pub content: String,
    pub is_soft_wrap: bool,
}

pub struct WrappingCalculator {
    terminal_width: u16,
    wrap_indicator: char,
}

impl WrappingCalculator {
    pub fn new(terminal_width: u16) -> Self {
        Self {
            terminal_width,
            wrap_indicator: '↩',
        }
    }

    pub fn with_indicator(terminal_width: u16, wrap_indicator: char) -> Self {
        Self {
            terminal_width,
            wrap_indicator,
        }
    }

    pub fn set_terminal_width(&mut self, width: u16) {
        self.terminal_width = width;
    }

    pub fn wrap_text(&self, text: &str) -> Vec<WrappedLine> {
        let mut wrapped_lines = Vec::new();
        let width = self.terminal_width as usize;

        if width == 0 {
            return wrapped_lines;
        }

        for paragraph in text.split('\n') {
            if paragraph.is_empty() {
                wrapped_lines.push(WrappedLine {
                    content: String::new(),
                    is_soft_wrap: false,
                });
                continue;
            }

            let mut current_line = String::new();
            let mut current_width = 0;

            for word in paragraph.split_whitespace() {
                let word_width = UnicodeWidthStr::width(word);

                if word_width > width {
                    if !current_line.is_empty() {
                        wrapped_lines.push(WrappedLine {
                            content: current_line.clone(),
                            is_soft_wrap: true,
                        });
                        current_line.clear();
                        current_width = 0;
                    }

                    let mut remaining = word;
                    while !remaining.is_empty() {
                        let mut take_width = 0;
                        let mut take_chars = 0;

                        for ch in remaining.chars() {
                            let ch_str = ch.to_string();
                            let ch_width = UnicodeWidthStr::width(ch_str.as_str());
                            if take_width + ch_width > width && take_chars > 0 {
                                break;
                            }
                            take_width += ch_width;
                            take_chars += 1;
                        }

                        let (chunk, rest) = remaining.split_at(
                            remaining
                                .char_indices()
                                .nth(take_chars)
                                .map(|(i, _)| i)
                                .unwrap_or(remaining.len()),
                        );

                        wrapped_lines.push(WrappedLine {
                            content: chunk.to_string(),
                            is_soft_wrap: !rest.is_empty(),
                        });

                        remaining = rest;
                    }
                    continue;
                }

                if current_width + word_width + 1 > width && !current_line.is_empty() {
                    wrapped_lines.push(WrappedLine {
                        content: current_line.clone(),
                        is_soft_wrap: true,
                    });
                    current_line = word.to_string();
                    current_width = word_width;
                } else {
                    if !current_line.is_empty() {
                        current_line.push(' ');
                        current_width += 1;
                    }
                    current_line.push_str(word);
                    current_width += word_width;
                }
            }

            if !current_line.is_empty() || paragraph.split_whitespace().count() == 0 {
                wrapped_lines.push(WrappedLine {
                    content: current_line,
                    is_soft_wrap: false,
                });
            }
        }

        wrapped_lines
    }

    pub fn wrap_indicator(&self) -> char {
        self.wrap_indicator
    }
}
