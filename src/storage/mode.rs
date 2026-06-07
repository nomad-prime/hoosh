use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ConversationStorageMode {
    #[default]
    Off,
    Local,
    Central,
}

impl ConversationStorageMode {
    pub fn is_enabled(self) -> bool {
        !matches!(self, Self::Off)
    }
}

pub fn deserialize_conversation_storage<'de, D>(
    deserializer: D,
) -> Result<Option<ConversationStorageMode>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Bool(bool),
        Mode(String),
    }

    let opt = Option::<Raw>::deserialize(deserializer)?;
    Ok(match opt {
        None => None,
        Some(Raw::Bool(true)) => Some(ConversationStorageMode::Local),
        Some(Raw::Bool(false)) => Some(ConversationStorageMode::Off),
        Some(Raw::Mode(s)) => match s.as_str() {
            "off" => Some(ConversationStorageMode::Off),
            "local" => Some(ConversationStorageMode::Local),
            "central" => Some(ConversationStorageMode::Central),
            other => {
                return Err(Error::custom(format!(
                    "invalid conversation_storage value: {} (expected off|local|central|true|false)",
                    other
                )));
            }
        },
    })
}

pub fn encode_cwd(cwd: &Path) -> String {
    let s = cwd.to_string_lossy();
    s.replace(std::path::MAIN_SEPARATOR, "-")
}

/// Resolve the storage root directory for the given mode and cwd.
///
/// Returns:
/// - `Off`     → `None` (no on-disk storage)
/// - `Local`   → `<cwd>/.hoosh/conversations`
/// - `Central` → `<data_dir>/projects/<encoded-cwd>/conversations`
pub fn resolve_storage_root(
    mode: ConversationStorageMode,
    cwd: &Path,
    data_dir: &Path,
) -> Option<PathBuf> {
    match mode {
        ConversationStorageMode::Off => None,
        ConversationStorageMode::Local => Some(cwd.join(".hoosh").join("conversations")),
        ConversationStorageMode::Central => Some(
            data_dir
                .join("projects")
                .join(encode_cwd(cwd))
                .join("conversations"),
        ),
    }
}

const GITIGNORE_MARKER: &str = ".hoosh/conversations/";
const GITIGNORE_BLOCK: &str = "\n# hoosh conversations (added automatically). Remove this line if you want to commit conversation history.\n.hoosh/conversations/\n.hoosh/memory/\n";

