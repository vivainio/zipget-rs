use crate::cache::get_cache_dir;
use crate::crypto::compute_sha256_from_bytes;
use crate::download::http;
use crate::models::{GitHubAsset, GitHubRelease};
use crate::utils::get_filename_from_url;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Fetch GitHub release info from API with optional token authentication
fn fetch_release_info(repo: &str, tag: Option<&str>) -> Result<GitHubRelease> {
    let api_url = if let Some(tag) = tag {
        format!("https://api.github.com/repos/{repo}/releases/tags/{tag}")
    } else {
        format!("https://api.github.com/repos/{repo}/releases/latest")
    };

    println!("Fetching release info from: {api_url}");

    let mut request = ureq::get(&api_url).set("User-Agent", "zipget-rs");

    // Use GITHUB_TOKEN if available for higher rate limits
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        request = request.set("Authorization", &format!("Bearer {token}"));
    }

    let response = request
        .call()
        .with_context(|| format!("Failed to fetch release info for {repo}"))?;

    if response.status() != 200 {
        return Err(anyhow::anyhow!(
            "GitHub API request failed with status: {}",
            response.status()
        ));
    }

    response
        .into_json()
        .with_context(|| "Failed to parse GitHub release JSON")
}

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
        let release = fetch_release_info(repo, tag)?;
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
    } else if unzip_to.is_none() {
        // Only copy to current directory if not extracting (extraction handles the file itself)
        let output_path = Path::new(".").join(&filename);
        fs::copy(&file_path, &output_path)
            .with_context(|| format!("Failed to copy file to: {}", output_path.display()))?;
        println!("Saved as: {}", output_path.display());
    }

    // Extract if unzip_to is specified
    if let Some(unzip_to) = unzip_to {
        println!("Extracting to: {unzip_to}");
        extract_archive_with_options(&file_path, unzip_to, files_pattern, &filename)?;
    }

    Ok(())
}

/// Extract archive with options (simplified version)
/// For non-archive files (direct binaries), copies them to the extraction directory
fn extract_archive_with_options(
    file_path: &Path,
    extract_to: &str,
    files_pattern: Option<&str>,
    original_filename: &str,
) -> Result<()> {
    use crate::archive::{tar, zip};

    if original_filename.ends_with(".zip") {
        let _ = zip::extract_zip(file_path, extract_to, files_pattern)?;
    } else if original_filename.ends_with(".tar.gz") || original_filename.ends_with(".tgz") {
        let _ = tar::extract_tar_gz(file_path, extract_to, files_pattern)?;
    } else if original_filename.ends_with(".tar.zst") {
        let _ = tar::extract_tar_zst(file_path, extract_to, files_pattern)?;
    } else {
        // Not an archive - assume it's a direct binary and copy it to the extraction directory
        let dest_path = Path::new(extract_to).join(original_filename);
        fs::copy(file_path, &dest_path)
            .with_context(|| format!("Failed to copy binary to: {}", dest_path.display()))?;

        // Set executable permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            fs::set_permissions(&dest_path, perms).with_context(|| {
                format!(
                    "Failed to set executable permissions on: {}",
                    dest_path.display()
                )
            })?;
        }

        println!("Copied direct binary to: {}", dest_path.display());
    }
    Ok(())
}

