use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Check if directory should be flattened (contains only one subdirectory)
pub fn should_flatten_directory(extract_to: &Path) -> Result<Option<String>> {
    if !extract_to.exists() {
        return Ok(None);
    }

    let entries: Vec<_> = fs::read_dir(extract_to)?.collect::<Result<Vec<_>, _>>()?;

    // Check if there's exactly one directory and no files at the top level
    if entries.len() == 1 {
        let entry = &entries[0];
        if entry.file_type()?.is_dir() {
            if let Some(dir_name) = entry.file_name().to_str() {
                return Ok(Some(dir_name.to_string()));
            }
        }
    }

    Ok(None)
}

/// Flatten directory structure by moving contents up one level
pub fn flatten_directory_structure(extract_to: &Path, single_dir_name: &str) -> Result<()> {
    let single_dir_path = extract_to.join(single_dir_name);

    // Create a temporary directory to move files through
    let temp_dir = extract_to.join(format!("_zipget_temp_{}", std::process::id()));
    fs::create_dir_all(&temp_dir)?;

    // Move all contents from the single directory to the temp directory
    for entry in fs::read_dir(&single_dir_path)? {
        let entry = entry?;
        let source = entry.path();
        let dest = temp_dir.join(entry.file_name());

        if source.is_dir() {
            copy_dir_all(&source, &dest)?;
        } else {
            fs::copy(&source, &dest)?;
        }
    }

    // Remove the original single directory
    fs::remove_dir_all(&single_dir_path)?;

    // Move all contents from temp directory back to the extraction directory
    for entry in fs::read_dir(&temp_dir)? {
        let entry = entry?;
        let source = entry.path();
        let dest = extract_to.join(entry.file_name());

        if source.is_dir() {
            copy_dir_all(&source, &dest)?;
        } else {
            fs::copy(&source, &dest)?;
        }
    }

    // Remove the temporary directory
    fs::remove_dir_all(&temp_dir)?;

    println!("Flattened directory structure: removed top-level '{single_dir_name}' directory");
    Ok(())
}

/// Copy directory recursively
pub fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)
        .with_context(|| format!("Failed to create destination directory: {}", dst.display()))?;

    for entry in fs::read_dir(src)
        .with_context(|| format!("Failed to read source directory: {}", src.display()))?
    {
        let entry = entry.with_context(|| "Failed to read directory entry")?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "Failed to copy {} to {}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
        }
    }

    Ok(())
}
