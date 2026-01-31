use hoosh::tui::scroll_state::ScrollState;

#[test]
fn test_scroll_state_new() {
    let state = ScrollState::new(100);
    assert_eq!(state.offset, 0);
    assert_eq!(state.content_height, 0);
    assert_eq!(state.viewport_height, 100);
}

#[test]
fn test_scroll_down() {
    let mut state = ScrollState::new(10);
    state.content_height = 50;

    state.scroll_down(5);
    assert_eq!(state.offset, 5);

    state.scroll_down(10);
    assert_eq!(state.offset, 15);
}

#[test]
fn test_scroll_down_at_bottom() {
    let mut state = ScrollState::new(10);
    state.content_height = 50;

    state.scroll_down(100);
    assert_eq!(state.offset, 40);
}

#[test]
fn test_scroll_up() {
    let mut state = ScrollState::new(10);
    state.content_height = 50;
    state.offset = 20;

    state.scroll_up(5);
    assert_eq!(state.offset, 15);

    state.scroll_up(20);
    assert_eq!(state.offset, 0);
}

#[test]
fn test_scroll_to_bottom() {
    let mut state = ScrollState::new(10);
    state.content_height = 50;

    state.scroll_to_bottom();
    assert_eq!(state.offset, 40);
}

#[test]
fn test_is_at_bottom() {
    let mut state = ScrollState::new(10);
    state.content_height = 50;

    assert!(!state.is_at_bottom());

    state.scroll_down(30);
    assert!(!state.is_at_bottom());

    state.scroll_down(20);
    assert!(state.is_at_bottom());

    state.scroll_to_bottom();
    assert!(state.is_at_bottom());
}

#[test]
fn test_page_down() {
    let mut state = ScrollState::new(10);
    state.content_height = 50;

    state.page_down();
    assert_eq!(state.offset, 9);
}

#[test]
fn test_page_up() {
    let mut state = ScrollState::new(10);
    state.content_height = 50;
    state.offset = 20;

    state.page_up();
    assert_eq!(state.offset, 11);
}

#[test]
fn test_update_viewport_height() {
    let mut state = ScrollState::new(10);
    state.content_height = 50;
    state.offset = 45;

    state.update_viewport_height(20);
    assert_eq!(state.viewport_height, 20);
    assert_eq!(state.offset, 30);
}

#[test]
fn test_update_content_height() {
    let mut state = ScrollState::new(10);
    state.content_height = 50;
    state.offset = 45;

    state.update_content_height(30);
    assert_eq!(state.content_height, 30);
    assert_eq!(state.offset, 20);
}

#[test]
fn test_scroll_with_small_content() {
    let mut state = ScrollState::new(20);
    state.content_height = 10;

    state.scroll_down(5);
    assert_eq!(state.offset, 0);

    state.scroll_to_bottom();
    assert_eq!(state.offset, 0);
    assert!(state.is_at_bottom());
}
