### 4. Benefits

- **Open/Closed Principle** - can add new providers without modifying registry
- **Extensible** - supports future MCP tools, custom tools, plugin tools
- **Loose coupling** - registry doesn't depend on specific tool implementations
- **Testable** - easy to inject mock tool providers
- **Dynamic tools** - can refresh tools at runtime
- **Clear separation** - tool discovery vs tool execution logic

---

# Architecture: Simplify Backend Factory with Strategy Pattern

## Problem

The backend creation logic is scattered and complex:

In `src/main.rs`:

```rust
fn create_backend(backend_name: &str, config: &AppConfig) -> Result<Box<dyn LlmBackend>> {
    let backend_config = config.get_backend_config(backend_name)?;

    match backend_name {
        "mock" => Ok(Box::new(MockBackend::new())),
        #[cfg(feature = "together-ai")]
        "together_ai" => TogetherAiBackend::create(backend_config, backend_name),
        #[cfg(feature = "anthropic")]
        "anthropic" => AnthropicBackend::create(backend_config, backend_name),
        #[cfg(feature = "openai-compatible")]
        name if matches!(name, "openai" | "groq" | "ollama" | ...) => {
            OpenAICompatibleBackend::create(backend_config, backend_name)
        }
        _ => Err(anyhow::anyhow!("Unknown backend: {}", backend_name)),
    }
}
```

Issues:

- **Feature gate complexity** - multiple `#[cfg(feature)]` attributes scattered throughout
- **Pattern matching complexity** - `matches!` macro with multiple backend names
- **Hardcoded backend names** - adding new backends requires code changes
- **No centralization** - backend creation logic mixed with application logic
- **Difficult to test** - can't easily mock backend creation
- **No validation** - backend names not validated until runtime
- **Backend-specific logic in main** - violates single responsibility

The existing `BackendFactory` exists but isn't fully utilized.

## Proposed Solution

### 1. Define a BackendProvider trait

Create `src/backends/provider.rs`:

```rust
/// Trait for backend providers that can create backend instances
pub trait BackendProvider: Send + Sync {
    /// The name(s) this provider handles (e.g., "anthropic", "openai")
    fn supported_names(&self) -> Vec<&'static str>;

    /// Check if this provider can handle the given backend name
    fn supports(&self, name: &str) -> bool {
        self.supported_names().contains(&name)
    }

    /// Create a backend instance with the given configuration
    fn create_backend(
        &self,
        name: &str,
        config: &BackendConfig,
    ) -> Result<Box<dyn LlmBackend>, BackendError>;

    /// Get default configuration for this backend
    fn default_config(&self) -> BackendConfig {
        BackendConfig {
            api_key: None,
            model: None,
            base_url: None,
            temperature: Some(0.7),
        }
    }
}
```

### 2. Implement providers for each backend type

```rust
#[cfg(feature = "anthropic")]
pub struct AnthropicProvider;

#[cfg(feature = "anthropic")]
impl BackendProvider for AnthropicProvider {
    fn supported_names(&self) -> Vec<&'static str> {
        vec!["anthropic", "claude"]
    }

    fn create_backend(
        &self,
        name: &str,
        config: &BackendConfig,
    ) -> Result<Box<dyn LlmBackend>, BackendError> {
        AnthropicBackend::create(config, name)
            .map_err(|e| BackendError::CreationFailed {
                backend: name.to_string(),
                reason: e.to_string(),
            })
    }

    fn default_config(&self) -> BackendConfig {
        BackendConfig {
            api_key: None,
            model: Some("claude-sonnet-4.5".to_string()),
            base_url: Some("https://api.anthropic.com".to_string()),
            temperature: Some(0.7),
        }
    }
}

#[cfg(feature = "openai-compatible")]
pub struct OpenAICompatibleProvider;

#[cfg(feature = "openai-compatible")]
impl BackendProvider for OpenAICompatibleProvider {
    fn supported_names(&self) -> Vec<&'static str> {
        vec!["openai", "groq", "ollama", "deepseek", "together_ai"]
    }

    fn create_backend(
        &self,
        name: &str,
        config: &BackendConfig,
    ) -> Result<Box<dyn LlmBackend>, BackendError> {
        OpenAICompatibleBackend::create(config, name)
            .map_err(|e| BackendError::CreationFailed {
                backend: name.to_string(),
                reason: e.to_string(),
            })
    }
}

pub struct MockProvider;

impl BackendProvider for MockProvider {
    fn supported_names(&self) -> Vec<&'static str> {
        vec!["mock"]
    }

    fn create_backend(
        &self,
        _name: &str,
        _config: &BackendConfig,
    ) -> Result<Box<dyn LlmBackend>, BackendError> {
        Ok(Box::new(MockBackend::new()))
    }
}
```

