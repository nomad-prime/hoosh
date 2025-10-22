use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub struct PathValidator {
    working_directory: PathBuf,
}

impl PathValidator {
    pub fn new(working_directory: PathBuf) -> Self {
        Self { working_directory }
    }

    /// Validates and resolves a path, ensuring it's within the working directory
    /// Works with both existing and non-existing paths
    pub fn validate_and_resolve(&self, path: &str) -> Result<PathBuf> {
        let resolved = self.resolve_path(path);
        self.check_security(&resolved)?;
        Ok(resolved)
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.working_directory.join(path)
        }
    }

    fn check_security(&self, path: &Path) -> Result<()> {
        let canonical_working = self
            .working_directory
            .canonicalize()
            .context("Failed to resolve working directory")?;

        let canonical_path = if path.exists() {
            path.canonicalize().context("Failed to resolve path")?
        } else {
            // For non-existent paths, canonicalize the parent and then append the file name
            if let Some(parent) = path.parent() {
                if parent.as_os_str().is_empty() {
                    // If parent is empty, use the working directory
                    canonical_working.join(
                        path.file_name()
                            .ok_or_else(|| anyhow::anyhow!("Invalid file path: no file name"))?,
                    )
                } else {
                    // Try to canonicalize parent; if it fails, the path is likely trying to escape
                    let canonical_parent = parent.canonicalize().map_err(|_| {
                        anyhow::anyhow!(
                            "Access denied: cannot access files outside working directory"
                        )
                    })?;
                    canonical_parent.join(
                        path.file_name()
                            .ok_or_else(|| anyhow::anyhow!("Invalid file path: no file name"))?,
                    )
                }
            } else {
                anyhow::bail!("Invalid file path");
            }
        };

        if !canonical_path.starts_with(&canonical_working) {
            anyhow::bail!(
                "Access denied: cannot access files outside working directory\n\
                 Attempted: {}\n\
                 Working directory: {}",
                canonical_path.display(),
                canonical_working.display()
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_validate_and_resolve_relative_path() {
        let temp_dir = tempdir().unwrap();
        let validator = PathValidator::new(temp_dir.path().to_path_buf());

        let result = validator.validate_and_resolve("test.txt");
        assert!(result.is_ok());
        let resolved = result.unwrap();
        assert!(resolved.ends_with("test.txt"));
    }

    #[test]
    fn test_validate_and_resolve_current_dir() {
        let temp_dir = tempdir().unwrap();
        let validator = PathValidator::new(temp_dir.path().to_path_buf());

        let result = validator.validate_and_resolve(".");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_and_resolve_subdirectory() {
        let temp_dir = tempdir().unwrap();
        let subdir = temp_dir.path().join("subdir");
        std::fs::create_dir(&subdir).unwrap();

        let validator = PathValidator::new(temp_dir.path().to_path_buf());
        let result = validator.validate_and_resolve("subdir/file.txt");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_and_resolve_path_traversal_attack() {
        let temp_dir = tempdir().unwrap();
        let validator = PathValidator::new(temp_dir.path().to_path_buf());

        // Attempt to escape working directory with ..
        let result = validator.validate_and_resolve("../../../etc/passwd");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Access denied"),
            "Expected 'Access denied' in error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_validate_and_resolve_absolute_path_outside() {
        let temp_dir = tempdir().unwrap();
        let validator = PathValidator::new(temp_dir.path().to_path_buf());

        // Attempt to use absolute path outside working directory
        let result = validator.validate_and_resolve("/etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Access denied"));
    }
}
