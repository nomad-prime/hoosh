use std::path::{Path, PathBuf};
use std::str::FromStr;

/// Operation kind for cache keys (structured alternative to string-based keys)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationKind {
    Read,
    Write,
    Create,
    Delete,
    Bash,
    List,
}

impl FromStr for OperationKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "read" => Ok(Self::Read),
            "write" => Ok(Self::Write),
            "create" => Ok(Self::Create),
            "delete" => Ok(Self::Delete),
            "bash" => Ok(Self::Bash),
            "list" => Ok(Self::List),
            _ => Err(()),
        }
    }
}

impl OperationKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Create => "create",
            Self::Delete => "delete",
            Self::Bash => "bash",
            Self::List => "list",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PermissionCacheKey {
    ProjectWide {
        operation: OperationKind,
        project_root: PathBuf,
    },
    Specific {
        operation: OperationKind,
        target: PathBuf,
    },
    Directory {
        operation: OperationKind,
        directory: PathBuf,
    },
    Global {
        operation: OperationKind,
    },
}

impl PermissionCacheKey {
    pub fn precedence(&self) -> u8 {
        match self {
            Self::ProjectWide { .. } => 4,
            Self::Specific { .. } => 3,
            Self::Directory { .. } => 2,
            Self::Global { .. } => 1,
        }
    }

    pub fn matches(&self, operation_kind: OperationKind, target: &str) -> bool {
        match self {
            Self::ProjectWide {
                operation: op,
                project_root,
            } => *op == operation_kind && Self::is_within_project_static(target, project_root),
            Self::Specific {
                operation: op,
                target: cached_target,
            } => *op == operation_kind && Self::paths_equal(target, cached_target),
            Self::Directory {
                operation: op,
                directory,
            } => *op == operation_kind && Self::is_in_directory(target, directory),
            Self::Global { operation: op } => *op == operation_kind,
        }
    }

    /// Check if a target path is within a project directory (public static version)
    /// This handles both existing and non-existent files by normalizing paths
    pub fn is_within_project_static(target: &str, project_root: &Path) -> bool {
        let target_path = PathBuf::from(target);

        // Canonicalize project root first
        let canonical_project = match project_root.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };

        // For target, try to canonicalize. If it fails, find the first existing ancestor
        let canonical_target = if let Ok(t) = target_path.canonicalize() {
            t
        } else {
            // Find the first existing ancestor directory
            let mut current = target_path.clone();
            let mut found_existing = None;

            loop {
                if current.exists() {
                    found_existing = current.canonicalize().ok();
                    break;
                }

                current = match current.parent() {
                    Some(parent) if parent != current => parent.to_path_buf(),
                    _ => break,
                };
            }

            match found_existing {
                Some(existing) => {
                    // Append remaining path components
                    if let Ok(rel_path) = target_path.strip_prefix(existing.clone()) {
                        existing.join(rel_path)
                    } else {
                        existing
                    }
                }
                None => return false, // Couldn't find any existing ancestor
            }
        };

