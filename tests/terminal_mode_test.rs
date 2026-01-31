use hoosh::terminal_mode::{TerminalMode, select_terminal_mode};

#[test]
fn test_terminal_mode_from_str_inline() {
    let mode: TerminalMode = "inline".parse().unwrap();
    assert_eq!(mode, TerminalMode::Inline);
}

#[test]
fn test_terminal_mode_from_str_fullview() {
    let mode: TerminalMode = "fullview".parse().unwrap();
    assert_eq!(mode, TerminalMode::Fullview);
}

#[test]
fn test_terminal_mode_from_str_tagged() {
    let mode: TerminalMode = "tagged".parse().unwrap();
    assert_eq!(mode, TerminalMode::Tagged);
}

#[test]
fn test_terminal_mode_from_str_case_insensitive() {
    assert_eq!(
        "INLINE".parse::<TerminalMode>().unwrap(),
        TerminalMode::Inline
    );
    assert_eq!(
        "Fullview".parse::<TerminalMode>().unwrap(),
        TerminalMode::Fullview
    );
    assert_eq!(
        "TaGgEd".parse::<TerminalMode>().unwrap(),
        TerminalMode::Tagged
    );
}

#[test]
fn test_terminal_mode_from_str_invalid() {
    assert!("invalid".parse::<TerminalMode>().is_err());
    assert!("".parse::<TerminalMode>().is_err());
}

#[test]
fn test_terminal_mode_default() {
    assert_eq!(TerminalMode::default(), TerminalMode::Inline);
}

#[test]
fn test_terminal_mode_display() {
    assert_eq!(TerminalMode::Inline.to_string(), "inline");
    assert_eq!(TerminalMode::Fullview.to_string(), "fullview");
    assert_eq!(TerminalMode::Tagged.to_string(), "tagged");
}

#[test]
fn test_select_terminal_mode_cli_priority() {
    let mode = select_terminal_mode(Some("fullview".to_string()), Some("tagged".to_string()));
    assert_eq!(mode, TerminalMode::Fullview);
}

#[test]
fn test_select_terminal_mode_config_fallback() {
    let mode = select_terminal_mode(None, Some("tagged".to_string()));
    assert_eq!(mode, TerminalMode::Tagged);
}

#[test]
fn test_select_terminal_mode_default_fallback() {
    let mode = select_terminal_mode(None, None);
    assert_eq!(mode, TerminalMode::Inline);
}

#[test]
fn test_select_terminal_mode_invalid_cli() {
    let mode = select_terminal_mode(Some("invalid".to_string()), Some("fullview".to_string()));
    assert_eq!(mode, TerminalMode::Fullview);
}
