use hoosh::session_files::SessionFile;
use tempfile::tempdir;

#[test]
#[ignore]
fn test_session_file_save_and_load() {
    let temp_dir = tempdir().unwrap();
    unsafe {
        std::env::set_var("HOME", temp_dir.path());
    }

    let mut session = SessionFile::new(99999);
    session
        .context
        .insert("test_key".to_string(), serde_json::json!("test_value"));

    session.save().unwrap();

    let loaded = SessionFile::load(99999).unwrap();
    assert!(loaded.is_some());

    let loaded = loaded.unwrap();
    assert_eq!(loaded.terminal_pid, 99999);
    assert_eq!(
        loaded.context.get("test_key"),
        Some(&serde_json::json!("test_value"))
    );

    unsafe {
        std::env::remove_var("HOME");
    }
}

#[test]
fn test_session_file_load_nonexistent() {
    let temp_dir = tempdir().unwrap();
    unsafe {
        std::env::set_var("HOME", temp_dir.path());
    }

    let loaded = SessionFile::load(88888).unwrap();
    assert!(loaded.is_none());

    unsafe {
        std::env::remove_var("HOME");
    }
}

#[test]
#[ignore]
fn test_session_file_save_creates_directory() {
    let temp_dir = tempdir().unwrap();
    unsafe {
        std::env::set_var("HOME", temp_dir.path());
    }

    let mut session = SessionFile::new(77777);
    session.save().unwrap();

    let sessions_dir = temp_dir.path().join(".hoosh").join("sessions");
    assert!(sessions_dir.exists());

    unsafe {
        std::env::remove_var("HOME");
    }
}

#[test]
#[ignore]
fn test_session_file_touch_updates_timestamp() {
    let temp_dir = tempdir().unwrap();
    unsafe {
        std::env::set_var("HOME", temp_dir.path());
    }

    let mut session = SessionFile::new(66666);
    session.save().unwrap();

    std::thread::sleep(std::time::Duration::from_millis(10));

    let mut loaded = SessionFile::load(66666).unwrap().unwrap();
    let original_time = loaded.last_accessed;

    std::thread::sleep(std::time::Duration::from_millis(10));
    loaded.touch();
    loaded.save().unwrap();

    let reloaded = SessionFile::load(66666).unwrap().unwrap();
    assert!(reloaded.last_accessed > original_time);

    unsafe {
        std::env::remove_var("HOME");
    }
}
