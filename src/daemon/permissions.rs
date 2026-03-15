use anyhow::Result;
use std::path::Path;

use crate::permissions::storage::PermissionsFile;

pub struct PermissionResolver;

impl PermissionResolver {
    pub fn resolve(
        global: PermissionsFile,
        repo_level: Option<PermissionsFile>,
    ) -> PermissionsFile {
        let Some(repo) = repo_level else {
            return global;
        };

        let combined_deny: Vec<_> = global
            .deny
            .iter()
            .cloned()
            .chain(repo.deny.iter().cloned())
            .collect();

        let filtered_repo_allow: Vec<_> = repo
            .allow
            .into_iter()
            .filter(|r| !global.deny.iter().any(|d| d.operation == r.operation))
            .collect();

        let combined_allow: Vec<_> = global
            .allow
            .into_iter()
            .chain(filtered_repo_allow)
            .collect();

        PermissionsFile {
            version: global.version,
            allow: combined_allow,
            deny: combined_deny,
        }
    }

    pub fn load_global() -> Result<PermissionsFile> {
        let global_path = crate::config::AppConfig::global_permissions_path()
            .map_err(|e| anyhow::anyhow!("Could not determine permissions path: {}", e))?;

        if !global_path.exists() {
            return Ok(PermissionsFile::default());
        }

        let content = std::fs::read_to_string(&global_path)?;
        let perms: PermissionsFile = serde_json::from_str(&content)?;
        Ok(perms)
    }

    pub fn load_repo(repo_path: &Path) -> Option<PermissionsFile> {
        let perms_path = PermissionsFile::get_permissions_path(repo_path);
        if !perms_path.exists() {
            return None;
        }
        PermissionsFile::load_permissions_safe(repo_path).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::storage::PermissionRule;

    fn allow_rule(op: &str) -> PermissionRule {
        PermissionRule {
            operation: op.to_string(),
            pattern: None,
            reason: None,
        }
    }

    fn deny_rule(op: &str) -> PermissionRule {
        PermissionRule {
            operation: op.to_string(),
            pattern: None,
            reason: None,
        }
    }

    #[test]
    fn resolve_with_no_repo_returns_global() {
        let global = PermissionsFile {
            version: 1,
            allow: vec![allow_rule("read_file")],
            deny: vec![deny_rule("bash")],
        };

        let result = PermissionResolver::resolve(global.clone(), None);
        assert_eq!(result.allow.len(), 1);
        assert_eq!(result.deny.len(), 1);
        assert_eq!(result.allow[0].operation, "read_file");
    }

    #[test]
    fn resolve_merges_global_and_repo() {
        let global = PermissionsFile {
            version: 1,
            allow: vec![allow_rule("read_file")],
            deny: vec![],
        };
        let repo = PermissionsFile {
            version: 1,
            allow: vec![allow_rule("write_file")],
            deny: vec![deny_rule("bash")],
        };

        let result = PermissionResolver::resolve(global, Some(repo));
        assert_eq!(result.allow.len(), 2);
        assert_eq!(result.deny.len(), 1);
    }

    #[test]
    fn resolve_with_empty_global_and_repo_returns_merged() {
        let global = PermissionsFile::default();
        let repo = PermissionsFile {
            version: 1,
            allow: vec![allow_rule("read_file")],
            deny: vec![],
        };

        let result = PermissionResolver::resolve(global, Some(repo));
        assert_eq!(result.allow.len(), 1);
    }

    #[test]
    fn load_repo_returns_none_for_missing_dir() {
        let result = PermissionResolver::load_repo(Path::new("/nonexistent/path"));
        assert!(result.is_none());
    }
}
