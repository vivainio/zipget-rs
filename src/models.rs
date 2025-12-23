use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// TOML recipe format with optional vars section
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Recipe {
    /// Variable definitions for substitution
    #[serde(default)]
    pub vars: HashMap<String, String>,
    /// Fetch items (all other sections)
    #[serde(flatten)]
    pub items: HashMap<String, FetchItem>,
}

/// Command line arguments
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

/// CLI subcommands
#[derive(Subcommand)]
pub enum Commands {
    /// Process a TOML recipe file to download and extract packages
    Recipe {
        /// TOML recipe file path
        file: String,
        /// Optional tag to filter items by
        tag: Option<String>,
        /// Upgrade all GitHub releases to latest versions
        #[arg(long)]
        upgrade: bool,
        /// AWS profile to use for S3 downloads
        #[arg(short, long)]
        profile: Option<String>,
        /// Write SHA-256 hashes for each file to the recipe (creates lock file)
        #[arg(long)]
        lock: bool,
        /// Set variable overrides (format: key=value), can be specified multiple times
        #[arg(long = "set", value_name = "KEY=VALUE")]
        var_overrides: Vec<String>,
    },
    /// Fetch the latest release binary from a GitHub repository
    Github {
        /// GitHub repository in format "owner/repo"
        repo: String,
        /// Name of the binary to download from release assets (auto-detected if not specified)
        #[arg(short = 'a', long = "asset")]
        binary: Option<String>,
        /// Optional path to save the downloaded file (defaults to current directory with original filename)
        #[arg(short = 's', long = "save-as")]
        save_as: Option<String>,
        /// Optional tag to download specific release (defaults to latest)
        #[arg(short, long)]
        tag: Option<String>,
        /// Optional directory to extract archives to (supports ZIP and tar.gz files)
        #[arg(short = 'u', long = "unzip-to")]
        unzip_to: Option<String>,
        /// Optional glob pattern for files to extract from archives (extracts all if not specified)
        #[arg(short = 'f', long = "files")]
        files: Option<String>,
    },
    /// Fetch a file from a direct URL
    Fetch {
        /// Direct URL to download
        url: String,
        /// Optional path to save the downloaded file (defaults to current directory with original filename)
        #[arg(short = 's', long = "save-as")]
        save_as: Option<String>,
        /// Optional directory to extract archives to (supports ZIP and tar.gz files)
        #[arg(short = 'u', long = "unzip-to")]
        unzip_to: Option<String>,
        /// Optional glob pattern for files to extract from archives (extracts all if not specified)
        #[arg(short = 'f', long = "files")]
        files: Option<String>,
        /// AWS profile to use for S3 downloads
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Download and run an executable from a package
    Run {
        /// Source to download from: URL or GitHub repository (owner/repo format)
        source: String,
        /// Name of the binary to download from GitHub release assets (auto-detected if not specified)
        #[arg(short, long)]
        binary: Option<String>,
        /// Optional tag to download specific GitHub release (defaults to latest)
        #[arg(short, long)]
        tag: Option<String>,
        /// Optional glob pattern for files to extract from archives (extracts all if not specified)
        #[arg(short = 'f', long = "files")]
        files: Option<String>,
        /// AWS profile to use for S3 downloads
        #[arg(short, long)]
        profile: Option<String>,
        /// Executable name to run (required if multiple executables found)
        #[arg(short = 'e', long = "exe")]
        executable: Option<String>,
        /// Arguments to pass to the executable
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Install a binary to local Programs folder and create a shim (Windows), or directly to ~/.local/bin (--no-shim)
    Install {
        /// Source to download from: URL or GitHub repository (owner/repo format)
        source: String,
        /// Name of the binary to download from GitHub release assets (auto-detected if not specified)
        #[arg(short, long)]
        binary: Option<String>,
        /// Optional tag to download specific GitHub release (defaults to latest)
        #[arg(short, long)]
        tag: Option<String>,
        /// Optional glob pattern for files to extract from archives (extracts all if not specified)
        #[arg(short = 'f', long = "files")]
        files: Option<String>,
        /// AWS profile to use for S3 downloads
        #[arg(short, long)]
        profile: Option<String>,
        /// Executable name to install (installs all executables if not specified)
        #[arg(short = 'e', long = "exe")]
        executable: Option<String>,
        /// Install executable directly to ~/.local/bin instead of creating shims
        #[arg(long)]
        no_shim: bool,
    },
    /// Create a shim in ~/.local/bin pointing to an existing executable
    Shim {
        /// Path to the existing executable to create a shim for
        target_executable: String,
    },
}

/// Lock information for downloaded files
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LockInfo {
    /// SHA-256 hash for file verification (hex string)
    pub sha: Option<String>,
    /// Direct download URL (stored during lock file generation for faster access)
    pub download_url: Option<String>,
}

/// Recipe item configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FetchItem {
    pub url: Option<String>,
    pub github: Option<GitHubFetch>,
    pub unzip_to: Option<String>,
    pub save_as: Option<String>,
    /// Optional glob pattern for files to extract from archives (extracts all if not specified)
    pub files: Option<String>,
    /// Optional AWS profile for S3 downloads
    pub profile: Option<String>,
    /// List of executables to install from the extracted directory (supports glob patterns)
    pub install_exes: Option<Vec<String>>,
    /// Install executable directly without creating shims (defaults to false on Windows)
    pub no_shim: Option<bool>,
    /// Lock information (SHA-256 hash and direct download URL)
    pub lock: Option<LockInfo>,
    /// Set executable permission on extracted files (Unix only)
    pub executable: Option<bool>,
}

/// GitHub repository fetch configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubFetch {
    pub repo: String,
    pub asset: Option<String>,
    pub tag: Option<String>,
}

/// GitHub release information
#[derive(Debug, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: String,
    pub assets: Vec<GitHubAsset>,
}

/// GitHub release asset information
#[derive(Debug, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// Result of lock file processing
#[derive(Debug)]
pub struct LockResult {
    pub sha: String,
    pub resolved_tag: Option<String>, // For GitHub releases without explicit tags
    pub download_url: Option<String>, // Direct download URL for GitHub assets
}
