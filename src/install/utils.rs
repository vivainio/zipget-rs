use anyhow::Result;
use std::path::Path;

/// Copy directory recursively
pub fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    // Re-export the implementation from archive::utils to avoid duplication
    crate::archive::utils::copy_dir_all(src, dst)
}

/// Check if a directory is in PATH
pub fn is_directory_in_path(directory: &Path) -> bool {
    if let Ok(path_var) = std::env::var("PATH") {
        let paths = std::env::split_paths(&path_var);
        for path in paths {
            if path == directory {
                return true;
            }
        }
    }
    false
}
