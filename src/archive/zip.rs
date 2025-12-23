use anyhow::{Context, Result};
use glob_match::glob_match;
use std::fs;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

/// Extract ZIP archive, returns list of extracted file paths
pub fn extract_zip(
    zip_path: &Path,
    extract_to: &str,
    file_pattern: Option<&str>,
) -> Result<Vec<PathBuf>> {
    let file = fs::File::open(zip_path)
        .with_context(|| format!("Failed to open zip file: {}", zip_path.display()))?;

    let mut archive = ZipArchive::new(file).with_context(|| "Failed to read zip archive")?;

    fs::create_dir_all(extract_to)
        .with_context(|| format!("Failed to create extraction directory: {extract_to}"))?;

    let mut extracted_files = Vec::new();

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .with_context(|| format!("Failed to access zip entry {i}"))?;

        // Check if file matches the glob pattern (if specified)
        let flatten = if let Some(pattern) = file_pattern {
            let filename = Path::new(file.name())
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if !glob_match(pattern, file.name()) && !glob_match(pattern, filename) {
                continue; // Skip files that don't match the pattern (checking both full path and filename)
            }
            true // Flatten when file pattern is specified
        } else {
            false
        };

        // Skip directories when flattening
        if file.name().ends_with('/') {
            if flatten {
                continue; // Skip directory entries when flattening
            }
            // Directory (only when not flattening)
            let outpath = Path::new(extract_to).join(file.mangled_name());
            fs::create_dir_all(&outpath)
                .with_context(|| format!("Failed to create directory: {}", outpath.display()))?;
        } else {
            // When file pattern is specified, flatten to just the filename
            let outpath = if flatten {
                if let Some(filename) = Path::new(file.name()).file_name() {
                    Path::new(extract_to).join(filename)
                } else {
                    continue; // Skip entries without a filename
                }
            } else {
                Path::new(extract_to).join(file.mangled_name())
            };
            // File
            if let Some(p) = outpath.parent()
                && !p.exists()
            {
                fs::create_dir_all(p).with_context(|| {
                    format!("Failed to create parent directory: {}", p.display())
                })?;
            }

            let mut outfile = fs::File::create(&outpath).with_context(|| {
                format!("Failed to create extracted file: {}", outpath.display())
            })?;

            std::io::copy(&mut file, &mut outfile)
                .with_context(|| format!("Failed to extract file: {}", outpath.display()))?;

            // Set file permissions on Unix-like systems
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))?;
                }
            }

            extracted_files.push(outpath);
        }
    }

    if let Some(pattern) = file_pattern {
        println!(
            "Extracted {} files matching pattern '{pattern}'",
            extracted_files.len()
        );
    } else {
        println!("Extracted {} files", extracted_files.len());
    }
    Ok(extracted_files)
}
