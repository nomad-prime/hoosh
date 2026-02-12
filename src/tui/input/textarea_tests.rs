use super::TextArea;

fn textarea_with(text: &str) -> TextArea {
    let mut ta = TextArea::new();
    ta.insert_str(text);
    ta
}

// --- Basic insertion and text access ---

#[test]
fn insert_str_appends_text_and_advances_cursor() {
    let mut ta = TextArea::new();
    ta.insert_str("hello");
    assert_eq!(ta.text(), "hello");
    assert_eq!(ta.cursor(), 5);
}

#[test]
fn insert_str_at_cursor_inserts_in_place() {
    let mut ta = TextArea::new();
    ta.insert_str("hllo");
    ta.set_cursor(1);
    ta.insert_str("e");
    assert_eq!(ta.text(), "hello");
    assert_eq!(ta.cursor(), 2);
}

#[test]
fn is_empty_on_new_textarea() {
    assert!(TextArea::new().is_empty());
}

#[test]
fn is_empty_false_after_insert() {
    assert!(!textarea_with("hi").is_empty());
}

#[test]
fn set_text_replaces_content() {
    let mut ta = textarea_with("old");
    ta.set_text("new content");
    assert_eq!(ta.text(), "new content");
}

#[test]
fn set_text_clears_elements_so_cursor_moves_freely() {
    let mut ta = TextArea::new();
    ta.insert_element("[ref-1]");
    ta.set_text("plain");
    ta.set_cursor(2);
    ta.insert_str("X");
    assert_eq!(ta.text(), "plXain");
}

// --- Deletion ---

#[test]
fn delete_backward_removes_one_char() {
    let mut ta = textarea_with("hello");
    ta.delete_backward(1);
    assert_eq!(ta.text(), "hell");
    assert_eq!(ta.cursor(), 4);
}

#[test]
fn delete_backward_at_start_is_noop() {
    let mut ta = textarea_with("hello");
    ta.set_cursor(0);
    ta.delete_backward(1);
    assert_eq!(ta.text(), "hello");
    assert_eq!(ta.cursor(), 0);
}

#[test]
fn delete_forward_removes_one_char() {
    let mut ta = textarea_with("hello");
    ta.set_cursor(0);
    ta.delete_forward(1);
    assert_eq!(ta.text(), "ello");
    assert_eq!(ta.cursor(), 0);
}

#[test]
fn delete_forward_at_end_is_noop() {
    let mut ta = textarea_with("hello");
    ta.delete_forward(1);
    assert_eq!(ta.text(), "hello");
    assert_eq!(ta.cursor(), 5);
}

// --- Cursor movement ---

#[test]
fn cursor_moves_left_one_grapheme() {
    let mut ta = textarea_with("hello");
    ta.move_cursor_left();
    assert_eq!(ta.cursor(), 4);
}

#[test]
fn cursor_stops_at_start_on_left() {
    let mut ta = TextArea::new();
    ta.move_cursor_left();
    assert_eq!(ta.cursor(), 0);
}

#[test]
fn cursor_moves_right_one_grapheme() {
    let mut ta = textarea_with("hello");
    ta.set_cursor(0);
    ta.move_cursor_right();
    assert_eq!(ta.cursor(), 1);
}

#[test]
fn cursor_stops_at_end_on_right() {
    let mut ta = textarea_with("hello");
    ta.move_cursor_right();
    assert_eq!(ta.cursor(), 5);
}

#[test]
fn cursor_to_beginning_of_line() {
    let mut ta = textarea_with("first\nsecond");
    // cursor is at end of "second" (byte 12)
    ta.move_cursor_to_beginning_of_line();
    assert_eq!(ta.cursor(), 6); // start of "second"
}

#[test]
fn cursor_to_end_of_line() {
    let mut ta = textarea_with("first\nsecond");
    ta.set_cursor(0);
    ta.move_cursor_to_end_of_line();
    assert_eq!(ta.cursor(), 5); // after "first", before '\n'
}

// --- Multi-line navigation ---

#[test]
fn insert_newline_creates_new_line() {
    let mut ta = textarea_with("hello");
    ta.set_cursor(2);
    ta.insert_newline();
    assert_eq!(ta.text(), "he\nllo");
    assert_eq!(ta.cursor(), 3);
}

#[test]
fn lines_returns_each_line_as_string() {
    let ta = textarea_with("first\nsecond\nthird");
    assert_eq!(ta.lines(), vec!["first", "second", "third"]);
}

#[test]
fn lines_on_empty_returns_one_empty_string() {
    assert_eq!(TextArea::new().lines(), vec![String::new()]);
}

#[test]
fn cursor_up_moves_to_previous_line() {
    let mut ta = textarea_with("abc\nxyz");
    // cursor at end (byte 7, end of "xyz")
    ta.move_cursor_up();
    assert_eq!(ta.cursor(), 3); // end of "abc"
}

#[test]
fn cursor_up_from_first_line_goes_to_start() {
    let mut ta = textarea_with("hello");
    ta.set_cursor(3);
    ta.move_cursor_up();
    assert_eq!(ta.cursor(), 0);
}

