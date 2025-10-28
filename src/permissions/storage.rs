use glob::Pattern;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;


/// Persistent permission file format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionsFile {
    pub version: u32,
    pub trusted: bool,
    pub allow: Vec<PermissionRule>,
    pub deny: Vec<PermissionRule>,
}

impl Default for PermissionsFile {
    fn default() -> Self {
        Self {
            version: 1,
            trusted: false,
            allow: Vec::new(),
            deny: Vec::new(),
        }
    }
}

/// A single permission rule (in allow or deny list)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PermissionRule {
    /// Operation type: "read", "write", "create", "delete", "bash", "web_search"
    pub operation: String,

    /// Pattern for matching:
    /// - File operations: glob patterns like "/src/**", "/config.toml"
    /// - Bash commands: command patterns like "cargo build*", "npm*"
    /// - Non-path operations: omit this field (e.g., web_search)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,

    /// Optional reason for this rule (helpful for deny rules)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl PermissionsFile {
    /// Get the permissions file path for a project
    pub fn get_permissions_path(project_root: &PathBuf) -> PathBuf {
        project_root.join(".hoosh").join("permissions.json")
    }

    /// Save permissions file to disk
    pub fn save_permissions(&self, project_root: &PathBuf) -> Result<(), anyhow::Error> {
        let path = Self::get_permissions_path(project_root);
        
        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        // Write to file with pretty formatting
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Load permissions file from disk
    pub fn load_permissions(project_root: &PathBuf) -> Result<PermissionsFile, anyhow::Error> {
        let path = Self::get_permissions_path(project_root);
        let content = std::fs::read_to_string(&path)?;
        let file: PermissionsFile = serde_json::from_str(&content)?;
        Ok(file)
    }

    /// Safely load permissions file with fallback to default
    pub fn load_permissions_safe(project_root: &PathBuf) -> PermissionsFile {
        Self::load_permissions(project_root).unwrap_or_default()
    }
    
    /// Check if a stored permission allows the operation
    pub fn check_permission(&self, operation: &super::OperationType) -> Option<bool> {
        let operation_str = operation.operation_kind();
        let target = operation.target();
        
        // Check deny list first (explicit deny takes precedence)
        for rule in &self.deny {
            if rule.matches(operation_str, target) {
                return Some(false);
            }
        }
        
        // Check allow list
        for rule in &self.allow {
            if rule.matches(operation_str, target) {
                return Some(true);
            }
        }
        
        // No matching rule found
        None
    }
    
    /// Add a permission rule to either allow or deny list
    pub fn add_permission(&mut self, rule: PermissionRule, allow: bool) {
        if allow {
            self.allow.push(rule);
        } else {
            self.deny.push(rule);
        }
    }
    
    /// Remove all permissions matching the given operation and pattern
    pub fn remove_permission(&mut self, operation: &str, pattern: Option<&str>) {
        self.allow.retain(|rule| {
            !(rule.operation == operation 
            && pattern.map_or(true, |p| rule.pattern.as_deref() == Some(p)))
        });
        
        self.deny.retain(|rule| {
            !(rule.operation == operation 
            && pattern.map_or(true, |p| rule.pattern.as_deref() == Some(p)))
        });
    }
}

