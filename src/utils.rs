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
