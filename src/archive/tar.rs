use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use glob_match::glob_match;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use tar::Archive;

/// Extract TAR.GZ archive, returns list of extracted file paths
pub fn extract_tar_gz(
    tar_path: &Path,
    extract_to: &str,
    file_pattern: Option<&str>,
) -> Result<Vec<PathBuf>> {
    let file = fs::File::open(tar_path)
        .with_context(|| format!("Failed to open tar.gz file: {}", tar_path.display()))?;

    let decoder = GzDecoder::new(file);
    extract_tar_from_reader(decoder, extract_to, file_pattern, "tar.gz")
}

/// Extract TAR.ZST archive (Zstandard compression), returns list of extracted file paths
pub fn extract_tar_zst(
    tar_path: &Path,
    extract_to: &str,
    file_pattern: Option<&str>,
) -> Result<Vec<PathBuf>> {
    let file = fs::File::open(tar_path)
        .with_context(|| format!("Failed to open tar.zst file: {}", tar_path.display()))?;

    let decoder = zstd::Decoder::new(file)
        .with_context(|| format!("Failed to create zstd decoder for: {}", tar_path.display()))?;
    extract_tar_from_reader(decoder, extract_to, file_pattern, "tar.zst")
}

/// Extract TAR archive from a generic reader, returns list of extracted file paths
fn extract_tar_from_reader<R: Read>(
    reader: R,
    extract_to: &str,
    file_pattern: Option<&str>,
    archive_type: &str,
) -> Result<Vec<PathBuf>> {
    let mut archive = Archive::new(reader);

    fs::create_dir_all(extract_to)
        .with_context(|| format!("Failed to create extraction directory: {extract_to}"))?;

    let mut extracted_files = Vec::new();

    for entry in archive
        .entries()
        .with_context(|| format!("Failed to read {archive_type} entries"))?
    {
        let mut entry = entry.with_context(|| format!("Failed to access {archive_type} entry"))?;

        let path = entry.path().with_context(|| "Failed to get entry path")?;

        let path_str = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in path"))?;

        // Check if file matches the glob pattern (if specified)
        let flatten = if let Some(pattern) = file_pattern {
            let filename = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if !glob_match(pattern, path_str) && !glob_match(pattern, filename) {
                continue; // Skip files that don't match the pattern (checking both full path and filename)
            }
            true // Flatten when file pattern is specified
        } else {
            false
        };

        // When file pattern is specified, flatten to just the filename
        let outpath = if flatten {
            if let Some(filename) = path.file_name() {
                Path::new(extract_to).join(filename)
            } else {
                continue; // Skip entries without a filename (e.g., directories)
            }
        } else {
            Path::new(extract_to).join(&path)
        };

        // Create parent directories if they don't exist
        if let Some(parent) = outpath.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent directory: {}", parent.display())
            })?;
        }

        // Extract the entry
        entry
            .unpack(&outpath)
            .with_context(|| format!("Failed to extract file: {}", outpath.display()))?;

        extracted_files.push(outpath);
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
