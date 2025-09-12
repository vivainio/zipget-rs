use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use flate2::read::GzDecoder;
use tar::Archive;
use glob_match::glob_match;

/// Extract TAR.GZ archive
pub fn extract_tar_gz(tar_path: &Path, extract_to: &str, file_pattern: Option<&str>) -> Result<()> {
    let file = fs::File::open(tar_path)
        .with_context(|| format!("Failed to open tar.gz file: {}", tar_path.display()))?;

    let tar = GzDecoder::new(file);
    let mut archive = Archive::new(tar);

    fs::create_dir_all(extract_to)
        .with_context(|| format!("Failed to create extraction directory: {extract_to}"))?;

    let mut extracted_count = 0;

    for entry in archive
        .entries()
        .with_context(|| "Failed to read tar.gz entries")?
    {
        let mut entry = entry.with_context(|| "Failed to access tar.gz entry")?;

        let path = entry.path().with_context(|| "Failed to get entry path")?;

        let path_str = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in path"))?;

        // Check if file matches the glob pattern (if specified)
        if let Some(pattern) = file_pattern {
            let filename = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if !glob_match(pattern, path_str) && !glob_match(pattern, filename) {
                continue; // Skip files that don't match the pattern (checking both full path and filename)
            }
        }

        let outpath = Path::new(extract_to).join(&path);

        // Create parent directories if they don't exist
        if let Some(parent) = outpath.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create parent directory: {}", parent.display())
                })?;
            }
        }

        // Extract the entry
        entry
            .unpack(&outpath)
            .with_context(|| format!("Failed to extract file: {}", outpath.display()))?;

        extracted_count += 1;
    }

    if let Some(pattern) = file_pattern {
        println!("Extracted {extracted_count} files matching pattern '{pattern}'");
    } else {
        println!("Extracted {extracted_count} files");
    }
    Ok(())
}
