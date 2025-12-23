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
        if entry.file_type()?.is_dir()
            && let Some(dir_name) = entry.file_name().to_str()
        {
            return Ok(Some(dir_name.to_string()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_should_flatten_nonexistent_directory() {
        let result = should_flatten_directory(Path::new("/nonexistent/path/12345")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_should_flatten_single_subdir() {
        let temp = TempDir::new().unwrap();
        let subdir = temp.path().join("repo-master");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("file.txt"), "content").unwrap();

        let result = should_flatten_directory(temp.path()).unwrap();
        assert_eq!(result, Some("repo-master".to_string()));
    }

    #[test]
    fn test_should_flatten_multiple_items() {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join("dir1")).unwrap();
        fs::create_dir(temp.path().join("dir2")).unwrap();

        let result = should_flatten_directory(temp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_should_flatten_file_at_root() {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("file.txt"), "content").unwrap();

        let result = should_flatten_directory(temp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_should_flatten_mixed_content() {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join("subdir")).unwrap();
        fs::write(temp.path().join("file.txt"), "content").unwrap();

        let result = should_flatten_directory(temp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_copy_dir_all_simple() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();

        fs::write(src.path().join("file1.txt"), "content1").unwrap();
        fs::write(src.path().join("file2.txt"), "content2").unwrap();

        let dst_path = dst.path().join("copied");
        copy_dir_all(src.path(), &dst_path).unwrap();

        assert!(dst_path.join("file1.txt").exists());
        assert!(dst_path.join("file2.txt").exists());
        assert_eq!(
            fs::read_to_string(dst_path.join("file1.txt")).unwrap(),
            "content1"
        );
    }

    #[test]
    fn test_copy_dir_all_nested() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();

        let nested = src.path().join("subdir").join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("deep.txt"), "deep content").unwrap();

        let dst_path = dst.path().join("copied");
        copy_dir_all(src.path(), &dst_path).unwrap();

        assert!(
            dst_path
                .join("subdir")
                .join("nested")
                .join("deep.txt")
                .exists()
        );
    }

    #[test]
    fn test_flatten_directory_structure() {
        let temp = TempDir::new().unwrap();
        let subdir = temp.path().join("repo-v1.0");
        fs::create_dir(&subdir).unwrap();
        fs::write(subdir.join("README.md"), "readme").unwrap();
        fs::create_dir(subdir.join("src")).unwrap();
        fs::write(subdir.join("src").join("main.rs"), "fn main() {}").unwrap();

        flatten_directory_structure(temp.path(), "repo-v1.0").unwrap();

        // After flattening, files should be at root level
        assert!(temp.path().join("README.md").exists());
        assert!(temp.path().join("src").join("main.rs").exists());
        // Original subdir should be gone
        assert!(!temp.path().join("repo-v1.0").exists());
    }
}
