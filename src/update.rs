use crate::download::github;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

const REPO: &str = "vivainio/zipget-rs";

/// Self-update zipget to the latest version from GitHub.
///
/// Unlike a normal install, this replaces the *running* binary in place rather
/// than creating a shim, so the `zipget` command keeps resolving to the updated
/// executable.
pub fn self_update() -> Result<()> {
    println!("Updating zipget from {REPO}...");

    let current_exe = std::env::current_exe().context("Failed to get current executable path")?;

    // Platform binary hint; assets are auto-detected when None. Prefer the musl
    // build on Linux for maximum portability.
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let binary = Some("linux-x64-musl");
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    let binary = Some("linux-arm64-musl");
    #[cfg(not(target_os = "linux"))]
    let binary: Option<&str> = None;

    // Stage the download next to the current executable so the final swap is a
    // same-directory rename (atomic, no cross-device copy).
    let staging = current_exe.with_extension("new");
    let staging_str = staging
        .to_str()
        .context("Current executable path is not valid UTF-8")?;

    github::fetch_github_release(REPO, binary, Some(staging_str), None, None, None)
        .context("Failed to download the latest zipget release")?;

    replace_running_exe(&current_exe, &staging).with_context(|| {
        format!(
            "Failed to replace running executable {}",
            current_exe.display()
        )
    })?;

    println!("Updated zipget at {}", current_exe.display());
    Ok(())
}

/// Replace the running executable on Windows.
///
/// Windows refuses to delete or overwrite a running `.exe`, but it does allow
/// renaming it. Move the running binary aside, then move the freshly downloaded
/// one into its place.
#[cfg(windows)]
fn replace_running_exe(current: &Path, staging: &Path) -> Result<()> {
    let backup = current.with_extension("old");
    // Clear any leftover backup from a previous update (the running .old file
    // could not be deleted then).
    let _ = fs::remove_file(&backup);

    fs::rename(current, &backup).context("Failed to move the running executable aside")?;

    if let Err(e) = fs::rename(staging, current) {
        // Restore the original so the user is not left without a binary.
        let _ = fs::rename(&backup, current);
        return Err(anyhow::Error::new(e).context("Failed to move the new executable into place"));
    }

    // The old binary is still mapped by the running process and cannot be
    // removed now; it is cleaned up on the next update.
    Ok(())
}

/// Replace the running executable on Unix.
///
/// Unix lets us replace the file backing a running process, so a rename over
/// the current path is safe and atomic.
#[cfg(unix)]
fn replace_running_exe(current: &Path, staging: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(staging, fs::Permissions::from_mode(0o755))
        .context("Failed to set executable permissions on the downloaded binary")?;
    fs::rename(staging, current).context("Failed to move the new executable into place")?;
    Ok(())
}
