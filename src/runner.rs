use crate::archive::{tar, zip};
use crate::download::http;
use crate::utils::get_filename_from_url;
use anyhow::Result;
use std::path::PathBuf;

/// Run a package (download and execute)
pub fn run_package() -> Result<()> {
    // TODO: Implement package running logic from main.rs
    println!("Package running not yet implemented in refactored version");
    Ok(())
}

/// Fetch content from a direct URL
pub fn fetch_direct_url(
    url: &str,
    save_as: Option<&str>,
    unzip_to: Option<&str>,
    files: Option<&str>,
    profile: Option<&str>,
) -> Result<()> {
    // Determine the local file path
    let local_path = if let Some(save_path) = save_as {
        PathBuf::from(save_path)
    } else {
        PathBuf::from(get_filename_from_url(url))
    };

    // Download the file
    http::download_file(url, &local_path, profile)?;

    // Extract if needed
    if let Some(extract_dir) = unzip_to {
        println!("Extracting to: {extract_dir}");

        if local_path.extension().and_then(|s| s.to_str()) == Some("zip") {
            zip::extract_zip(&local_path, extract_dir, files)?;
        } else if local_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .ends_with(".tar.gz")
            || local_path.extension().and_then(|s| s.to_str()) == Some("tgz")
        {
            tar::extract_tar_gz(&local_path, extract_dir, files)?;
        } else {
            println!("Warning: Unknown archive format, skipping extraction");
        }
    }

    Ok(())
}