#[test]
fn cursor_down_moves_to_next_line() {
    let mut ta = textarea_with("abc\nxyz");
    ta.set_cursor(0);
    ta.move_cursor_down();
    assert_eq!(ta.cursor(), 4); // start of "xyz"
}

#[test]
fn cursor_down_from_last_line_goes_to_end() {
    let mut ta = textarea_with("hello");
    ta.set_cursor(2);
    ta.move_cursor_down();
    assert_eq!(ta.cursor(), 5);
}

// --- Kill / yank ---

#[test]
fn kill_to_eol_removes_text_after_cursor() {
    let mut ta = textarea_with("hello world");
    ta.set_cursor(5); // after "hello"
    ta.kill_to_end_of_line();
    assert_eq!(ta.text(), "hello");
}

#[test]
fn kill_to_eol_at_eol_removes_newline() {
    let mut ta = textarea_with("line1\nline2");
    ta.set_cursor(5); // at '\n'
    ta.kill_to_end_of_line();
    assert_eq!(ta.text(), "line1line2");
}

#[test]
fn kill_to_bol_removes_text_before_cursor() {
    let mut ta = textarea_with("hello world");
    ta.set_cursor(5); // after "hello"
    ta.kill_to_beginning_of_line();
    assert_eq!(ta.text(), " world");
}

#[test]
fn yank_restores_killed_text() {
    let mut ta = textarea_with("hello world");
    ta.set_cursor(5);
    ta.kill_to_end_of_line();
    ta.yank();
    assert_eq!(ta.text(), "hello world");
    assert_eq!(ta.cursor(), 11);
}

// --- Word operations ---

#[test]
fn delete_backward_word_removes_previous_word() {
    let mut ta = textarea_with("hello world");
    ta.delete_backward_word();
    assert_eq!(ta.text(), "hello ");
}

#[test]
fn delete_forward_word_removes_next_word() {
    let mut ta = textarea_with("hello world");
    ta.set_cursor(6); // start of "world"
    ta.delete_forward_word();
    assert_eq!(ta.text(), "hello ");
}

// --- Element (attachment reference) tracking ---

#[test]
fn insert_element_creates_atomic_token() {
    let mut ta = TextArea::new();
    ta.insert_str("before ");
    ta.insert_element("[ref-1]");
    assert!(ta.text().contains("[ref-1]"));
    assert_eq!(ta.cursor(), "before [ref-1]".len());
}

#[test]
fn cursor_left_skips_entire_element() {
    let mut ta = TextArea::new();
    ta.insert_element("[ref-1]");
    // cursor is at 7 (end of element)
    ta.move_cursor_left();
    assert_eq!(ta.cursor(), 0);
}

#[test]
fn cursor_right_skips_entire_element() {
    let mut ta = TextArea::new();
    ta.insert_element("[ref-1]");
    ta.set_cursor(0);
    ta.move_cursor_right();
    assert_eq!(ta.cursor(), 7);
}

#[test]
fn delete_backward_removes_whole_element() {
    let mut ta = TextArea::new();
    ta.insert_str("before");
    ta.insert_element("[ref-1]");
    // cursor is right after element (byte 13)
    ta.delete_backward(1);
    assert_eq!(ta.text(), "before");
    assert_eq!(ta.cursor(), 6);
}

#[test]
fn text_after_element_is_typed_normally() {
    let mut ta = TextArea::new();
    ta.insert_element("[ref-1]");
    ta.insert_str(" suffix");
    assert_eq!(ta.text(), "[ref-1] suffix");
    assert_eq!(ta.cursor(), 14);
}

// --- Wrapping and height ---

#[test]
fn desired_height_is_one_for_short_text() {
    assert_eq!(textarea_with("hello").desired_height(80), 1);
}

#[test]
fn desired_height_increases_with_newlines() {
    let ta = textarea_with("line1\nline2\nline3");
    assert_eq!(ta.desired_height(80), 3);
}

#[test]
fn desired_height_wraps_long_line() {
    let ta = textarea_with("hello world foo bar baz");
    assert!(ta.desired_height(10) >= 2);
}

// --- Unicode handling ---

#[test]
fn cursor_moves_correctly_past_multibyte_char() {
    let mut ta = textarea_with("héllo"); // é = 2 bytes
    // cursor at 6 (h=1 + é=2 + l=1 + l=1 + o=1)
    assert_eq!(ta.cursor(), 6);
    ta.move_cursor_left(); // skip 'o' (1 byte)
    assert_eq!(ta.cursor(), 5);
    ta.move_cursor_left(); // skip 'l' (1 byte)
    assert_eq!(ta.cursor(), 4);
    ta.move_cursor_left(); // skip 'l' (1 byte)
    assert_eq!(ta.cursor(), 3);
    ta.move_cursor_left(); // skip 'é' (2 bytes)
    assert_eq!(ta.cursor(), 1);
}

#[test]
fn delete_backward_on_multibyte_char_removes_correctly() {
    let mut ta = textarea_with("héllo");
    ta.set_cursor(3); // after 'é' (bytes 0..3)
    ta.delete_backward(1);
    assert_eq!(ta.text(), "hllo");
    assert_eq!(ta.cursor(), 1);
}
