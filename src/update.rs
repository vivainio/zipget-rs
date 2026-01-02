use crate::install::executable::{InstallOptions, install_package};
use anyhow::{Context, Result};

const REPO: &str = "vivainio/zipget-rs";

/// Self-update zipget to the latest version from GitHub
pub fn self_update() -> Result<()> {
    println!("Updating zipget from {REPO}...");

    // Detect the directory where the current executable is located
    let current_exe = std::env::current_exe().context("Failed to get current executable path")?;
    let install_dir = current_exe
        .parent()
        .map(|p| p.to_path_buf())
        .context("Failed to get parent directory of current executable")?;

    // On Linux, prefer the musl build for maximum portability
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let binary = Some("linux-x64-musl");
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    let binary = Some("linux-arm64-musl");
    #[cfg(not(target_os = "linux"))]
    let binary: Option<&str> = None;

    install_package(
        REPO,
        InstallOptions {
            binary,
            executable: Some("zipget"),
            install_as: Some("zipget"),
            install_dir: Some(install_dir),
            ..Default::default()
        },
    )
}
