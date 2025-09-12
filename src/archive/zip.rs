use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use zip::ZipArchive;
use glob_match::glob_match;

/// Extract ZIP archive
pub fn extract_zip(zip_path: &Path, extract_to: &str, file_pattern: Option<&str>) -> Result<()> {
    let file = fs::File::open(zip_path)
        .with_context(|| format!("Failed to open zip file: {}", zip_path.display()))?;

    let mut archive = ZipArchive::new(file).with_context(|| "Failed to read zip archive")?;

    fs::create_dir_all(extract_to)
        .with_context(|| format!("Failed to create extraction directory: {extract_to}"))?;

    let mut extracted_count = 0;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .with_context(|| format!("Failed to access zip entry {i}"))?;

        // Check if file matches the glob pattern (if specified)
        if let Some(pattern) = file_pattern {
            let filename = Path::new(file.name())
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if !glob_match(pattern, file.name()) && !glob_match(pattern, filename) {
                continue; // Skip files that don't match the pattern (checking both full path and filename)
            }
        }

        let outpath = Path::new(extract_to).join(file.mangled_name());

        if file.name().ends_with('/') {
            // Directory
            fs::create_dir_all(&outpath)
                .with_context(|| format!("Failed to create directory: {}", outpath.display()))?;
        } else {
            // File
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p).with_context(|| {
                        format!("Failed to create parent directory: {}", p.display())
                    })?;
                }
            }

            let mut outfile = fs::File::create(&outpath).with_context(|| {
                format!("Failed to create extracted file: {}", outpath.display())
            })?;

            std::io::copy(&mut file, &mut outfile)
                .with_context(|| format!("Failed to extract file: {}", outpath.display()))?;
        }

        // Set file permissions on Unix-like systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))?;
            }
        }

        extracted_count += 1;
    }

    if let Some(pattern) = file_pattern {
        println!("Extracted {extracted_count} files matching pattern '{pattern}'");
    } else {
        println!("Extracted {extracted_count} files");
    }
    Ok(())
}
