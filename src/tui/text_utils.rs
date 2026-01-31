use regex::Regex;
use std::sync::OnceLock;

/// Strips ANSI escape codes from text
///
/// This is useful when tool previews contain colored output (from the `colored` crate)
/// that needs to be displayed in a TUI context where ANSI codes aren't interpreted.
pub fn strip_ansi_codes(text: &str) -> String {
    static ANSI_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = ANSI_REGEX
        .get_or_init(|| Regex::new(r"\x1b\[[0-9;]*m").expect("Failed to compile ANSI regex"));
    regex.replace_all(text, "").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_codes_plain_text() {
        let text = "Hello, world!";
        assert_eq!(strip_ansi_codes(text), "Hello, world!");
    }

    #[test]
    fn test_strip_ansi_codes_with_color() {
        let text = "\x1b[1;36mCyan text\x1b[0m";
        assert_eq!(strip_ansi_codes(text), "Cyan text");
    }

    #[test]
    fn test_strip_ansi_codes_multiple_colors() {
        let text = "\x1b[31mRed\x1b[0m and \x1b[32mGreen\x1b[0m";
        assert_eq!(strip_ansi_codes(text), "Red and Green");
    }

    #[test]
    fn test_strip_ansi_codes_complex() {
        let text =
            "\x1b[1;36mCreating new file: test.txt\x1b[0m\n\x1b[33mSize: 2 lines, 63 bytes\x1b[0m";
        assert_eq!(
            strip_ansi_codes(text),
            "Creating new file: test.txt\nSize: 2 lines, 63 bytes"
        );
    }

    #[test]
    fn test_strip_ansi_codes_empty() {
        assert_eq!(strip_ansi_codes(""), "");
    }
}
