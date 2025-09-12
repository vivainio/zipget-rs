use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Get the cache directory for zipget
pub fn get_cache_dir() -> Result<PathBuf> {
    let temp_dir = std::env::temp_dir();
    let cache_dir = temp_dir.join("zipget-cache");
    fs::create_dir_all(&cache_dir)
        .with_context(|| format!("Failed to create cache directory: {}", cache_dir.display()))?;
    Ok(cache_dir)
}
