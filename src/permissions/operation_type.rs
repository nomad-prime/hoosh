use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationType {
    kind: String,
    target: String,
    read_only: bool,
    is_destructive: bool,
    parent_directory: Option<String>,
    display_name: String,
    approval_title: String,
    approval_prompt: String,
    persistent_approval: String,
}

impl OperationType {
    pub fn new(kind: impl Into<String>) -> Self {
        let kind = kind.into();
        Self {
            kind: kind.clone(),
            target: String::new(),
            read_only: false,
            is_destructive: false,
            parent_directory: None,
            display_name: capitalize(&kind),
            approval_title: String::new(),
            approval_prompt: String::new(),
            persistent_approval: String::new(),
        }
    }

    // Builder methods (consume and return self)

    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = target.into();
        self
    }

    pub fn with_target_path(mut self, path: &PathBuf) -> Self {
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

    pub fn into_destructive(mut self) -> Self {
        self.is_destructive = true;
        self
    }

    pub fn with_parent_directory(mut self, parent: impl Into<String>) -> Self {
        self.parent_directory = Some(parent.into());
        self
    }

    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = name.into();
        self
    }

    pub fn with_approval_title(mut self, title: impl Into<String>) -> Self {
        self.approval_title = title.into();
        self
    }

    pub fn with_approval_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.approval_prompt = prompt.into();
        self
    }

    pub fn with_persistent_approval(mut self, message: impl Into<String>) -> Self {
        self.persistent_approval = message.into();
        self
    }

    /// Finalize the operation type with defaults for any unset values
    pub fn build(mut self) -> anyhow::Result<Self> {
        if self.target.is_empty() {
            return Err(anyhow::anyhow!("Target is required"));
        }

        // Fill in defaults if not explicitly set
        if self.approval_title.is_empty() {
            self.approval_title = format!("{} {}", self.display_name, self.target);
        }

        if self.approval_prompt.is_empty() {
            self.approval_prompt = format!("Can I {} {}", self.kind.replace('_', " "), self.target);
        }

        if self.persistent_approval.is_empty() {
            let project_path = std::env::current_dir()
                .ok()
                .and_then(|p| p.to_str().map(String::from))
                .unwrap_or_else(|| "this project".to_string());

            self.persistent_approval = format!(
                "don't ask me again for {} in {}",
                self.kind.replace('_', " "),
                project_path
            );
        }

        Ok(self)
    }

    // Accessors

    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn is_destructive(&self) -> bool {
        self.is_destructive
    }

    pub fn parent_directory(&self) -> Option<&str> {
        self.parent_directory.as_deref()
    }

    pub fn display_name(&self) -> String {
        self.display_name.clone()
    }

    pub fn approval_title(&self) -> &String {
        &self.approval_title
    }

    pub fn approval_prompt(&self) -> &String {
        &self.approval_prompt
    }

    pub fn persistent_approval(&self) -> &String {
        &self.persistent_approval
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
    use std::path::PathBuf;

    #[test]
    fn test_basic_construction() {
        let op = OperationType::new("test_op")
            .with_target("target_file")
            .build()
            .unwrap();

        assert_eq!(op.kind(), "test_op");
        assert_eq!(op.target(), "target_file");
        assert!(!op.is_read_only());
        assert!(!op.is_destructive());
    }

    #[test]
    fn test_with_flags() {
        let op = OperationType::new("test_op")
            .with_target("target_file")
            .into_read_only()
            .into_destructive()
            .build()
            .unwrap();

        assert!(op.is_read_only());
        assert!(op.is_destructive());
    }

    #[test]
    fn test_with_path() {
        let path = PathBuf::from("/home/user/project/src/main.rs");
        let op = OperationType::new("read_file")
            .with_target_path(&path)
            .into_read_only()
            .build()
            .unwrap();

        assert_eq!(op.target(), "/home/user/project/src/main.rs");
        assert_eq!(op.parent_directory(), Some("/home/user/project/src"));
    }

    #[test]
    fn test_custom_display() {
        let op = OperationType::new("custom_op")
            .with_target("target")
            .with_display_name("Custom")
            .with_approval_title("Custom Title")
            .with_approval_prompt("Custom Prompt?")
            .with_persistent_approval("Custom Persistent")
            .build()
            .unwrap();

        assert_eq!(op.display_name, "Custom");
        assert_eq!(op.approval_title, "Custom Title");
        assert_eq!(op.approval_prompt, "Custom Prompt?");
        assert_eq!(op.persistent_approval, "Custom Persistent");
    }

    #[test]
    fn test_default_display() {
        let op = OperationType::new("read_file")
            .with_target("test.txt")
            .build()
            .unwrap();

        assert_eq!(op.display_name, "Read_file");
        assert!(op.approval_title.contains("test.txt"));
        assert!(op.approval_prompt.contains("read file"));
    }

    #[test]
    fn test_missing_target() {
        let result = OperationType::new("test_op").build();
        assert!(result.is_err());
    }

    #[test]
    fn test_fluent_chaining() {
        let op = OperationType::new("complex_op")
            .with_target("some_target")
            .into_read_only()
            .with_parent_directory("/parent/dir")
            .with_display_name("Complex")
            .with_approval_title("Complex Title")
            .with_approval_prompt("Complex Prompt")
            .with_persistent_approval("Complex Persistent")
            .build()
            .unwrap();

        assert_eq!(op.kind(), "complex_op");
        assert_eq!(op.target(), "some_target");
        assert!(op.is_read_only());
        assert_eq!(op.parent_directory(), Some("/parent/dir"));
        assert_eq!(op.display_name, "Complex");
    }

    #[test]
    fn test_no_naming_conflicts() {
        // Builder methods use with_* prefix
        let op = OperationType::new("test")
            .with_target("target")
            .with_parent_directory("parent")
            .build()
            .unwrap();

        // Accessors don't have with_* prefix
        assert_eq!(op.target(), "target");
        assert_eq!(op.parent_directory(), Some("parent"));
        assert_eq!(op.kind(), "test");
    }

    #[test]
    fn test_builder_is_self_contained() {
        let op1 = OperationType::new("op1").with_target("t1").build().unwrap();
        let op2 = OperationType::new("op2").with_target("t2").build().unwrap();

        assert_eq!(op1.target(), "t1");
        assert_eq!(op2.target(), "t2");
    }
}
