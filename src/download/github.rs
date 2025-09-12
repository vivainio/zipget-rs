use crate::cache::get_cache_dir;
use crate::crypto::compute_sha256_from_bytes;
use crate::download::http;
use crate::models::{GitHubAsset, GitHubRelease};
use crate::utils::get_filename_from_url;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Fetch a GitHub release with the specified parameters
pub fn fetch_github_release(
    repo: &str,
    binary_name: Option<&str>,
    save_as: Option<&str>,
    tag: Option<&str>,
    unzip_to: Option<&str>,
    files_pattern: Option<&str>,
) -> Result<()> {
    let (release, binary_name) = if let Some(name) = binary_name {
        // User specified binary name, fetch release separately
        let api_url = if let Some(tag) = tag {
            format!("https://api.github.com/repos/{repo}/releases/tags/{tag}")
        } else {
            format!("https://api.github.com/repos/{repo}/releases/latest")
        };

        println!("Fetching release info from: {api_url}");

        let response = ureq::get(&api_url)
            .set("User-Agent", "zipget-rs")
            .call()
            .with_context(|| format!("Failed to fetch release info for {repo}"))?;

        if response.status() != 200 {
            return Err(anyhow::anyhow!(
                "GitHub API request failed with status: {}",
                response.status()
            ));
        }

        let release: GitHubRelease = response
            .into_json()
            .with_context(|| "Failed to parse GitHub release JSON")?;

        (release, name.to_string())
    } else {
        println!("No binary specified for {repo}, analyzing available assets...");
        get_best_binary_from_release(repo, tag)?
    };

    println!("Found release: {} ({})", release.name, release.tag_name);

    // Find the matching asset (case-insensitive)
    let asset = release
        .assets
        .iter()
        .find(|asset| {
            asset
                .name
                .to_lowercase()
                .contains(&binary_name.to_lowercase())
        })
        .ok_or_else(|| anyhow::anyhow!("Binary '{}' not found in release assets", binary_name))?;

    println!("Found asset: {} ({} bytes)", asset.name, asset.size);

    // Use caching mechanism
    let cache_dir = get_cache_dir()?;
    let download_url = &asset.browser_download_url;
    let filename = get_filename_from_url(download_url);

    let url_hash = compute_sha256_from_bytes(download_url.as_bytes());
    let cached_filename = format!("{url_hash}_{filename}");
    let cached_file_path = cache_dir.join(&cached_filename);

    let file_path = if cached_file_path.exists() {
        println!("Found cached file: {}", cached_file_path.display());
        cached_file_path
    } else {
        // Download the file
        println!("Downloading: {download_url}");
        http::download_file(download_url, &cached_file_path, None)?;
        cached_file_path
    };

    // Save as specified file if requested
    if let Some(save_as) = save_as {
        let save_path = Path::new(save_as);
        if let Some(parent) = save_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
        fs::copy(&file_path, save_path)
            .with_context(|| format!("Failed to save file as: {save_as}"))?;
        println!("Saved as: {save_as}");
    } else {
        // If no save_as specified, copy to current directory with original filename
        let output_path = Path::new(".").join(&filename);
        fs::copy(&file_path, &output_path)
            .with_context(|| format!("Failed to copy file to: {}", output_path.display()))?;
        println!("Saved as: {}", output_path.display());
    }

    // Extract if unzip_to is specified
    if let Some(unzip_to) = unzip_to {
        println!("Extracting to: {unzip_to}");
        extract_archive_with_options(&file_path, unzip_to, files_pattern)?;
    }

    Ok(())
}

/// Extract archive with options (simplified version)
fn extract_archive_with_options(
    file_path: &Path,
    extract_to: &str,
    files_pattern: Option<&str>,
) -> Result<()> {
    use crate::archive::{tar, zip};

    if file_path.extension().and_then(|s| s.to_str()) == Some("zip") {
        zip::extract_zip(file_path, extract_to, files_pattern)?;
    } else if file_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .ends_with(".tar.gz")
        || file_path.extension().and_then(|s| s.to_str()) == Some("tgz")
    {
        tar::extract_tar_gz(file_path, extract_to, files_pattern)?;
    } else {
        println!("Warning: Unknown archive format, skipping extraction");
    }
    Ok(())
}

