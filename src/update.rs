use crate::install::executable::{InstallOptions, install_package};
use anyhow::Result;

const REPO: &str = "vivainio/zipget-rs";

/// Self-update zipget to the latest version from GitHub
pub fn self_update() -> Result<()> {
    println!("Updating zipget from {REPO}...");
    install_package(
        REPO,
        InstallOptions {
            executable: Some("zipget"),
            install_as: Some("zipget"),
            ..Default::default()
        },
    )
}
