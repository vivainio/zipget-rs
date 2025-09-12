use crate::download::s3;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Download file from HTTP/HTTPS URL or S3
pub fn download_file(url: &str, path: &Path, profile: Option<&str>) -> Result<()> {
    if s3::is_s3_url(url) {
        s3::download_s3_file(url, path, profile)
    } else {
        download_http_file(url, path)
    }
}

/// Download file via HTTP/HTTPS
fn download_http_file(url: &str, path: &Path) -> Result<()> {
    let response = ureq::get(url)
        .call()
        .with_context(|| format!("Failed to download: {url}"))?;

    if response.status() != 200 {
        return Err(anyhow::anyhow!(
            "Download failed with status: {}",
            response.status()
        ));
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Create a temporary file in the same directory as the target file
    let temp_path = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("download")
    ));

    // Download to temporary file first
    let mut temp_file = fs::File::create(&temp_path)
        .with_context(|| format!("Failed to create temporary file: {}", temp_path.display()))?;

    std::io::copy(&mut response.into_reader(), &mut temp_file).with_context(|| {
        // Clean up temporary file on failure
        let _ = fs::remove_file(&temp_path);
        format!("Failed to write to temporary file: {}", temp_path.display())
    })?;

    // Ensure data is written to disk
    temp_file.sync_all().with_context(|| {
        let _ = fs::remove_file(&temp_path);
        format!("Failed to sync temporary file: {}", temp_path.display())
    })?;

    let file_size = temp_file.metadata()?.len();
    drop(temp_file); // Close the file handle

    // Atomically move the temporary file to the final location
    fs::rename(&temp_path, path).with_context(|| {
        let _ = fs::remove_file(&temp_path);
        format!(
            "Failed to move temporary file to final location: {} -> {}",
            temp_path.display(),
            path.display()
        )
    })?;

    println!("Downloaded: {} ({} bytes)", path.display(), file_size);
    Ok(())
}
