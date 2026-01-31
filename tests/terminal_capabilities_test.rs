use hoosh::terminal_capabilities::TerminalCapabilities;
use hoosh::terminal_mode::TerminalMode;

#[test]
fn test_terminal_capabilities_detect_vscode() {
    unsafe {
        std::env::set_var("TERM_PROGRAM", "vscode");
    }
    let caps = TerminalCapabilities::detect().unwrap();
    assert!(caps.is_vscode);
    assert_eq!(caps.term_program, Some("vscode".to_string()));
    unsafe {
        std::env::remove_var("TERM_PROGRAM");
    }
}

#[test]
fn test_terminal_capabilities_detect_iterm() {
    unsafe {
        std::env::set_var("TERM_PROGRAM", "iTerm.app");
    }
    let caps = TerminalCapabilities::detect().unwrap();
    assert!(caps.is_iterm);
    assert_eq!(caps.term_program, Some("iTerm.app".to_string()));
    unsafe {
        std::env::remove_var("TERM_PROGRAM");
    }
}

#[test]
fn test_terminal_capabilities_mouse_support() {
    let caps = TerminalCapabilities::detect().unwrap();
    assert!(caps.supports_mouse);
}

#[test]
fn test_terminal_capabilities_no_mouse_dumb_terminal() {
    unsafe {
        std::env::set_var("TERM", "dumb");
    }
    let caps = TerminalCapabilities::detect().unwrap();
    assert!(!caps.supports_mouse);
    unsafe {
        std::env::remove_var("TERM");
    }
}

#[test]
fn test_warn_vscode_with_inline_no_panic() {
    unsafe {
        std::env::set_var("TERM_PROGRAM", "vscode");
    }
    let caps = TerminalCapabilities::detect().unwrap();
    caps.warn_if_vscode_with_inline(TerminalMode::Inline);
    unsafe {
        std::env::remove_var("TERM_PROGRAM");
    }
}
