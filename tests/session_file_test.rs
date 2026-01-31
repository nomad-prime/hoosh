use chrono::{Duration, Utc};
use hoosh::session_files::SessionFile;

#[test]
fn test_session_file_new() {
    let session = SessionFile::new(12345);
    assert_eq!(session.terminal_pid, 12345);
    assert!(session.messages.is_empty());
    assert!(session.context.is_empty());
    assert_eq!(session.created_at, session.last_accessed);
}

#[test]
fn test_session_file_touch() {
    let mut session = SessionFile::new(12345);
    let original_time = session.last_accessed;

    std::thread::sleep(std::time::Duration::from_millis(10));
    session.touch();

    assert!(session.last_accessed > original_time);
}

#[test]
fn test_session_file_is_stale_fresh() {
    let session = SessionFile::new(12345);
    assert!(!session.is_stale(7));
}

#[test]
fn test_session_file_is_stale_old() {
    let mut session = SessionFile::new(12345);
    session.last_accessed = Utc::now() - Duration::days(10);
    assert!(session.is_stale(7));
}

#[test]
fn test_session_file_is_stale_boundary() {
    let mut session = SessionFile::new(12345);
    session.last_accessed = Utc::now() - Duration::days(7);
    assert!(!session.is_stale(7));

    session.last_accessed = Utc::now() - Duration::days(8);
    assert!(session.is_stale(7));
}

#[test]
fn test_session_file_serialization() {
    let session = SessionFile::new(12345);
    let json = serde_json::to_string(&session).unwrap();
    let deserialized: SessionFile = serde_json::from_str(&json).unwrap();

    assert_eq!(session.terminal_pid, deserialized.terminal_pid);
    assert_eq!(session.messages.len(), deserialized.messages.len());
}
