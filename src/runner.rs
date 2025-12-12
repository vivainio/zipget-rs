use crate::archive::{tar, utils as archive_utils, zip};
use crate::download::{github, http};
use crate::install::executable::find_executables;
use crate::utils::get_filename_from_url;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Run a package (download and execute)
pub fn run_package(
    source: &str,
    binary: Option<&str>,
    tag: Option<&str>,
    files_pattern: Option<&str>,
    profile: Option<&str>,
    executable: Option<&str>,
    args: &[String],
) -> Result<()> {
    // Create temporary directory for extraction
    let temp_base = std::env::temp_dir();
    let temp_dir_name = format!("zipget_{}", std::process::id());
    let temp_path = temp_base.join(&temp_dir_name);
    fs::create_dir_all(&temp_path).context("Failed to create temporary directory")?;

    // Determine if source is a GitHub repository or direct URL
    if source.contains("github.com/") {
        // For simplicity, use the direct source string or extract user/repo from URL
        let repo_path = if source.contains("github.com/") {
            source
                .strip_prefix("https://")
                .or_else(|| source.strip_prefix("http://"))
                .unwrap_or(source)
                .strip_prefix("github.com/")
                .unwrap_or(source)
                .split('/')
                .take(2)
                .collect::<Vec<_>>()
                .join("/")
        } else {
            source.to_string()
        };

        // Download from GitHub release
        println!("Downloading from GitHub: {}", repo_path);
        github::fetch_github_release(
            &repo_path,
            binary,
            None,
            tag,
            Some(temp_path.to_str().unwrap()),
            files_pattern,
        )?;
    } else {
        // Direct URL download
        fetch_direct_url(
            source,
            None,
            Some(temp_path.to_str().unwrap()),
            files_pattern,
            profile,
        )?;
    }

    // Flatten directory if needed
    if let Ok(Some(dir_name)) = archive_utils::should_flatten_directory(&temp_path) {
        archive_utils::flatten_directory_structure(&temp_path, &dir_name)?;
    }

    // Find executables in extracted content
    let executables = find_executables(&temp_path).context("Failed to find executables")?;

    if executables.is_empty() {
        return Err(anyhow::anyhow!(
            "No executables found in downloaded package"
        ));
    }

    // Select which executable to run
    let exe_to_run = if let Some(exe_name) = executable {
        executables
            .iter()
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.contains(exe_name))
                    .unwrap_or(false)
            })
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Executable '{}' not found", exe_name))?
    } else if executables.len() == 1 {
        executables[0].clone()
    } else {
        return Err(anyhow::anyhow!(
            "Multiple executables found, please specify which one with --executable"
        ));
    };

    println!("Running: {}", exe_to_run.display());

    // Run the executable with provided arguments
    let mut command = Command::new(&exe_to_run);
    command.args(args);

    let status = command.status().context("Failed to execute package")?;

    // Cleanup temporary directory
    let _ = fs::remove_dir_all(&temp_path);

    if !status.success() {
        return Err(anyhow::anyhow!(
            "Package execution failed with exit code: {}",
            status.code().unwrap_or(-1)
        ));
    }

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