### 3. Refactor BackendFactory to use providers

```rust
pub struct BackendFactory {
    providers: Vec<Box<dyn BackendProvider>>,
}

impl BackendFactory {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn with_provider(mut self, provider: Box<dyn BackendProvider>) -> Self {
        self.providers.push(provider);
        self
    }

    pub fn create(
        &self,
        name: &str,
        config: &BackendConfig,
    ) -> Result<Box<dyn LlmBackend>, BackendError> {
        for provider in &self.providers {
            if provider.supports(name) {
                return provider.create_backend(name, config);
            }
        }

        Err(BackendError::BackendNotFound {
            backend: name.to_string(),
        })
    }

    pub fn list_supported_backends(&self) -> Vec<&'static str> {
        self.providers
            .iter()
            .flat_map(|p| p.supported_names())
            .collect()
    }
}

impl Default for BackendFactory {
    fn default() -> Self {
        let mut factory = Self::new()
            .with_provider(Box::new(MockProvider));

        #[cfg(feature = "anthropic")]
        {
            factory = factory.with_provider(Box::new(AnthropicProvider));
        }

        #[cfg(feature = "openai-compatible")]
        {
            factory = factory.with_provider(Box::new(OpenAICompatibleProvider));
        }

        #[cfg(feature = "together-ai")]
        {
            factory = factory.with_provider(Box::new(TogetherAiProvider));
        }

        factory
    }
}
```

### 4. Simplify main.rs

```rust
fn create_backend(backend_name: &str, config: &AppConfig) -> Result<Box<dyn LlmBackend>> {
    let backend_config = config
        .get_backend_config(backend_name)
        .ok_or_else(|| BackendError::BackendNotFound {
            backend: backend_name.to_string(),
        })?;

    let factory = BackendFactory::default();
    factory.create(backend_name, backend_config)
        .map_err(|e| anyhow::anyhow!(e))
}
```

### 5. Benefits

- **No feature gates in main.rs** - all conditional compilation in factory
- **Easy to add backends** - just implement `BackendProvider` trait
- **Centralized creation logic** - all in `BackendFactory`
- **Testable** - can inject mock providers
- **Discoverable** - can list all supported backends
- **Configuration defaults** - each provider defines its own defaults
- **Clear errors** - specific error types for backend creation failures

---

# Architecture: Refactor Permission System Cache

## Problem

The `PermissionManager::check_cache()` method has complex hierarchical permission checking with multiple cache lookup
strategies:

```rust
fn check_cache(&self, operation: &OperationType) -> Option<bool> {
    let cache = self.session_cache.lock().ok()?;

    // 0. Check project-wide permissions (string parsing)
    for (key, &decision) in cache.iter() {
        if key.starts_with("project:") {
            if let Some(project_path_str) = key.strip_prefix("project:").and_then(|s| s.strip_suffix(":*")) {
                // Complex path canonicalization and comparison
            }
        }
    }

    // 1. Check specific file permissions (more string parsing)
    let specific_key = format!("{}:specific:{}", kind, target);
    if let Some(&decision) = cache.get(&specific_key) { ... }

    // 2. Check directory permissions (string construction)
    if let Some(parent) = Path::new(target).parent() {
        let dir_key = format!("{}:dir:{}", kind, parent.display());
        if let Some(&decision) = cache.get(&dir_key) { ... }
    }

    // 3. Check global permissions (string construction)
    let global_key = format!("{}:*", kind);
    if let Some(&decision) = cache.get(&global_key) { ... }
}
```

