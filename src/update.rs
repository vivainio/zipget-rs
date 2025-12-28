use crate::install::executable::install_package;
use anyhow::Result;

const REPO: &str = "vivainio/zipget-rs";

/// Self-update zipget to the latest version from GitHub
pub fn self_update() -> Result<()> {
    println!("Updating zipget from {REPO}...");
    install_package(
        REPO,
        None,           // auto-detect binary
        None,           // latest tag
        None,           // no files pattern
        None,           // no AWS profile
        Some("zipget"), // install zipget executable
        false,          // use default shim behavior
    )
}
