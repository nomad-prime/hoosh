use chrono::{Duration, Utc};
use hoosh::session_files::{SessionFile, cleanup_stale_sessions};
use tempfile::tempdir;

#[test]
#[ignore]
fn test_cleanup_stale_sessions_removes_old_files() {
    let temp_dir = tempdir().unwrap();
    unsafe {
        std::env::set_var("HOME", temp_dir.path());
    }

    let mut old_session = SessionFile::new(11111);
    old_session.last_accessed = Utc::now() - Duration::days(10);
    old_session.save().unwrap();

    let mut fresh_session = SessionFile::new(22222);
    fresh_session.save().unwrap();

    cleanup_stale_sessions().unwrap();

    let old_loaded = SessionFile::load(11111).unwrap();
    assert!(old_loaded.is_none());

    let fresh_loaded = SessionFile::load(22222).unwrap();
    assert!(fresh_loaded.is_some());

    unsafe {
        std::env::remove_var("HOME");
    }
}

#[test]
#[ignore]
fn test_cleanup_stale_sessions_no_directory() {
    let temp_dir = tempdir().unwrap();
    unsafe {
        std::env::set_var("HOME", temp_dir.path());
    }

    let result = cleanup_stale_sessions();
    assert!(result.is_ok());

    unsafe {
        std::env::remove_var("HOME");
    }
}

#[test]
#[ignore]
fn test_cleanup_stale_sessions_empty_directory() {
    let temp_dir = tempdir().unwrap();
    unsafe {
        std::env::set_var("HOME", temp_dir.path());
    }

    std::fs::create_dir_all(temp_dir.path().join(".hoosh").join("sessions")).unwrap();

    let result = cleanup_stale_sessions();
    assert!(result.is_ok());

    unsafe {
        std::env::remove_var("HOME");
    }
}