Issues:

- **String-based cache keys** - error-prone string parsing and construction
- **Linear search** - iterates through entire cache for project-wide checks
- **Complex logic** - nested loops, multiple string operations, path canonicalization
- **Performance bottleneck** - called frequently, does expensive operations
- **Hard to maintain** - complex string parsing logic scattered throughout
- **Difficult to test** - many edge cases in string handling
- **No clear hierarchy** - permission precedence is implicit in code order

## Proposed Solution

### 1. Define structured cache key types

Create `src/permissions/cache.rs`:

```rust
use std::path::PathBuf;

/// Structured cache key instead of string-based keys
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PermissionCacheKey {
    /// Project-wide permission: applies to all operations in a project
    ProjectWide {
        operation: OperationKind,
        project_root: PathBuf,
    },
    /// Specific file/target permission
    Specific {
        operation: OperationKind,
        target: PathBuf,
    },
    /// Directory-level permission: applies to all files in directory
    Directory {
        operation: OperationKind,
        directory: PathBuf,
    },
    /// Global permission: applies to all targets for this operation type
    Global {
        operation: OperationKind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationKind {
    ReadFile,
    WriteFile,
    EditFile,
    Bash,
}

impl PermissionCacheKey {
    /// Get the precedence order (higher number = higher precedence)
    pub fn precedence(&self) -> u8 {
        match self {
            Self::ProjectWide { .. } => 4,
            Self::Specific { .. } => 3,
            Self::Directory { .. } => 2,
            Self::Global { .. } => 1,
        }
    }

    /// Check if this key matches the given operation
    pub fn matches(&self, operation: &OperationType) -> bool {
        match self {
            Self::ProjectWide { operation: op, project_root } => {
                op == &operation.operation_kind()
                    && Self::is_within_project(operation.target(), project_root)
            }
            Self::Specific { operation: op, target } => {
                op == &operation.operation_kind()
                    && Self::paths_equal(operation.target(), target)
            }
            Self::Directory { operation: op, directory } => {
                op == &operation.operation_kind()
                    && Self::is_in_directory(operation.target(), directory)
            }
            Self::Global { operation: op } => {
                op == &operation.operation_kind()
            }
        }
    }

    fn is_within_project(target: &str, project_root: &PathBuf) -> bool {
        // Centralized path comparison logic
        let target_path = PathBuf::from(target);
        target_path.canonicalize()
            .and_then(|t| project_root.canonicalize().map(|p| (t, p)))
            .map(|(t, p)| t.starts_with(p))
            .unwrap_or(false)
    }

    fn paths_equal(target: &str, cached: &PathBuf) -> bool {
        PathBuf::from(target).canonicalize()
            .and_then(|t| cached.canonicalize().map(|c| t == c))
            .unwrap_or(false)
    }

    fn is_in_directory(target: &str, directory: &PathBuf) -> bool {
        Path::new(target)
            .parent()
            .and_then(|p| p.canonicalize().ok())
            .and_then(|p| directory.canonicalize().ok().map(|d| p == d))
            .unwrap_or(false)
    }
}
```

### 2. Refactor PermissionManager to use structured cache

```rust
pub struct PermissionManager {
    session_cache: Arc<Mutex<HashMap<PermissionCacheKey, bool>>>,
    skip_permissions: bool,
}

impl PermissionManager {
    fn check_cache(&self, operation: &OperationType) -> Option<bool> {
        let cache = self.session_cache.lock().ok()?;

        // Collect all matching cache entries with their precedence
        let mut matches: Vec<(u8, bool)> = cache
            .iter()
            .filter(|(key, _)| key.matches(operation))
            .map(|(key, &decision)| (key.precedence(), decision))
            .collect();

        // Sort by precedence (highest first)
        matches.sort_by(|a, b| b.0.cmp(&a.0));

        // Return the decision from the highest precedence match
        matches.first().map(|(_, decision)| *decision)
    }

    fn store_permission(&self, key: PermissionCacheKey, decision: bool) {
        if let Ok(mut cache) = self.session_cache.lock() {
            cache.insert(key, decision);
        }
    }
}
```

