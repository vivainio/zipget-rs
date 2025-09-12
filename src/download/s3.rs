use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Check if URL is an S3 URL
pub fn is_s3_url(url: &str) -> bool {
    url.starts_with("s3://")
}

/// Download file from S3 using AWS CLI
pub fn download_s3_file(s3_url: &str, local_path: &Path, profile: Option<&str>) -> Result<()> {
    println!("Downloading from S3: {s3_url}");

    // Check if AWS CLI is available
    let aws_version = std::process::Command::new("aws").arg("--version").output();

    match aws_version {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            println!(
                "Using AWS CLI: {}",
                version.lines().next().unwrap_or("").trim()
            );
        }
        Ok(_) => return Err(anyhow::anyhow!("AWS CLI returned an error")),
        Err(_) => {
            return Err(anyhow::anyhow!(
                "AWS CLI not found. Please install AWS CLI and configure credentials:\n\
             - Install: https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html\n\
             - Configure: aws configure"
            ));
        }
    }

    // Create parent directory if needed
    if let Some(parent) = local_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Create a temporary file in the same directory as the target file
    let temp_path = local_path.with_extension(format!(
        "{}.tmp",
        local_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("download")
    ));

    // Download with aws s3 cp to temporary file first
    let mut cmd = std::process::Command::new("aws");

    // Add profile if specified
    if let Some(profile_name) = profile {
        println!("Using AWS profile: {profile_name}");
        cmd.arg("--profile").arg(profile_name);
    }

    let output = cmd
        .arg("s3")
        .arg("cp")
        .arg(s3_url)
        .arg(&temp_path)
        .output()
        .with_context(|| "Failed to execute aws s3 cp command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Clean up temp file if it exists
        let _ = fs::remove_file(&temp_path);

        return Err(anyhow::anyhow!(
            "AWS S3 download failed:\nStdout: {}\nStderr: {}",
            stdout,
            stderr
        ));
    }

    // Move temporary file to final location
    fs::rename(&temp_path, local_path).with_context(|| {
        format!(
            "Failed to move downloaded file from {} to {}",
            temp_path.display(),
            local_path.display()
        )
    })?;

    println!("✓ Downloaded: {}", local_path.display());
    Ok(())
}
