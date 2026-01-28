// Session file storage and management for tagged mode

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

/// Session file structure for tagged mode context persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFile {
    /// Terminal process ID (from $PPID)
    pub terminal_pid: u32,

    /// Session creation timestamp
    #[serde(with = "chrono::serde::ts_seconds")]
    pub created_at: DateTime<Utc>,

    /// Last access timestamp (updated on every load/save)
    #[serde(with = "chrono::serde::ts_seconds")]
    pub last_accessed: DateTime<Utc>,

    /// Conversation messages (full history)
    pub messages: Vec<serde_json::Value>,

    /// Extensible metadata (e.g., current working directory, tool state)
    #[serde(default)]
    pub context: HashMap<String, serde_json::Value>,
}

impl SessionFile {
    /// Create a new session file for the given terminal PID
    pub fn new(terminal_pid: u32) -> Self {
        let now = Utc::now();
        Self {
            terminal_pid,
            created_at: now,
            last_accessed: now,
            messages: Vec::new(),
            context: HashMap::new(),
        }
    }

    /// Update last_accessed timestamp
    pub fn touch(&mut self) {
        self.last_accessed = Utc::now();
    }

    /// Check if session file is stale (>threshold_days old)
    pub fn is_stale(&self, threshold_days: i64) -> bool {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.last_accessed);
        duration.num_days() > threshold_days
    }

    pub fn save(&mut self) -> Result<()> {
        self.touch();
        let path = get_session_file_path(self.terminal_pid)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create sessions directory")?;
        }

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .context("Failed to open session file for writing")?;

        if file.try_lock_exclusive().is_ok() {
            let json =
                serde_json::to_string_pretty(self).context("Failed to serialize session file")?;
            let mut file = file;
            file.write_all(json.as_bytes())
                .context("Failed to write session file")?;
            file.unlock().ok();
        } else {
            eprintln!("⚠️  Could not acquire lock on session file, skipping save");
        }

        Ok(())
    }

    pub fn load(terminal_pid: u32) -> Result<Option<Self>> {
        let path = get_session_file_path(terminal_pid)?;

        if !path.exists() {
            return Ok(None);
        }

        let mut file = File::open(&path).context("Failed to open session file")?;

        if file.try_lock_shared().is_ok() {
            let mut contents = String::new();
            file.read_to_string(&mut contents)
                .context("Failed to read session file")?;
            file.unlock().ok();

            let session: Self =
                serde_json::from_str(&contents).context("Failed to parse session file")?;
            Ok(Some(session))
        } else {
            eprintln!("⚠️  Could not acquire lock on session file, starting fresh");
            Ok(None)
        }
    }
}

/// Get terminal PID from environment variable with fallback
pub fn get_terminal_pid() -> Result<u32> {
    // Try $PPID environment variable (shell's PID)
    if let Ok(ppid) = std::env::var("PPID")
        && let Ok(pid) = ppid.parse()
    {
        return Ok(pid);
    }

    // Fallback to current process ID
    Ok(std::process::id())
}

/// Get the directory where session files are stored
pub fn get_sessions_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Failed to get home directory")?;
    let sessions_dir = home.join(".hoosh").join("sessions");

    // Create directory if it doesn't exist
    if !sessions_dir.exists() {
        std::fs::create_dir_all(&sessions_dir).context("Failed to create sessions directory")?;
    }

    Ok(sessions_dir)
}

/// Get the path to a session file for the given PID
pub fn get_session_file_path(pid: u32) -> Result<PathBuf> {
    let sessions_dir = get_sessions_dir()?;
    Ok(sessions_dir.join(format!("session_{}.json", pid)))
}