### 3. Update permission storage calls

Before:

```rust
let key = format!("{}:specific:{}", operation.operation_kind(), target);
cache.insert(key, decision);
```

After:

```rust
let key = PermissionCacheKey::Specific {
operation: operation.operation_kind(),
target: PathBuf::from(target),
};
self .store_permission(key, decision);
```

### 4. Benefits

- **Type-safe cache keys** - no string parsing errors
- **O(n) with early exit** - efficient matching with precedence
- **Clear hierarchy** - precedence explicitly defined in enum
- **Centralized path logic** - all path comparisons in one place
- **Easy to test** - can test each key type independently
- **Better performance** - no string allocations during lookup
- **Maintainable** - adding new permission types is straightforward
- **Self-documenting** - enum variants make permission types clear

---

# Refactoring: Extract Common File Operation Logic

## Problem

Each file operation tool (ReadFileTool, WriteFileTool, EditFileTool, ListDirectoryTool) duplicates significant amounts
of code:

**Duplicated patterns across all tools:**

1. **Path resolution logic** (4 duplicates):

```rust
fn resolve_path(&self, path: &str) -> PathBuf {
    if path.is_empty() || path == "." {
        return self.working_directory.clone();
    }
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        self.working_directory.join(path)
    }
}
```

2. **Security validation** (4 duplicates):

```rust
let canonical_path = path.canonicalize()
.context("Failed to resolve path") ?;
let canonical_working = self .working_directory.canonicalize()
.context("Failed to resolve working directory") ?;
if ! canonical_path.starts_with( & canonical_working) {
anyhow::bail ! ("Access denied: cannot access files outside working directory");
}
```

3. **Working directory management** (4 duplicates):

```rust
pub struct SomeTool {
    working_directory: PathBuf,
}

impl SomeTool {
    pub fn new() -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    pub fn with_working_directory(working_dir: PathBuf) -> Self {
        Self {
            working_directory: working_dir,
        }
    }
}
```

4. **Similar error handling patterns** (4 duplicates with slight variations)

**Impact:**

- **~100-150 lines of duplicated code** across 4 files
- **Bug fixes must be applied 4 times** (easy to miss one)
- **Inconsistent behavior** - slight variations in implementation
- **Testing overhead** - same logic tested multiple times
- **Maintenance burden** - changes ripple across multiple files

## Proposed Solution

### 1. Create a FileSystemService for shared functionality

Create `src/tools/file_ops/fs_service.rs`:

```rust
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

/// Shared file system operations for all file tools
pub struct FileSystemService {
    working_directory: PathBuf,
}

impl FileSystemService {
    pub fn new() -> Self {
        Self {
            working_directory: std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    pub fn with_working_directory(working_dir: PathBuf) -> Self {
        Self {
            working_directory: working_dir,
        }
    }

    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }

    /// Resolve a path relative to the working directory
    pub fn resolve_path(&self, path: &str) -> PathBuf {
        if path.is_empty() || path == "." {
            return self.working_directory.clone();
        }

        let path = Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.working_directory.join(path)
        }
    }

    /// Validate that a path is within the working directory (security check)
    pub fn validate_path_security(&self, path: &Path) -> Result<PathBuf> {
        let canonical_path = path.canonicalize()
            .with_context(|| format!("Failed to resolve path: {}", path.display()))?;

        let canonical_working = self.working_directory.canonicalize()
            .with_context(|| format!(
                "Failed to resolve working directory: {}",
                self.working_directory.display()
            ))?;

        if !canonical_path.starts_with(&canonical_working) {
            anyhow::bail!(
                "Access denied: cannot access files outside working directory\n\
                 Attempted: {}\n\
                 Working directory: {}",
                canonical_path.display(),
                canonical_working.display()
            );
        }

        Ok(canonical_path)
    }

    /// Resolve and validate a path (combines the two operations)
    pub fn resolve_and_validate(&self, path: &str) -> Result<PathBuf> {
        let resolved = self.resolve_path(path);
        self.validate_path_security(&resolved)
    }

    /// Check if a path exists
    pub fn path_exists(&self, path: &Path) -> bool {
        path.exists()
    }

    /// Get parent directory of a path
    pub fn parent_directory(&self, path: &Path) -> Option<PathBuf> {
        path.parent().map(|p| p.to_path_buf())
    }

    /// Create parent directories if they don't exist
    pub async fn ensure_parent_exists(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await
                .with_context(|| format!("Failed to create parent directory: {}", parent.display()))?;
        }
        Ok(())
    }
}
```