/// Get the best binary from a GitHub release automatically
pub fn get_best_binary_from_release(
    repo: &str,
    tag: Option<&str>,
) -> Result<(GitHubRelease, String)> {
    let api_url = if let Some(tag) = tag {
        format!("https://api.github.com/repos/{repo}/releases/tags/{tag}")
    } else {
        format!("https://api.github.com/repos/{repo}/releases/latest")
    };

    println!("Fetching release info from: {api_url}");

    let response = ureq::get(&api_url)
        .set("User-Agent", "zipget-rs")
        .call()
        .with_context(|| format!("Failed to fetch release info for {repo}"))?;

    if response.status() != 200 {
        return Err(anyhow::anyhow!(
            "GitHub API request failed with status: {}",
            response.status()
        ));
    }

    let release: GitHubRelease = response
        .into_json()
        .with_context(|| "Failed to parse GitHub release JSON")?;

    // Simple heuristic to find best binary for current platform
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    println!("Available assets:");
    for asset in &release.assets {
        println!("  - {} ({} bytes)", asset.name, asset.size);
    }

    // Look for platform-specific binaries
    let platform_keywords = match os {
        "windows" => vec!["windows", "win", "pc"],
        "linux" => vec!["linux", "gnu"],
        "macos" => vec!["darwin", "macos", "osx"],
        _ => vec![],
    };

    let arch_keywords = match arch {
        "x86_64" => vec!["x86_64", "amd64", "x64"],
        "aarch64" => vec!["aarch64", "arm64"],
        _ => vec![],
    };

    // Find best matching asset
    let mut best_asset: Option<&GitHubAsset> = None;
    let mut best_score = 0;

    for asset in &release.assets {
        let name_lower = asset.name.to_lowercase();
        let mut score = 0;

        // Prefer executable-like files
        if name_lower.ends_with(".exe")
            || name_lower.ends_with(".zip")
            || name_lower.ends_with(".tar.gz")
        {
            score += 10;
        }

        // Platform matching
        for keyword in &platform_keywords {
            if name_lower.contains(keyword) {
                score += 5;
            }
        }

        // Architecture matching
        for keyword in &arch_keywords {
            if name_lower.contains(keyword) {
                score += 3;
            }
        }

        if score > best_score {
            best_score = score;
            best_asset = Some(asset);
        }
    }

    let asset = best_asset
        .ok_or_else(|| anyhow::anyhow!("No suitable binary found for platform {os}-{arch}"))?;

    println!(
        "Auto-selected asset: {} (score: {})",
        asset.name, best_score
    );
    let asset_name = asset.name.clone();
    Ok((release, asset_name))
}

/// Find the best matching binary asset from GitHub release assets
pub fn find_best_matching_binary(assets: &[GitHubAsset]) -> Option<String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    // Define OS patterns (in order of preference)
    let os_patterns = match os {
        "windows" => vec!["windows", "win", "pc-windows", "msvc"],
        "linux" => vec!["linux", "unknown-linux", "gnu"],
        "macos" => vec!["darwin", "macos", "apple"],
        _ => vec![os],
    };

    // Define architecture patterns (in order of preference)
    let arch_patterns = match arch {
        "x86_64" => vec!["x86_64", "amd64", "x64", "64"],
        "x86" => vec!["x86", "i386", "i686", "32", "win32"],
        "aarch64" => vec!["aarch64", "arm64", "armv8"],
        "arm" => vec!["arm", "armv7", "armhf"],
        _ => vec![arch],
    };

    let mut best_score = 0;
    let mut best_asset = None;

    for asset in assets {
        let name_lower = asset.name.to_lowercase();
        let mut score = 0;

        // Score OS match (higher weight)
        for (i, pattern) in os_patterns.iter().enumerate() {
            if name_lower.contains(pattern) {
                score += 100 - (i * 10); // First match gets 100, second gets 90, etc.
                break;
            }
        }

        // Score architecture match (medium weight)
        for (i, pattern) in arch_patterns.iter().enumerate() {
            if name_lower.contains(pattern) {
                score += 50 - (i * 5); // First match gets 50, second gets 45, etc.
                break;
            }
        }

        // Bonus for common binary extensions/patterns
        if name_lower.ends_with(".zip")
            || name_lower.ends_with(".tar.gz")
            || name_lower.ends_with(".tgz")
        {
            score += 10;
        }

        // Penalty for source packages or unwanted patterns
        if name_lower.contains("src") || name_lower.contains("source") {
            score -= 50;
        }
        if name_lower.contains("debug") || name_lower.contains("symbols") {
            score -= 30;
        }

        println!("  {} -> score: {}", asset.name, score);

        if score > best_score {
            best_score = score;
            best_asset = Some(asset.name.clone());
        }
    }

    best_asset
}

// TODO: Add other GitHub functions here
// - get_best_binary_from_release
// - fetch_github_release
// - get_github_release_url
// - get_latest_github_tag