/// In a git repo, append `.hoosh/conversations/` and `.hoosh/memory/` to `.gitignore`
/// if not already present. Idempotent and silent in non-git directories.
pub fn ensure_local_storage_gitignored(cwd: &Path) -> Result<()> {
    if !cwd.join(".git").exists() {
        return Ok(());
    }

    let gitignore_path = cwd.join(".gitignore");
    let existing = match std::fs::read_to_string(&gitignore_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).context("Failed to read .gitignore"),
    };

    if existing
        .lines()
        .any(|l| l.trim() == GITIGNORE_MARKER || l.trim() == ".hoosh/conversations")
    {
        return Ok(());
    }

    let mut new_content = existing;
    if !new_content.is_empty() && !new_content.ends_with('\n') {
        new_content.push('\n');
    }
    new_content.push_str(GITIGNORE_BLOCK.trim_start_matches('\n'));

    std::fs::write(&gitignore_path, new_content).context("Failed to write .gitignore")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn encode_cwd_replaces_separators() {
        let p = Path::new("/Users/dev/Projects/hoosh");
        #[cfg(unix)]
        assert_eq!(encode_cwd(p), "-Users-dev-Projects-hoosh");
    }

    #[test]
    fn resolve_off_is_none() {
        assert!(
            resolve_storage_root(
                ConversationStorageMode::Off,
                Path::new("/tmp/proj"),
                Path::new("/tmp/data"),
            )
            .is_none()
        );
    }

    #[test]
    fn resolve_local_uses_cwd() {
        let r = resolve_storage_root(
            ConversationStorageMode::Local,
            Path::new("/tmp/proj"),
            Path::new("/tmp/data"),
        )
        .unwrap();
        assert!(r.ends_with(".hoosh/conversations"));
    }

    #[test]
    fn resolve_central_uses_data_dir_and_encoded_cwd() {
        let r = resolve_storage_root(
            ConversationStorageMode::Central,
            Path::new("/tmp/proj"),
            Path::new("/tmp/data"),
        )
        .unwrap();
        let s = r.to_string_lossy();
        assert!(s.contains("/tmp/data/projects/"));
        assert!(s.contains("-tmp-proj"));
        assert!(s.ends_with("conversations"));
    }

    #[test]
    fn gitignore_no_op_outside_git_repo() {
        let tmp = TempDir::new().unwrap();
        ensure_local_storage_gitignored(tmp.path()).unwrap();
        assert!(!tmp.path().join(".gitignore").exists());
    }

    #[test]
    fn gitignore_writes_lines_when_missing() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        ensure_local_storage_gitignored(tmp.path()).unwrap();
        let content = std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains(".hoosh/conversations/"));
        assert!(content.contains(".hoosh/memory/"));
    }

    #[test]
    fn gitignore_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        std::fs::write(tmp.path().join(".gitignore"), "node_modules/\n").unwrap();
        ensure_local_storage_gitignored(tmp.path()).unwrap();
        ensure_local_storage_gitignored(tmp.path()).unwrap();
        let content = std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        let occurrences = content.matches(".hoosh/conversations/").count();
        assert_eq!(occurrences, 1);
        assert!(content.contains("node_modules/"));
    }

    #[test]
    fn gitignore_skips_when_marker_already_present() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        std::fs::write(tmp.path().join(".gitignore"), ".hoosh/conversations/\n").unwrap();
        ensure_local_storage_gitignored(tmp.path()).unwrap();
        let content = std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(!content.contains("# hoosh conversations"));
    }

    #[test]
    fn deserialize_accepts_legacy_bool_true() {
        let toml = r#"conversation_storage = true"#;
        #[derive(Deserialize)]
        struct T {
            #[serde(default, deserialize_with = "deserialize_conversation_storage")]
            #[allow(dead_code)]
            conversation_storage: Option<ConversationStorageMode>,
        }
        let t: T = toml::from_str(toml).unwrap();
        assert_eq!(t.conversation_storage, Some(ConversationStorageMode::Local));
    }

    #[test]
    fn deserialize_accepts_legacy_bool_false() {
        let toml = r#"conversation_storage = false"#;
        #[derive(Deserialize)]
        struct T {
            #[serde(default, deserialize_with = "deserialize_conversation_storage")]
            #[allow(dead_code)]
            conversation_storage: Option<ConversationStorageMode>,
        }
        let t: T = toml::from_str(toml).unwrap();
        assert_eq!(t.conversation_storage, Some(ConversationStorageMode::Off));
    }

    #[test]
    fn deserialize_accepts_string_central() {
        let toml = r#"conversation_storage = "central""#;
        #[derive(Deserialize)]
        struct T {
            #[serde(default, deserialize_with = "deserialize_conversation_storage")]
            #[allow(dead_code)]
            conversation_storage: Option<ConversationStorageMode>,
        }
        let t: T = toml::from_str(toml).unwrap();
        assert_eq!(
            t.conversation_storage,
            Some(ConversationStorageMode::Central)
        );
    }

    #[test]
    fn deserialize_rejects_unknown_string() {
        let toml = r#"conversation_storage = "wat""#;
        #[derive(Deserialize)]
        struct T {
            #[serde(default, deserialize_with = "deserialize_conversation_storage")]
            #[allow(dead_code)]
            conversation_storage: Option<ConversationStorageMode>,
        }
        let r: Result<T, _> = toml::from_str(toml);
        assert!(r.is_err());
    }
}
