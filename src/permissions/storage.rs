use crate::permissions::tool_permission::ToolPermissionDescriptor;
use glob::Pattern;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
        project_root.join(".hoosh").join("permissions.json")
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

    pub fn load_permissions(project_root: &Path) -> Result<PermissionsFile, anyhow::Error> {
        let path = Self::get_permissions_path(project_root);
        let content = std::fs::read_to_string(&path)?;
        let file: PermissionsFile = serde_json::from_str(&content)?;
        Ok(file)
    }

    pub fn load_permissions_safe(project_root: &Path) -> PermissionsFile {
        Self::load_permissions(project_root).unwrap_or_default()
    }

    pub fn check_tool_permission(&self, descriptor: &ToolPermissionDescriptor) -> Option<bool> {
        let operation_str = descriptor.kind();
        let target = descriptor.target();

        for rule in &self.deny {
            if rule.matches(operation_str, target) {
                return Some(false);
            }
        }

        for rule in &self.allow {
            if rule.matches(operation_str, target) {
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

    pub fn matches(&self, operation: &str, target: &str) -> bool {
        if self.operation != operation {
            return false;
        }

        let Some(ref pattern_str) = self.pattern else {
            return true;
        };

        if operation == "bash" {
            return self.matches_bash_pattern(pattern_str, target);
        }

        self.matches_file_pattern(pattern_str, target)
    }

    fn matches_bash_pattern(&self, pattern: &str, command: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        if let Some(prefix) = pattern.strip_suffix('*') {
            command.starts_with(prefix)
        } else {
            pattern == command
        }
    }

    fn matches_file_pattern(&self, pattern: &str, path: &str) -> bool {
        Pattern::new(pattern)
            .ok()
            .map(|p| p.matches(path))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        assert!(rule.matches("write_file", "/src/main.rs"));
        assert!(rule.matches("write_file", "/src/lib/mod.rs"));
        assert!(!rule.matches("write_file", "/tests/test.rs"));
        assert!(!rule.matches("read_file", "/src/main.rs"));
    }

    #[test]
    fn test_bash_pattern_matching() {
        let rule = PermissionRule::ops_rule("bash", "cargo build*");

        assert!(rule.matches("bash", "cargo build"));
        assert!(rule.matches("bash", "cargo build --release"));
        assert!(!rule.matches("bash", "cargo check"));
        assert!(!rule.matches("bash", "npm build"));
    }

    #[test]
    fn test_exact_file_match() {
        let rule = PermissionRule::ops_rule("write_file", "/config.toml");

        assert!(rule.matches("write_file", "/config.toml"));
        assert!(!rule.matches("write_file", "/src/config.toml"));
    }

    #[test]
    fn test_global_rule() {
        let rule = PermissionRule::ops_rule("read_file", "*");

        assert!(rule.matches("read_file", "anything"));
        assert!(rule.matches("read_file", ""));
        assert!(!rule.matches("write_file", "anything"));
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
}
