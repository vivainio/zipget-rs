use anyhow::{Context, Result};
use std::path::Path;

/// Copy directory recursively
pub fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    // Re-export the implementation from archive::utils to avoid duplication
    crate::archive::utils::copy_dir_all(src, dst)
}

/// Check if a directory is in PATH
pub fn is_directory_in_path(directory: &Path) -> bool {
    // TODO: Implement PATH checking logic from main.rs
    println!("PATH checking not yet implemented in refactored version");
    println!("Directory: {}", directory.display());
    false
}