        canonical_target.starts_with(&canonical_project)
    }

    /// Check if two paths are equal (accounting for canonicalization)
    fn paths_equal(target: &str, cached: &Path) -> bool {
        PathBuf::from(target)
            .canonicalize()
            .and_then(|t| cached.canonicalize().map(|c| t == c))
            .unwrap_or(false)
    }

    /// Check if a target is within a directory
    fn is_in_directory(target: &str, directory: &Path) -> bool {
        Path::new(target)
            .parent()
            .and_then(|p| p.canonicalize().ok())
            .and_then(|p| directory.canonicalize().ok().map(|d| p == d))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_kind_conversion() {
        assert_eq!("read".parse::<OperationKind>(), Ok(OperationKind::Read));
        assert_eq!("write".parse::<OperationKind>(), Ok(OperationKind::Write));
        assert_eq!("create".parse::<OperationKind>(), Ok(OperationKind::Create));
        assert_eq!("delete".parse::<OperationKind>(), Ok(OperationKind::Delete));
        assert_eq!("bash".parse::<OperationKind>(), Ok(OperationKind::Bash));
        assert_eq!("list".parse::<OperationKind>(), Ok(OperationKind::List));
        assert!("invalid".parse::<OperationKind>().is_err());
    }

    #[test]
    fn test_operation_kind_as_str() {
        assert_eq!(OperationKind::Read.as_str(), "read");
        assert_eq!(OperationKind::Write.as_str(), "write");
        assert_eq!(OperationKind::Create.as_str(), "create");
        assert_eq!(OperationKind::Delete.as_str(), "delete");
        assert_eq!(OperationKind::Bash.as_str(), "bash");
        assert_eq!(OperationKind::List.as_str(), "list");
    }

    #[test]
    fn test_cache_key_precedence() {
        let global = PermissionCacheKey::Global {
            operation: OperationKind::Write,
        };
        let directory = PermissionCacheKey::Directory {
            operation: OperationKind::Write,
            directory: PathBuf::from("/dir"),
        };
        let specific = PermissionCacheKey::Specific {
            operation: OperationKind::Write,
            target: PathBuf::from("/dir/file.txt"),
        };
        let project_wide = PermissionCacheKey::ProjectWide {
            operation: OperationKind::Write,
            project_root: PathBuf::from("/project"),
        };

        assert_eq!(global.precedence(), 1);
        assert_eq!(directory.precedence(), 2);
        assert_eq!(specific.precedence(), 3);
        assert_eq!(project_wide.precedence(), 4);

        // Higher precedence should override lower
        assert!(project_wide.precedence() > specific.precedence());
        assert!(specific.precedence() > directory.precedence());
        assert!(directory.precedence() > global.precedence());
    }

    #[test]
    fn test_cache_key_matches_global() {
        let key = PermissionCacheKey::Global {
            operation: OperationKind::Write,
        };

        // Should match any write operation
        assert!(key.matches(OperationKind::Write, "/any/path/file.txt"));
        assert!(key.matches(OperationKind::Write, "/other/file.txt"));

        // Should not match other operation types
        assert!(!key.matches(OperationKind::Read, "/any/path/file.txt"));
        assert!(!key.matches(OperationKind::Delete, "/file.txt"));
    }

    #[test]
    fn test_cache_key_matches_specific() {
        let key = PermissionCacheKey::Specific {
            operation: OperationKind::Write,
            target: PathBuf::from("Cargo.toml"),
        };

        // Should match the exact file (assuming Cargo.toml exists in current dir)
        if PathBuf::from("Cargo.toml").canonicalize().is_ok() {
            assert!(key.matches(OperationKind::Write, "Cargo.toml"));
        }

        // Should not match other files or operation types
        assert!(!key.matches(OperationKind::Read, "Cargo.toml"));
        assert!(!key.matches(OperationKind::Write, "other.txt"));
    }

    #[test]
    fn test_is_within_project_static_existing_files() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Create a test file
        let test_file = project_root.join("test.txt");
        std::fs::write(&test_file, "test").unwrap();

        // Test that existing file is recognized as within project
        assert!(PermissionCacheKey::is_within_project_static(
            test_file.to_str().unwrap(),
            project_root
        ));

        // Test that file outside project is not recognized
        let outside_file = std::env::temp_dir().join("outside_test_file.txt");
        std::fs::write(&outside_file, "outside").unwrap();
        assert!(!PermissionCacheKey::is_within_project_static(
            outside_file.to_str().unwrap(),
            project_root
        ));
        let _ = std::fs::remove_file(&outside_file);
    }

    #[test]
    fn test_is_within_project_static_nonexistent_files() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        // Test that non-existent file within project is still recognized
        let new_file = project_root.join("new_file.txt");
        assert!(PermissionCacheKey::is_within_project_static(
            new_file.to_str().unwrap(),
            project_root
        ));

        // Test nested non-existent file
        let nested_new_file = project_root.join("subdir/nested_new.txt");
        assert!(PermissionCacheKey::is_within_project_static(
            nested_new_file.to_str().unwrap(),
            project_root
        ));

        // Test file outside project (non-existent)
        let outside_new_file = std::env::temp_dir().join("outside_new_file_test.txt");
        assert!(!PermissionCacheKey::is_within_project_static(
            outside_new_file.to_str().unwrap(),
            project_root
        ));
    }
}
