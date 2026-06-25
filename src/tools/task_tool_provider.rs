use std::path::PathBuf;
use std::sync::Arc;

use crate::backends::LlmBackend;
use crate::permissions::PermissionManager;
use crate::tools::{TaskTool, Tool, ToolProvider};

pub struct TaskToolProvider {
    backend: Arc<dyn LlmBackend>,
    working_directory: PathBuf,
    permission_manager: Arc<PermissionManager>,
}

impl TaskToolProvider {
    pub fn new(
        backend: Arc<dyn LlmBackend>,
        working_directory: PathBuf,
        permission_manager: Arc<PermissionManager>,
    ) -> Self {
        Self {
            backend,
            working_directory,
            permission_manager,
        }
    }
}

impl ToolProvider for TaskToolProvider {
    fn provide_tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![Arc::new(TaskTool::new(
            self.backend.clone(),
            self.working_directory.clone(),
            self.permission_manager.clone(),
        ))]
    }

    fn provider_name(&self) -> &'static str {
        "task_tool"
    }
}
