use std::ops::Range;
use textwrap::Options;

/// Wrap text and return byte ranges into the original text.
/// Used by TextArea for efficient wrapping without copying strings.
pub fn wrap_ranges(text: &str, options: Options) -> Vec<Range<usize>> {
    if text.is_empty() {
        return vec![];
    }

    let mut ranges = Vec::new();
    let mut current_pos = 0;

    for paragraph in text.split_inclusive('\n') {
        if paragraph.is_empty() {
            continue;
        }

        let para_start = current_pos;
        let para_end = para_start + paragraph.len();

        // Handle newline-only paragraph
        if paragraph == "\n" {
            ranges.push(para_start..para_end);
            current_pos = para_end;
            continue;
        }

        // Remove trailing newline for wrapping, add back later
        let (para_text, has_newline) = if let Some(stripped) = paragraph.strip_suffix('\n') {
            (stripped, true)
        } else {
            (paragraph, false)
        };

        if para_text.is_empty() {
            ranges.push(para_start..para_end);
            current_pos = para_end;
            continue;
        }

        // Wrap the paragraph
        let wrapped = textwrap::wrap(para_text, options.clone());

        if wrapped.is_empty() {
            // Edge case: empty result, push the whole paragraph
            ranges.push(para_start..para_end);
            current_pos = para_end;
            continue;
        }

        // Convert wrapped lines back to byte ranges by finding them in the original text
        let mut search_start = 0; // Offset within para_text
        for (i, line) in wrapped.iter().enumerate() {
            let line_str = line.as_ref().trim_end(); // Remove trailing spaces that textwrap might have kept

            // Find this wrapped line in the original text
            if let Some(found_pos) = para_text[search_start..].find(line_str) {
                let line_start_in_para = search_start + found_pos;
                let line_end_in_para = line_start_in_para + line_str.len();

                // Convert to absolute positions
                let abs_start = para_start + line_start_in_para;
                let abs_end = para_start + line_end_in_para;

                // For the last wrapped line, include the newline if present
                let range_end = if i == wrapped.len() - 1 && has_newline {
                    abs_end + 1
                } else {
                    abs_end
                };

                ranges.push(abs_start..range_end);

                // Move search position past this line and any trailing whitespace
                search_start = line_end_in_para;
                // Skip whitespace for next line search
                while search_start < para_text.len() {
                    let ch = para_text[search_start..].chars().next();
                    if let Some(c) = ch {
                        if c.is_whitespace() && c != '\n' {
                            search_start += c.len_utf8();
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            } else {
                // Fallback: couldn't find the line, just use lengths
                let abs_start = para_start + search_start;
                let abs_end = abs_start + line_str.len();
                ranges.push(abs_start..abs_end);
                search_start += line_str.len();
            }
        }

        current_pos = para_end;
    }

    // Ensure we have at least one range
    if ranges.is_empty() {
        ranges.push(0..text.len());
    }

    ranges
}
