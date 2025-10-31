use anyhow::Result;
use std::path::Path;

use crate::Tool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPermissionDescriptor {
    kind: String,
    target: String,
    read_only: bool,
    is_write_safe: bool,
    is_destructive: bool,
    parent_directory: Option<String>,
    display_name: String,
    approval_title: String,
    approval_prompt: String,
    persistent_approval: String,
}

impl ToolPermissionDescriptor {
    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn is_write_safe(&self) -> bool {
        self.is_write_safe
    }

    pub fn is_destructive(&self) -> bool {
        self.is_destructive
    }

    pub fn parent_directory(&self) -> Option<&str> {
        self.parent_directory.as_deref()
    }

    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    pub fn approval_title(&self) -> &str {
        &self.approval_title
    }

    pub fn approval_prompt(&self) -> &str {
        &self.approval_prompt
    }

    pub fn persistent_approval(&self) -> &str {
        &self.persistent_approval
    }
}

pub struct ToolPermissionBuilder<'a> {
    tool: &'a dyn Tool,
    target: String,
    parent_directory: Option<String>,
    read_only: bool,
    is_write_safe: bool,
    is_destructive: bool,
    display_name: Option<String>,
    approval_title: Option<String>,
    approval_prompt: Option<String>,
    persistent_approval: Option<String>,
}

