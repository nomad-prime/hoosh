use crate::permissions::tool_permission::ToolPermissionDescriptor;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PermissionLoadError {
    #[error("Unsupported permissions file version: {version}. This version of hoosh only supports version 1. Please upgrade hoosh or delete the permissions file at: {}", path.display())]
    UnsupportedVersion { version: u32, path: PathBuf },

    #[error("I/O error loading permissions: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error loading permissions: {0}")]
    Parse(#[from] serde_json::Error),
}

/// Persistent permission file format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionsFile {
    pub version: u32,
    pub allow: Vec<PermissionRule>,
    pub deny: Vec<PermissionRule>,
}

impl Default for PermissionsFile {
    fn default() -> Self {
        Self {
            version: 1,
            allow: Vec::new(),
            deny: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PermissionRule {
    pub operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl PermissionsFile {
    pub fn get_permissions_path(project_root: &Path) -> PathBuf {
        project_root.join("../../.hoosh.bak").join("permissions.json")
    }

    pub fn save_permissions(&self, project_root: &Path) -> Result<(), anyhow::Error> {
        let path = Self::get_permissions_path(project_root);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    pub fn load_permissions(project_root: &Path) -> Result<PermissionsFile, PermissionLoadError> {
        let path = Self::get_permissions_path(project_root);
        let content = std::fs::read_to_string(&path).map_err(PermissionLoadError::Io)?;
        let file: PermissionsFile =
            serde_json::from_str(&content).map_err(PermissionLoadError::Parse)?;

        if file.version != 1 {
            return Err(PermissionLoadError::UnsupportedVersion {
                version: file.version,
                path,
            });
        }

        Ok(file)
    }

    /// Load permissions from disk, returning default if file doesn't exist or can't be parsed.
    /// Returns an error only for unsupported versions (which should be handled by the caller).
    pub fn load_permissions_safe(
        project_root: &Path,
    ) -> Result<PermissionsFile, PermissionLoadError> {
        match Self::load_permissions(project_root) {
            Ok(perms) => Ok(perms),
            Err(PermissionLoadError::UnsupportedVersion { version, path }) => {
                Err(PermissionLoadError::UnsupportedVersion { version, path })
            }
            Err(_) => Ok(PermissionsFile::default()),
        }
    }

    pub fn check_tool_permission(&self, descriptor: &ToolPermissionDescriptor) -> Option<bool> {
        let operation_str = descriptor.kind();

        for rule in self.deny.iter().filter(|r| r.operation == operation_str) {
            if rule.matches_pattern(descriptor) {
                return Some(false);
            }
        }

        for rule in self.allow.iter().filter(|r| r.operation == operation_str) {
            if rule.matches_pattern(descriptor) {
                return Some(true);
            }
        }

        None
    }

    pub fn add_permission(&mut self, rule: PermissionRule, allow: bool) {
        if allow {
            self.allow.push(rule);
        } else {
            self.deny.push(rule);
        }
    }

    pub fn remove_permission(&mut self, operation: &str, pattern: Option<&str>) {
        self.allow.retain(|rule| {
            !(rule.operation == operation
                && pattern.is_none_or(|p| rule.pattern.as_deref() == Some(p)))
        });

        self.deny.retain(|rule| {
            !(rule.operation == operation
                && pattern.is_none_or(|p| rule.pattern.as_deref() == Some(p)))
        });
    }
}

impl PermissionRule {
    pub fn ops_rule(operation: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            pattern: Some(pattern.into()),
            reason: None,
        }
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub fn matches_pattern(&self, descriptor: &ToolPermissionDescriptor) -> bool {
        let Some(ref pattern_str) = self.pattern else {
            return true;
        };

        descriptor.matches_pattern(pattern_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::{BashPatternMatcher, FilePatternMatcher, ToolPermissionBuilder};
    use crate::tools::bash::BashTool;
    use crate::tools::ReadFileTool;
    use std::sync::Arc;

    #[test]
    fn test_default_permissions_file() {
        let file = PermissionsFile::default();
        assert_eq!(file.version, 1);
        assert!(file.allow.is_empty());
        assert!(file.deny.is_empty());
    }

    #[test]
    fn test_serialize_deserialize() {
        let file = PermissionsFile {
            version: 1,
            allow: vec![
                PermissionRule::ops_rule("write_file", "/src/**"),
                PermissionRule::ops_rule("bash", "cargo check"),
            ],
            deny: vec![
                PermissionRule::ops_rule("write_file", "/important/**")
                    .with_reason("User explicitly denied"),
            ],
        };

        let json = serde_json::to_string_pretty(&file).unwrap();
        let deserialized: PermissionsFile = serde_json::from_str(&json).unwrap();

        assert_eq!(file.version, deserialized.version);
        assert_eq!(file.allow.len(), deserialized.allow.len());
        assert_eq!(file.deny.len(), deserialized.deny.len());
    }

    #[test]
    fn test_file_pattern_matching() {
        let rule = PermissionRule::ops_rule("write_file", "/src/**");
        let tool = ReadFileTool::new();

        let desc1 = ToolPermissionBuilder::new(&tool, "/src/main.rs")
            .with_pattern_matcher(Arc::new(FilePatternMatcher))
            .build()
            .unwrap();
        let desc2 = ToolPermissionBuilder::new(&tool, "/src/lib/mod.rs")
            .with_pattern_matcher(Arc::new(FilePatternMatcher))
            .build()
            .unwrap();
        let desc3 = ToolPermissionBuilder::new(&tool, "/tests/test.rs")
            .with_pattern_matcher(Arc::new(FilePatternMatcher))
            .build()
            .unwrap();

        assert!(rule.matches_pattern(&desc1));
        assert!(rule.matches_pattern(&desc2));
        assert!(!rule.matches_pattern(&desc3));
    }

    #[test]
    fn test_bash_pattern_matching() {
        let rule = PermissionRule::ops_rule("bash", "cargo build:*");
        let tool = BashTool::new();

        let desc1 = ToolPermissionBuilder::new(&tool, "cargo build")
            .with_pattern_matcher(Arc::new(BashPatternMatcher))
            .build()
            .unwrap();
        let desc2 = ToolPermissionBuilder::new(&tool, "cargo build --release")
            .with_pattern_matcher(Arc::new(BashPatternMatcher))
            .build()
            .unwrap();
        let desc3 = ToolPermissionBuilder::new(&tool, "cargo check")
            .with_pattern_matcher(Arc::new(BashPatternMatcher))
            .build()
            .unwrap();
        let desc4 = ToolPermissionBuilder::new(&tool, "npm build")
            .with_pattern_matcher(Arc::new(BashPatternMatcher))
            .build()
            .unwrap();

        assert!(rule.matches_pattern(&desc1));
        assert!(rule.matches_pattern(&desc2));
        assert!(!rule.matches_pattern(&desc3));
        assert!(!rule.matches_pattern(&desc4));
    }

    #[test]
    fn test_exact_file_match() {
        let rule = PermissionRule::ops_rule("write_file", "/config.toml");
        let tool = ReadFileTool::new();

        let desc1 = ToolPermissionBuilder::new(&tool, "/config.toml")
            .with_pattern_matcher(Arc::new(FilePatternMatcher))
            .build()
            .unwrap();
        let desc2 = ToolPermissionBuilder::new(&tool, "/src/config.toml")
            .with_pattern_matcher(Arc::new(FilePatternMatcher))
            .build()
            .unwrap();

        assert!(rule.matches_pattern(&desc1));
        assert!(!rule.matches_pattern(&desc2));
    }

    #[test]
    fn test_global_rule() {
        let rule = PermissionRule::ops_rule("read_file", "*");
        let tool = ReadFileTool::new();

        let desc1 = ToolPermissionBuilder::new(&tool, "anything")
            .with_pattern_matcher(Arc::new(FilePatternMatcher))
            .build()
            .unwrap();
        let desc2 = ToolPermissionBuilder::new(&tool, "some/path")
            .with_pattern_matcher(Arc::new(FilePatternMatcher))
            .build()
            .unwrap();

        assert!(rule.matches_pattern(&desc1));
        assert!(rule.matches_pattern(&desc2));
    }

    #[test]
    fn test_json_format() {
        let file = PermissionsFile {
            version: 1,
            allow: vec![
                PermissionRule::ops_rule("read_file", "/src/**"),
                PermissionRule::ops_rule("bash", "cargo check"),
                PermissionRule::ops_rule("list_directory", "*"),
            ],
            deny: vec![],
        };

        let json = serde_json::to_string_pretty(&file).unwrap();

        assert!(json.contains(r#"allow"#));
        assert!(json.contains(r#"deny"#));
        assert!(json.contains(r#"pattern": "/src/**""#));
        assert!(json.contains(r#"operation": "list_directory""#));
    }

    #[test]
    fn test_initial_permission_read_only() {
        let mut perms_file = PermissionsFile::default();
        perms_file.add_permission(PermissionRule::ops_rule("read_file", "*"), true);
        perms_file.add_permission(PermissionRule::ops_rule("list_directory", "*"), true);

        assert_eq!(perms_file.allow.len(), 2);
        assert!(perms_file.allow.iter().any(|r| r.operation == "read_file"));
        assert!(
            perms_file
                .allow
                .iter()
                .any(|r| r.operation == "list_directory")
        );
    }

    #[test]
    fn test_initial_permission_trust_project() {
        let perms_file = PermissionsFile::default();

        assert!(perms_file.allow.is_empty());
        assert!(perms_file.deny.is_empty());
    }

    #[test]
    fn test_version_validation() {
        use std::io::Write;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        let perms_path = PermissionsFile::get_permissions_path(project_root);
        std::fs::create_dir_all(perms_path.parent().unwrap()).unwrap();

        let invalid_version_json = r#"{
            "version": 2,
            "allow": [],
            "deny": []
        }"#;

        let mut file = std::fs::File::create(&perms_path).unwrap();
        file.write_all(invalid_version_json.as_bytes()).unwrap();

        let result = PermissionsFile::load_permissions(project_root);
        assert!(result.is_err());
        match result.unwrap_err() {
            PermissionLoadError::UnsupportedVersion { version, .. } => {
                assert_eq!(version, 2);
            }
            _ => panic!("Expected UnsupportedVersion error"),
        }
    }

    #[test]
    fn test_load_permissions_safe_returns_default_when_file_missing() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        let perms = PermissionsFile::load_permissions_safe(project_root).unwrap();

        assert_eq!(perms.version, 1);
        assert!(perms.allow.is_empty());
        assert!(perms.deny.is_empty());
    }

    #[test]
    fn test_load_permissions_safe_returns_error_for_unsupported_version() {
        use std::io::Write;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        let perms_path = PermissionsFile::get_permissions_path(project_root);
        std::fs::create_dir_all(perms_path.parent().unwrap()).unwrap();

        let invalid_version_json = r#"{
            "version": 2,
            "allow": [],
            "deny": []
        }"#;

        let mut file = std::fs::File::create(&perms_path).unwrap();
        file.write_all(invalid_version_json.as_bytes()).unwrap();

        let result = PermissionsFile::load_permissions_safe(project_root);
        assert!(result.is_err());
        match result.unwrap_err() {
            PermissionLoadError::UnsupportedVersion { version, .. } => {
                assert_eq!(version, 2);
            }
            _ => panic!("Expected UnsupportedVersion error"),
        }
    }
}