### 2. Refactor ReadFileTool to use FileSystemService

Before (~80 lines):

```rust
pub struct ReadFileTool {
    working_directory: PathBuf,
}

impl ReadFileTool {
    pub fn new() -> Self { ... }
    pub fn with_working_directory(working_dir: PathBuf) -> Self { ... }
    fn resolve_path(&self, file_path: &str) -> PathBuf { ... }
}

#[async_trait]
impl Tool for ReadFileTool {
    async fn execute(&self, args: &Value) -> Result<String> {
        let args: ReadFileArgs = serde_json::from_value(args.clone())?;
        let file_path = self.resolve_path(&args.file_path);

        // Security validation
        let canonical_path = file_path.canonicalize()?;
        let canonical_working = self.working_directory.canonicalize()?;
        if !canonical_path.starts_with(&canonical_working) {
            anyhow::bail!("Access denied...");
        }

        // Read and handle line ranges
        let content = fs::read_to_string(&canonical_path).await?;
        // ... line range handling logic
    }
}
```

After (~40 lines):

```rust
pub struct ReadFileTool {
    fs_service: FileSystemService,
}

impl ReadFileTool {
    pub fn new() -> Self {
        Self {
            fs_service: FileSystemService::new(),
        }
    }

    pub fn with_working_directory(working_dir: PathBuf) -> Self {
        Self {
            fs_service: FileSystemService::with_working_directory(working_dir),
        }
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    async fn execute(&self, args: &Value) -> Result<String> {
        let args: ReadFileArgs = serde_json::from_value(args.clone())?;

        // Single line for resolution + validation
        let file_path = self.fs_service.resolve_and_validate(&args.file_path)?;

        // Tool-specific logic only
        let content = fs::read_to_string(&file_path).await?;
        // ... line range handling logic
    }
}
```

### 3. Apply same pattern to other tools

**WriteFileTool** - use `fs_service.ensure_parent_exists()`  
**EditFileTool** - use `fs_service.resolve_and_validate()`  
**ListDirectoryTool** - use `fs_service` for directory operations

### 4. Update tool creation

In `src/tool_executor.rs`:

```rust
pub fn create_tool_registry_with_working_dir(working_dir: PathBuf) -> ToolRegistry {
    let fs_service = FileSystemService::with_working_directory(working_dir.clone());

    ToolRegistry::new()
        .with_tool(Arc::new(ReadFileTool::with_fs_service(fs_service.clone())))
        .with_tool(Arc::new(WriteFileTool::with_fs_service(fs_service.clone())))
        .with_tool(Arc::new(EditFileTool::with_fs_service(fs_service.clone())))
        .with_tool(Arc::new(ListDirectoryTool::with_fs_service(fs_service.clone())))
        .with_tool(Arc::new(BashTool::new()))
}
```

### 5. Benefits

- **~100-150 lines of code eliminated** through consolidation
- **Single source of truth** for file operations
- **Consistent security validation** across all tools
- **Easier to test** - test FileSystemService once thoroughly
- **Simplified tools** - focus on tool-specific logic
- **Better maintainability** - changes in one place
- **Potential for caching** - could add path canonicalization cache in service
- **Future extensibility** - easy to add new file system operations

