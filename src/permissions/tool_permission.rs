use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

use crate::permissions::PatternMatcher;
use crate::Tool;

#[derive(Clone)]
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
    suggested_pattern: Option<String>,
    pattern_matcher: Arc<dyn PatternMatcher>,
}

// Manual Debug implementation since PatternMatcher doesn't implement Debug
impl std::fmt::Debug for ToolPermissionDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolPermissionDescriptor")
            .field("kind", &self.kind)
            .field("target", &self.target)
            .field("read_only", &self.read_only)
            .field("is_write_safe", &self.is_write_safe)
            .field("is_destructive", &self.is_destructive)
            .field("parent_directory", &self.parent_directory)
            .field("display_name", &self.display_name)
            .field("approval_title", &self.approval_title)
            .field("approval_prompt", &self.approval_prompt)
            .field("persistent_approval", &self.persistent_approval)
            .field("suggested_pattern", &self.suggested_pattern)
            .field("pattern_matcher", &"<PatternMatcher>")
            .finish()
    }
}

// Manual PartialEq implementation since PatternMatcher doesn't implement PartialEq
impl PartialEq for ToolPermissionDescriptor {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.target == other.target
            && self.read_only == other.read_only
            && self.is_write_safe == other.is_write_safe
            && self.is_destructive == other.is_destructive
            && self.parent_directory == other.parent_directory
            && self.display_name == other.display_name
            && self.approval_title == other.approval_title
            && self.approval_prompt == other.approval_prompt
            && self.persistent_approval == other.persistent_approval
            && self.suggested_pattern == other.suggested_pattern
    }
}

impl Eq for ToolPermissionDescriptor {}

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

    pub fn suggested_pattern(&self) -> Option<&str> {
        self.suggested_pattern.as_deref()
    }

    /// Check if a pattern matches this descriptor's target
    /// Delegates to the tool-specific pattern matcher
    pub fn matches_pattern(&self, pattern: &str) -> bool {
        self.pattern_matcher.matches(pattern, &self.target)
    }
}

pub struct ToolPermissionBuilder<'a> {
    tool: &'a dyn Tool,
    parent_directory: Option<String>,
    target: String,
    read_only: bool,
    is_write_safe: bool,
    is_destructive: bool,
    display_name: Option<String>,
    approval_title: Option<String>,
    approval_prompt: Option<String>,
    persistent_approval: Option<String>,
    suggested_pattern: Option<String>,
    pattern_matcher: Option<Arc<dyn PatternMatcher>>,
}

impl<'a> ToolPermissionBuilder<'a> {
    pub fn new(tool: &'a dyn Tool, target: impl Into<String>) -> Self {
        Self {
            tool,
            target: target.into(),
            parent_directory: None,
            read_only: false,
            is_write_safe: false,
            is_destructive: false,
            display_name: None,
            approval_title: None,
            approval_prompt: None,
            persistent_approval: None,
            suggested_pattern: None,
            pattern_matcher: None,
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

    pub fn with_suggested_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.suggested_pattern = Some(pattern.into());
        self
    }

    pub fn with_pattern_matcher(mut self, matcher: Arc<dyn PatternMatcher>) -> Self {
        self.pattern_matcher = Some(matcher);
        self
    }

    pub fn build(self) -> Result<ToolPermissionDescriptor> {
        let kind = self.tool.name().to_string();

        if self.target.is_empty() {
            return Err(anyhow::anyhow!("Target is required"));
        }

        let display_name = self
            .display_name
            .unwrap_or_else(|| capitalize(self.tool.name()));

        let approval_title = self
            .approval_title
            .unwrap_or_else(|| format!(" {} ", display_name));

        let approval_prompt = self.approval_prompt.unwrap_or_else(|| {
            format!("Can I \"{}\" \"{}\"", self.tool.display_name(), self.target)
        });

        let persistent_approval = self.persistent_approval.unwrap_or_else(|| {
            let project_path = std::env::current_dir()
                .ok()
                .and_then(|p| p.to_str().map(String::from))
                .unwrap_or_else(|| "this project".to_string());

            format!(
                "don't ask me again for \"{}\" in \"{}\"",
                self.tool.display_name(),
                project_path
            )
        });

        // Default to FilePatternMatcher if no matcher provided
        let pattern_matcher = self.pattern_matcher.unwrap_or_else(|| {
            use crate::permissions::FilePatternMatcher;
            Arc::new(FilePatternMatcher)
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
            suggested_pattern: self.suggested_pattern,
            pattern_matcher,
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
        assert!(descriptor.approval_prompt().contains("read"));
        assert!(descriptor.approval_prompt().contains("test.txt"));
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
