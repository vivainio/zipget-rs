/// Extract filename from URL (handles both HTTP and S3 URLs)
pub fn get_filename_from_url(url: &str) -> String {
    if url.starts_with("s3://") {
        // Extract filename from S3 URL: s3://bucket/path/to/file.zip -> file.zip
        url.split('/').next_back().unwrap_or("download").to_string()
    } else {
        // Existing HTTP URL logic - handle query parameters
        url.split('/')
            .next_back()
            .unwrap_or("download")
            .split('?')
            .next()
            .unwrap_or("download")
            .to_string()
    }
}

/// Asset-name markers that identify a macOS build.
const MACOS_MARKERS: &[&str] = &["darwin", "macos", "osx", "apple"];

/// Score adjustment for a macOS release asset, on top of the generic scoring.
///
/// Returns `(bonus, runs_via_rosetta)`. Apple Silicon prefers a native arm64
/// build, then a universal binary, then an Intel build (which Rosetta 2 can
/// run, so callers must not apply their wrong-architecture penalty to it).
/// Names with no macOS marker get `(0, false)`: an unlabelled `x86_64` asset
/// could be a Linux binary and must keep the penalty.
pub fn macos_arch_adjustment(name_lower: &str, arch: &str) -> (i32, bool) {
    if !MACOS_MARKERS.iter().any(|m| name_lower.contains(m)) {
        return (0, false);
    }
    let has = |patterns: &[&str]| patterns.iter().any(|p| name_lower.contains(p));
    if has(&["universal"]) {
        return (40, true);
    }
    if arch == "aarch64" {
        if has(&["arm64", "aarch64"]) {
            return (50, false);
        }
        if has(&["x86_64", "amd64", "x64"]) {
            return (0, true);
        }
    }
    (0, false)
}

/// Guess appropriate binary name pattern based on current OS and architecture
pub fn guess_binary_name() -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    match (os, arch) {
        ("windows", "x86_64") => "windows".to_string(),
        ("windows", "x86") => "win32".to_string(),
        ("windows", "aarch64") => "windows-arm64".to_string(),
        ("linux", "x86_64") => "linux".to_string(),
        ("linux", "aarch64") => "linux-arm64".to_string(),
        ("linux", "x86") => "linux-i386".to_string(),
        ("macos", "x86_64") => "macos".to_string(),
        ("macos", "aarch64") => "macos-arm64".to_string(),
        _ => {
            // Fallback: try common patterns
            match os {
                "windows" => "windows".to_string(),
                "linux" => "linux".to_string(),
                "macos" => "macos".to_string(),
                _ => "x86_64".to_string(), // Last resort
            }
        }
    }
}

/// Match an asset name against a pattern using regex.
///
/// The pattern is treated as a regex applied case-insensitively against the asset name.
/// For backwards compatibility, if the pattern is not a valid regex, it falls back
/// to a case-insensitive substring match.
pub fn match_asset_name(asset_name: &str, pattern: &str) -> bool {
    match regex::RegexBuilder::new(pattern)
        .case_insensitive(true)
        .build()
    {
        Ok(re) => re.is_match(asset_name),
        Err(_) => {
            // Fallback to substring match for backwards compatibility
            asset_name.to_lowercase().contains(&pattern.to_lowercase())
        }
    }
}

/// Check if a string looks like a version number (e.g., "1.2.3", "v2.0.1-alpha")
pub fn is_version_like(part: &str) -> bool {
    // Check for common version patterns
    // x.y.z, x.y, vx.y.z, x.y.z-alpha, etc.
    let part = part.trim_start_matches('v').trim_start_matches('V');

    // Simple regex-like check for version patterns
    let chars: Vec<char> = part.chars().collect();
    if chars.is_empty() {
        return false;
    }

    // Must start with a digit
    if !chars[0].is_ascii_digit() {
        return false;
    }

    // Look for patterns like x.y or x.y.z
    let mut dot_count = 0;
    let mut has_digit_after_dot = false;

    for (i, &ch) in chars.iter().enumerate() {
        match ch {
            '0'..='9' => {
                if i > 0 && chars[i - 1] == '.' {
                    has_digit_after_dot = true;
                }
            }
            '.' => {
                dot_count += 1;
                has_digit_after_dot = false;
                // Too many dots is suspicious
                if dot_count > 3 {
                    return false;
                }
            }
            '-' | '+' => {
                // Allow version suffixes like -alpha, -beta, +build
                break;
            }
            _ => {
                // Other characters might be part of version suffix
                if dot_count == 0 {
                    return false; // No dots seen yet, this doesn't look like a version
                }
                break;
            }
        }
    }

    // Must have at least one dot and a digit after it
    dot_count > 0 && has_digit_after_dot
}