impl<'a> ToolPermissionBuilder<'a> {
    pub fn new(tool: &'a dyn Tool, target: String) -> Self {
        Self {
            tool,
            target,
            parent_directory: None,
            read_only: false,
            is_write_safe: false,
            is_destructive: false,
            display_name: None,
            approval_title: None,
            approval_prompt: None,
            persistent_approval: None,
        }
    }

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = target.into();
        self
    }

    pub fn with_target_path(mut self, path: &Path) -> Self {
        if let Some(path_str) = path.to_str() {
            self.target = path_str.to_string();
            self.parent_directory = path.parent().and_then(|p| p.to_str()).map(String::from);
        }
        self
    }

    pub fn into_read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    pub fn into_write_safe(mut self) -> Self {
        self.is_write_safe = true;
        self
    }

    pub fn into_destructive(mut self) -> Self {
        self.is_destructive = true;
        self
    }

    pub fn with_parent_directory(mut self, parent: impl Into<String>) -> Self {
        self.parent_directory = Some(parent.into());
        self
    }

    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    pub fn with_approval_title(mut self, title: impl Into<String>) -> Self {
        self.approval_title = Some(title.into());
        self
    }

    pub fn with_approval_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.approval_prompt = Some(prompt.into());
        self
    }

    pub fn with_persistent_approval(mut self, message: impl Into<String>) -> Self {
        self.persistent_approval = Some(message.into());
        self
    }

    pub fn build(self) -> Result<ToolPermissionDescriptor> {
        let kind = self.tool.tool_name().to_string();

        if self.target.is_empty() {
            return Err(anyhow::anyhow!("Target is required"));
        }

        let display_name = self
            .display_name
            .unwrap_or_else(|| capitalize(self.tool.tool_name()));

        let approval_title = self
            .approval_title
            .unwrap_or_else(|| format!("{} {}", display_name, self.target));

        let approval_prompt = self
            .approval_prompt
            .unwrap_or_else(|| format!("Can I {} {}", self.tool.display_name(), self.target));

        let persistent_approval = self.persistent_approval.unwrap_or_else(|| {
            let project_path = std::env::current_dir()
                .ok()
                .and_then(|p| p.to_str().map(String::from))
                .unwrap_or_else(|| "this project".to_string());

            format!(
                "don't ask me again for {} in {}",
                self.tool.display_name(),
                project_path
            )
        });

        Ok(ToolPermissionDescriptor {
            kind,
            target: self.target,
            read_only: self.read_only,
            is_write_safe: self.is_write_safe,
            is_destructive: self.is_destructive,
            parent_directory: self.parent_directory,
            display_name,
            approval_title,
            approval_prompt,
            persistent_approval,
        })
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ReadFileTool;
    use std::path::PathBuf;

    #[test]
    fn test_basic_construction_with_target() {
        let tool = ReadFileTool::new();
        let descriptor = ToolPermissionBuilder::new(&tool, "target".to_string())
            .with_target("test.txt")
            .build()
            .unwrap();

        assert_eq!(descriptor.kind(), "read_file");
        assert_eq!(descriptor.target(), "test.txt");
        assert!(!descriptor.is_read_only());
        assert!(!descriptor.is_destructive());
    }

    #[test]
    fn test_with_flags() {
        let tool = ReadFileTool::new();
        let descriptor = ToolPermissionBuilder::new(&tool, "target".to_string())
            .with_target("test.txt")
            .into_read_only()
            .into_destructive()
            .build()
            .unwrap();

        assert!(descriptor.is_read_only());
        assert!(descriptor.is_destructive());
    }

    #[test]
    fn test_with_path() {
        let tool = ReadFileTool::new();
        let path = PathBuf::from("/home/user/project/src/main.rs");
        let descriptor = ToolPermissionBuilder::new(&tool, "target".to_string())
            .with_target_path(&path)
            .into_read_only()
            .build()
            .unwrap();

        assert_eq!(descriptor.target(), "/home/user/project/src/main.rs");
        assert_eq!(
            descriptor.parent_directory(),
            Some("/home/user/project/src")
        );
    }

    #[test]
    fn test_custom_display() {
        let tool = ReadFileTool::new();
        let descriptor = ToolPermissionBuilder::new(&tool, "target".to_string())
            .with_target("target".to_string())
            .with_display_name("Custom")
            .with_approval_title("Custom Title")
            .with_approval_prompt("Custom Prompt?")
            .with_persistent_approval("Custom Persistent")
            .build()
            .unwrap();

        assert_eq!(descriptor.display_name(), "Custom");
        assert_eq!(descriptor.approval_title(), "Custom Title");
        assert_eq!(descriptor.approval_prompt(), "Custom Prompt?");
        assert_eq!(descriptor.persistent_approval(), "Custom Persistent");
    }

    #[test]
    fn test_default_display() {
        let tool = ReadFileTool::new();
        let descriptor = ToolPermissionBuilder::new(&tool, "target".to_string())
            .with_target("test.txt")
            .build()
            .unwrap();

        assert_eq!(descriptor.display_name(), "Read_file");
        assert!(descriptor.approval_title().contains("test.txt"));
        assert!(descriptor.approval_prompt().contains("readRead_file"));
    }

    #[test]
    fn test_fluent_chaining() {
        let tool = ReadFileTool::new();
        let descriptor = ToolPermissionBuilder::new(&tool, "target".to_string())
            .with_target("some_target")
            .into_read_only()
            .with_parent_directory("/parent/dir")
            .with_display_name("Complex")
            .with_approval_title("Complex Title")
            .with_approval_prompt("Complex Prompt")
            .with_persistent_approval("Complex Persistent")
            .build()
            .unwrap();

        assert_eq!(descriptor.kind(), "read_file");
        assert_eq!(descriptor.target(), "some_target");
        assert!(descriptor.is_read_only());
        assert_eq!(descriptor.parent_directory(), Some("/parent/dir"));
        assert_eq!(descriptor.display_name(), "Complex");
    }

    #[test]
    fn test_builder_is_independent() {
        let tool = ReadFileTool::new();
        let desc1 = ToolPermissionBuilder::new(&tool, "target".to_string())
            .with_target("t1")
            .build()
            .unwrap();
        let desc2 = ToolPermissionBuilder::new(&tool, "target".to_string().to_string())
            .with_target("t2")
            .build()
            .unwrap();

        assert_eq!(desc1.target(), "t1");
        assert_eq!(desc2.target(), "t2");
    }
}
