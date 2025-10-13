use std::path::{Path, PathBuf};

pub fn resolve_path(file_path: &str, working_directory: &Path) -> PathBuf {
    let path = Path::new(file_path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        working_directory.join(path)
    }
}