/// Check if a string matches platform identifier patterns (Windows only)
#[cfg(windows)]
pub fn is_platform_identifier(part: &str, platform_patterns: &[&str]) -> bool {
    platform_patterns
        .iter()
        .any(|&pattern| part == pattern || part.contains(pattern))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_filename_from_url_simple() {
        assert_eq!(
            get_filename_from_url("https://example.com/file.zip"),
            "file.zip"
        );
    }

    #[test]
    fn test_get_filename_from_url_with_query() {
        assert_eq!(
            get_filename_from_url("https://example.com/file.zip?token=abc123"),
            "file.zip"
        );
    }

    #[test]
    fn test_get_filename_from_url_github() {
        assert_eq!(
            get_filename_from_url(
                "https://github.com/user/repo/releases/download/v1.0/app-linux-amd64.tar.gz"
            ),
            "app-linux-amd64.tar.gz"
        );
    }

    #[test]
    fn test_get_filename_from_url_s3() {
        assert_eq!(
            get_filename_from_url("s3://mybucket/path/to/file.zip"),
            "file.zip"
        );
    }

    #[test]
    fn test_get_filename_from_url_empty_path() {
        assert_eq!(get_filename_from_url("https://example.com/"), "");
    }

    #[test]
    fn test_is_version_like_semver() {
        assert!(is_version_like("1.2.3"));
        assert!(is_version_like("0.1.0"));
        assert!(is_version_like("10.20.30"));
    }

    #[test]
    fn test_is_version_like_with_v_prefix() {
        assert!(is_version_like("v1.2.3"));
        assert!(is_version_like("V2.0.0"));
    }

    #[test]
    fn test_is_version_like_two_parts() {
        assert!(is_version_like("1.0"));
        assert!(is_version_like("v2.1"));
    }

    #[test]
    fn test_is_version_like_with_suffix() {
        assert!(is_version_like("1.0.0-alpha"));
        assert!(is_version_like("2.0.0-beta.1"));
        assert!(is_version_like("1.0.0+build123"));
    }

    #[test]
    fn test_is_version_like_not_version() {
        assert!(!is_version_like("linux"));
        assert!(!is_version_like("amd64"));
        assert!(!is_version_like("windows"));
        assert!(!is_version_like(""));
        assert!(!is_version_like("abc"));
    }

    #[test]
    fn test_is_version_like_edge_cases() {
        assert!(!is_version_like("1")); // No dot
        assert!(!is_version_like("1.")); // No digit after dot
        assert!(!is_version_like(".1.2")); // Doesn't start with digit
    }

    #[test]
    fn test_match_asset_name_regex() {
        // Exact regex match
        assert!(match_asset_name(
            "obsidian-1.11.7.tar.gz",
            r"^obsidian-[\d.]+\.tar\.gz$"
        ));
        // Should NOT match arm64 variant
        assert!(!match_asset_name(
            "obsidian-1.11.7-arm64.tar.gz",
            r"^obsidian-[\d.]+\.tar\.gz$"
        ));
    }

    #[test]
    fn test_match_asset_name_case_insensitive() {
        assert!(match_asset_name("Tool-Linux-AMD64.tar.gz", "linux-amd64"));
    }

    #[test]
    fn test_match_asset_name_substring_fallback() {
        // Invalid regex falls back to substring match
        // "tool(((" is not valid regex, but "tool(((" is a substring of "tool(((-linux"
        assert!(match_asset_name("tool(((-linux.tar.gz", "tool((("));
        assert!(!match_asset_name("other.tar.gz", "tool((("));
    }

    #[test]
    fn test_match_asset_name_simple_pattern() {
        // Simple string is valid regex and matches as substring
        assert!(match_asset_name(
            "ripgrep-x86_64-unknown-linux-musl.tar.gz",
            "x86_64-unknown-linux-musl"
        ));
    }

    #[test]
    fn macos_adjustment_ranks_native_universal_then_intel() {
        assert_eq!(
            macos_arch_adjustment("tool-darwin-arm64.tar.gz", "aarch64"),
            (50, false)
        );
        assert_eq!(
            macos_arch_adjustment("tool-macos-universal.tar.gz", "aarch64"),
            (40, true)
        );
        assert_eq!(
            macos_arch_adjustment("tool-darwin-x86_64.tar.gz", "aarch64"),
            (0, true)
        );
    }

    #[test]
    fn macos_adjustment_ignores_assets_without_a_macos_marker() {
        assert_eq!(
            macos_arch_adjustment("tool-x86_64.tar.gz", "aarch64"),
            (0, false)
        );
        assert_eq!(
            macos_arch_adjustment("tool-linux-arm64.tar.gz", "aarch64"),
            (0, false)
        );
    }

    #[test]
    fn macos_adjustment_gives_no_rosetta_to_arm64_on_intel() {
        assert_eq!(
            macos_arch_adjustment("tool-darwin-arm64.tar.gz", "x86_64"),
            (0, false)
        );
    }
}