impl PermissionRule {
    /// Create a file operation rule
    pub fn file_rule(operation: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            pattern: Some(pattern.into()),
            reason: None,
        }
    }

    /// Create a bash command rule
    pub fn bash_rule(pattern: impl Into<String>) -> Self {
        Self {
            operation: "bash".to_string(),
            pattern: Some(pattern.into()),
            reason: None,
        }
    }

    /// Create a non-path operation rule (like web_search)
    pub fn global_rule(operation: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            pattern: None,
            reason: None,
        }
    }

    /// Add a reason to this rule
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Check if this rule matches the given target
    pub fn matches(&self, operation: &str, target: &str) -> bool {
        // Operation must match
        if self.operation != operation {
            return false;
        }

        // If no pattern, it's a global rule (matches everything)
        let Some(ref pattern_str) = self.pattern else {
            return true;
        };

        // For bash commands, do prefix/suffix matching
        if operation == "bash" {
            return self.matches_bash_pattern(pattern_str, target);
        }

        // For file operations, use glob matching
        self.matches_file_pattern(pattern_str, target)
    }

    fn matches_bash_pattern(&self, pattern: &str, command: &str) -> bool {
        // Handle wildcard patterns like "cargo build*"
        if pattern.ends_with('*') {
            let prefix = &pattern[..pattern.len() - 1];
            command.starts_with(prefix)
        } else {
            // Exact match
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
        assert!(!file.trusted);
        assert!(file.allow.is_empty());
        assert!(file.deny.is_empty());
    }

    #[test]
    fn test_serialize_deserialize() {
        let file = PermissionsFile {
            version: 1,
            trusted: false,
            allow: vec![
                PermissionRule::file_rule("write", "/src/**"),
                PermissionRule::bash_rule("cargo check"),
            ],
            deny: vec![
                PermissionRule::file_rule("delete", "/important/**")
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
        let rule = PermissionRule::file_rule("write", "/src/**");

        assert!(rule.matches("write", "/src/main.rs"));
        assert!(rule.matches("write", "/src/lib/mod.rs"));
        assert!(!rule.matches("write", "/tests/test.rs"));
        assert!(!rule.matches("read", "/src/main.rs")); // Wrong operation
    }

    #[test]
    fn test_bash_pattern_matching() {
        let rule = PermissionRule::bash_rule("cargo build*");

        assert!(rule.matches("bash", "cargo build"));
        assert!(rule.matches("bash", "cargo build --release"));
        assert!(!rule.matches("bash", "cargo check"));
        assert!(!rule.matches("bash", "npm build"));
    }

    #[test]
    fn test_exact_file_match() {
        let rule = PermissionRule::file_rule("write", "/config.toml");

        assert!(rule.matches("write", "/config.toml"));
        assert!(!rule.matches("write", "/src/config.toml"));
    }

    #[test]
    fn test_global_rule() {
        let rule = PermissionRule::global_rule("web_search");

        assert!(rule.matches("web_search", "anything"));
        assert!(rule.matches("web_search", ""));
        assert!(!rule.matches("read", "anything"));
    }

    #[test]
    fn test_json_format() {
        let file = PermissionsFile {
            version: 1,
            trusted: false,
            allow: vec![
                PermissionRule::file_rule("read", "/src/**"),
                PermissionRule::bash_rule("cargo check"),
                PermissionRule::global_rule("web_search"),
            ],
            deny: vec![],
        };

        let json = serde_json::to_string_pretty(&file).unwrap();

        assert!(json.contains(r#"allow"#));
        assert!(json.contains(r#"deny"#));
        assert!(json.contains(r#"pattern": "/src/**""#));
        assert!(json.contains(r#"operation": "web_search""#));
    }

    #[test]
    fn test_initial_permission_read_only() {
        let mut perms_file = PermissionsFile::default();
        perms_file.add_permission(PermissionRule::global_rule("read"), true);
        perms_file.add_permission(PermissionRule::global_rule("glob"), true);
        perms_file.add_permission(PermissionRule::global_rule("grep"), true);

        assert_eq!(perms_file.allow.len(), 3);
        assert!(!perms_file.trusted);
        assert!(perms_file.allow.iter().any(|r| r.operation == "read"));
        assert!(perms_file.allow.iter().any(|r| r.operation == "glob"));
        assert!(perms_file.allow.iter().any(|r| r.operation == "grep"));
    }

    #[test]
    fn test_initial_permission_trust_project() {
        let mut perms_file = PermissionsFile::default();
        perms_file.trusted = true;

        assert!(perms_file.trusted);
        assert!(perms_file.allow.is_empty());
    }
}
