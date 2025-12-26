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
        /// Optional tags to filter items by (can specify multiple)
        tags: Vec<String>,
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
        /// Show how variables would be expanded without downloading
        #[arg(long)]
        dry: bool,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_recipe() {
        let toml_str = r#"
[mypackage]
url = "https://example.com/file.zip"
save_as = "./downloads/file.zip"
"#;
        let recipe: Recipe = toml::from_str(toml_str).unwrap();
        assert!(recipe.items.contains_key("mypackage"));
        let item = &recipe.items["mypackage"];
        assert_eq!(item.url, Some("https://example.com/file.zip".to_string()));
        assert_eq!(item.save_as, Some("./downloads/file.zip".to_string()));
    }

    #[test]
    fn test_parse_recipe_with_github() {
        let toml_str = r#"
[cli-tool]
github = { repo = "owner/repo", asset = "tool-linux-amd64.tar.gz", tag = "v1.0.0" }
unzip_to = "./bin"
"#;
        let recipe: Recipe = toml::from_str(toml_str).unwrap();
        let item = &recipe.items["cli-tool"];
        let github = item.github.as_ref().unwrap();
        assert_eq!(github.repo, "owner/repo");
        assert_eq!(github.asset, Some("tool-linux-amd64.tar.gz".to_string()));
        assert_eq!(github.tag, Some("v1.0.0".to_string()));
    }

    #[test]
    fn test_parse_recipe_with_vars() {
        let toml_str = r#"
[vars]
version = "1.2.3"
platform = "linux"

[mypackage]
url = "https://example.com/file-${version}-${platform}.zip"
"#;
        let recipe: Recipe = toml::from_str(toml_str).unwrap();
        assert_eq!(recipe.vars.get("version"), Some(&"1.2.3".to_string()));
        assert_eq!(recipe.vars.get("platform"), Some(&"linux".to_string()));
        assert!(recipe.items.contains_key("mypackage"));
    }

    #[test]
    fn test_parse_recipe_with_lock() {
        let toml_str = r#"
[mypackage]
url = "https://example.com/file.zip"
lock = { sha = "abc123def456", download_url = "https://cdn.example.com/file.zip" }
"#;
        let recipe: Recipe = toml::from_str(toml_str).unwrap();
        let item = &recipe.items["mypackage"];
        let lock = item.lock.as_ref().unwrap();
        assert_eq!(lock.sha, Some("abc123def456".to_string()));
        assert_eq!(
            lock.download_url,
            Some("https://cdn.example.com/file.zip".to_string())
        );
    }

    #[test]
    fn test_parse_recipe_with_files_pattern() {
        let toml_str = r#"
[docs]
url = "https://example.com/repo.zip"
unzip_to = "./docs"
files = "*.md"
"#;
        let recipe: Recipe = toml::from_str(toml_str).unwrap();
        let item = &recipe.items["docs"];
        assert_eq!(item.files, Some("*.md".to_string()));
    }

    #[test]
    fn test_parse_recipe_with_executable_flag() {
        let toml_str = r#"
[binary]
url = "https://example.com/tool.tar.gz"
unzip_to = "./bin"
executable = true
"#;
        let recipe: Recipe = toml::from_str(toml_str).unwrap();
        let item = &recipe.items["binary"];
        assert_eq!(item.executable, Some(true));
    }

    #[test]
    fn test_parse_multiple_items() {
        let toml_str = r#"
[item1]
url = "https://example.com/file1.zip"

[item2]
url = "https://example.com/file2.zip"

[item3]
github = { repo = "owner/repo" }
"#;
        let recipe: Recipe = toml::from_str(toml_str).unwrap();
        assert_eq!(recipe.items.len(), 3);
        assert!(recipe.items.contains_key("item1"));
        assert!(recipe.items.contains_key("item2"));
        assert!(recipe.items.contains_key("item3"));
    }

    #[test]
    fn test_parse_empty_recipe() {
        let toml_str = "";
        let recipe: Recipe = toml::from_str(toml_str).unwrap();
        assert!(recipe.items.is_empty());
        assert!(recipe.vars.is_empty());
    }

    #[test]
    fn test_parse_recipe_with_install_exes() {
        let toml_str = r#"
[tools]
url = "https://example.com/tools.zip"
unzip_to = "./tools"
install_exes = ["bin/tool1", "bin/tool2"]
"#;
        let recipe: Recipe = toml::from_str(toml_str).unwrap();
        let item = &recipe.items["tools"];
        let exes = item.install_exes.as_ref().unwrap();
        assert_eq!(exes.len(), 2);
        assert_eq!(exes[0], "bin/tool1");
        assert_eq!(exes[1], "bin/tool2");
    }
}
