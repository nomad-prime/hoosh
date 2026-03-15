use hoosh::terminal_capabilities::TerminalCapabilities;

#[test]
fn detect_does_not_panic() {
    TerminalCapabilities::detect().unwrap();
}