/// Get the best binary from a GitHub release automatically
pub fn get_best_binary_from_release(
    repo: &str,
    tag: Option<&str>,
) -> Result<(GitHubRelease, String)> {
    let release = fetch_release_info(repo, tag)?;

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

    // Define wrong architecture patterns for penalties
    let wrong_arch_patterns: &[&str] = match arch {
        "x86_64" => &["aarch64", "arm64", "armv7", "armhf", "i386", "i686"],
        "aarch64" => &["x86_64", "amd64", "i386", "i686"],
        _ => &[],
    };

    // Define wrong OS patterns for penalties
    let wrong_os_patterns: &[&str] = match os {
        "linux" => &["windows", "win32", "win64", "darwin", "macos", "apple"],
        "windows" => &["linux", "darwin", "macos", "apple"],
        "macos" => &["linux", "windows", "win32", "win64"],
        _ => &[],
    };

    // Find best matching asset
    let mut best_asset: Option<&GitHubAsset> = None;
    let mut best_score: i32 = i32::MIN;

    for asset in &release.assets {
        let name_lower = asset.name.to_lowercase();
        let mut score: i32 = 0;

        // Prefer executable-like files
        if name_lower.ends_with(".exe")
            || name_lower.ends_with(".zip")
            || name_lower.ends_with(".tar.gz")
            || name_lower.ends_with(".tar.zst")
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

        // Prefer musl for portability on Linux
        if os == "linux" && name_lower.contains("musl") {
            score += 5;
        }

        // Penalty for wrong architecture
        for pattern in wrong_arch_patterns {
            if name_lower.contains(pattern) {
                score -= 100;
                break;
            }
        }

        // Penalty for wrong OS
        for pattern in wrong_os_patterns {
            if name_lower.contains(pattern) {
                score -= 100;
                break;
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

    let mut best_score: i32 = 0;
    let mut best_asset = None;

    for asset in assets {
        let name_lower = asset.name.to_lowercase();
        let mut score: i32 = 0;

        // Score OS match (higher weight)
        for (i, pattern) in os_patterns.iter().enumerate() {
            if name_lower.contains(pattern) {
                score += 100 - (i as i32 * 10); // First match gets 100, second gets 90, etc.
                break;
            }
        }

        // Score architecture match (medium weight)
        for (i, pattern) in arch_patterns.iter().enumerate() {
            if name_lower.contains(pattern) {
                score += 50 - (i as i32 * 5); // First match gets 50, second gets 45, etc.
                break;
            }
        }

        // Prefer musl for portability on Linux
        if os == "linux" && name_lower.contains("musl") {
            score += 25;
        }

        // Penalty for wrong architecture
        let wrong_arch_patterns: &[&str] = match arch {
            "x86_64" => &["aarch64", "arm64", "armv7", "armhf", "i386", "i686"],
            "aarch64" => &["x86_64", "amd64", "i386", "i686"],
            "x86" => &["x86_64", "amd64", "aarch64", "arm64"],
            "arm" => &["x86_64", "amd64", "aarch64", "arm64", "i386", "i686"],
            _ => &[],
        };
        for pattern in wrong_arch_patterns {
            if name_lower.contains(pattern) {
                score -= 100; // Strong penalty for wrong architecture
                break;
            }
        }

        // Penalty for wrong OS
        let wrong_os_patterns: &[&str] = match os {
            "linux" => &["windows", "win32", "win64", "darwin", "macos", "apple"],
            "windows" => &["linux", "darwin", "macos", "apple"],
            "macos" => &["linux", "windows", "win32", "win64"],
            _ => &[],
        };
        for pattern in wrong_os_patterns {
            if name_lower.contains(pattern) {
                score -= 100; // Strong penalty for wrong OS
                break;
            }
        }

        // Bonus for common binary extensions/patterns
        if name_lower.ends_with(".zip")
            || name_lower.ends_with(".tar.gz")
            || name_lower.ends_with(".tgz")
            || name_lower.ends_with(".tar.zst")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_asset(name: &str, size: u64) -> GitHubAsset {
        GitHubAsset {
            name: name.to_string(),
            browser_download_url: format!("https://example.com/{name}"),
            size,
        }
    }

    #[test]
    fn test_find_best_matching_binary_linux_x64() {
        let assets = vec![
            make_asset("tool-linux-amd64.tar.gz", 1000),
            make_asset("tool-windows-amd64.zip", 1000),
            make_asset("tool-darwin-amd64.tar.gz", 1000),
        ];

        // This test is platform-dependent, so just verify it returns something
        let result = find_best_matching_binary(&assets);
        assert!(result.is_some());
    }

    #[test]
    fn test_find_best_matching_binary_prefers_archive() {
        let assets = vec![
            make_asset("tool.tar.gz", 1000),
            make_asset("tool.deb", 1000),
            make_asset("tool.rpm", 1000),
        ];

        let result = find_best_matching_binary(&assets);
        // Should prefer .tar.gz over .deb and .rpm
        assert!(result.is_some());
        if let Some(name) = result {
            assert!(name.ends_with(".tar.gz") || name.ends_with(".zip"));
        }
    }

    #[test]
    fn test_find_best_matching_binary_avoids_source() {
        let assets = vec![
            make_asset("tool-src.tar.gz", 1000),
            make_asset("tool-source.tar.gz", 1000),
            make_asset("tool-linux-amd64.tar.gz", 1000),
        ];

        let result = find_best_matching_binary(&assets);
        assert!(result.is_some());
        if let Some(name) = result {
            // Should not select source packages
            assert!(!name.contains("src"));
            assert!(!name.contains("source"));
        }
    }

    #[test]
    fn test_find_best_matching_binary_avoids_debug() {
        let assets = vec![
            make_asset("tool-debug.tar.gz", 1000),
            make_asset("tool-symbols.tar.gz", 1000),
            make_asset("tool-linux-amd64.tar.gz", 1000),
        ];

        let result = find_best_matching_binary(&assets);
        assert!(result.is_some());
        if let Some(name) = result {
            // Should not select debug/symbols packages
            assert!(!name.contains("debug"));
            assert!(!name.contains("symbols"));
        }
    }

    #[test]
    fn test_find_best_matching_binary_empty_assets() {
        let assets: Vec<GitHubAsset> = vec![];
        let result = find_best_matching_binary(&assets);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_best_matching_binary_various_extensions() {
        let assets = vec![
            make_asset("tool.tar.gz", 1000),
            make_asset("tool.tgz", 1000),
            make_asset("tool.zip", 1000),
            make_asset("tool.tar.zst", 1000),
        ];

        let result = find_best_matching_binary(&assets);
        assert!(result.is_some());
    }
}
