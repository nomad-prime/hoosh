// Session file cleanup logic

use anyhow::Result;
use std::fs;

use super::store::{SessionFile, get_sessions_dir};

const DEFAULT_STALE_THRESHOLD_DAYS: i64 = 7;

/// Cleanup stale session files (>7 days old)
pub fn cleanup_stale_sessions() -> Result<()> {
    cleanup_stale_sessions_with_threshold(DEFAULT_STALE_THRESHOLD_DAYS)
}

/// Cleanup stale session files with custom threshold
pub fn cleanup_stale_sessions_with_threshold(threshold_days: i64) -> Result<()> {
    let sessions_dir = get_sessions_dir()?;

    if !sessions_dir.exists() {
        return Ok(()); // Nothing to cleanup
    }

    for entry in fs::read_dir(sessions_dir)? {
        let entry = entry?;
        let path = entry.path();

        // Only process .json files
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }

        // Try to read and parse session file
        if let Ok(content) = fs::read_to_string(&path)
            && let Ok(session) = serde_json::from_str::<SessionFile>(&content)
            && session.is_stale(threshold_days)
        {
            // Remove stale session file
            let _ = fs::remove_file(&path);
        }
    }

    Ok(())
}

/// Check if a PID exists (process validation)
#[cfg(unix)]
pub fn check_pid_exists(pid: u32) -> bool {
    use std::process::Command;

    // Use `kill -0` to check if process exists without sending a signal
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
pub fn check_pid_exists(_pid: u32) -> bool {
    // On non-Unix systems, assume PID exists (graceful degradation)
    true
}
