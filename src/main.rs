use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use flate2::read::GzDecoder;
use glob_match::glob_match;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use tar::Archive;
use zip::ZipArchive;

// Embed the scoop shim executable at compile time
#[cfg(windows)]
static SCOOP_SHIM_BYTES: &[u8] = include_bytes!("../shims/shim_scoop.exe");

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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
}

// TOML recipe format: HashMap where key is the section name (becomes tag)
type Recipe = HashMap<String, FetchItem>;

#[derive(Debug, Deserialize, Serialize, Clone)]
struct FetchItem {
    url: Option<String>,
    github: Option<GitHubFetch>,
    unzip_to: Option<String>,
    save_as: Option<String>,
    /// Optional glob pattern for files to extract from archives (extracts all if not specified)
    files: Option<String>,
    /// Optional AWS profile for S3 downloads
    profile: Option<String>,
    /// List of executables to install from the extracted directory (supports glob patterns)
    install_exes: Option<Vec<String>>,
    /// Install executable directly without creating shims (defaults to false on Windows)
    no_shim: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct GitHubFetch {
    repo: String,
    asset: Option<String>,
    tag: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

fn get_cache_dir() -> Result<std::path::PathBuf> {
    let temp_dir = std::env::temp_dir();
    let cache_dir = temp_dir.join("zipget-cache");
    fs::create_dir_all(&cache_dir)
        .with_context(|| format!("Failed to create cache directory: {}", cache_dir.display()))?;
    Ok(cache_dir)
}

fn is_s3_url(url: &str) -> bool {
    url.starts_with("s3://")
}

fn download_s3_file(s3_url: &str, local_path: &Path, profile: Option<&str>) -> Result<()> {
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

    if output.status.success() {
        // Get file size for consistent logging
        let file_size = fs::metadata(&temp_path)
            .with_context(|| format!("Failed to get file metadata: {}", temp_path.display()))?
            .len();

        // Atomically move the temporary file to the final location
        fs::rename(&temp_path, local_path).with_context(|| {
            let _ = fs::remove_file(&temp_path);
            format!(
                "Failed to move temporary file to final location: {} -> {}",
                temp_path.display(),
                local_path.display()
            )
        })?;

        println!("Downloaded: {} ({} bytes)", local_path.display(), file_size);
        Ok(())
    } else {
        // Clean up temporary file on failure
        let _ = fs::remove_file(&temp_path);

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Provide helpful error messages for common issues
        let error_msg = if stderr.contains("NoCredentialsError")
            || stderr.contains("Unable to locate credentials")
        {
            "AWS credentials not configured. Run 'aws configure' to set up credentials."
        } else if stderr.contains("NoSuchBucket") {
            "S3 bucket does not exist or you don't have access to it."
        } else if stderr.contains("NoSuchKey") {
            "S3 object does not exist."
        } else if stderr.contains("AccessDenied") {
            "Access denied. Check your AWS permissions for this S3 bucket/object."
        } else {
            "AWS S3 download failed"
        };

        Err(anyhow::anyhow!(
            "{}:\nSTDERR: {}\nSTDOUT: {}",
            error_msg,
            stderr.trim(),
            stdout.trim()
        ))
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::Recipe {
            file,
            tag,
            upgrade,
            profile,
        } => {
            if upgrade {
                upgrade_recipe(&file)?;
            } else {
                let recipe_content = fs::read_to_string(&file)
                    .with_context(|| format!("Failed to read recipe file: {file}"))?;

                let recipe: Recipe = toml::from_str(&recipe_content)
                    .with_context(|| "Failed to parse recipe TOML")?;

                // Filter items by tag if specified
                let items_to_process: Vec<FetchItem> = if let Some(filter_tag) = &tag {
                    recipe
                        .into_iter()
                        .filter(|(k, _)| k.contains(filter_tag))
                        .map(|(_, v)| v)
                        .collect()
                } else {
                    recipe.into_values().collect()
                };

                if items_to_process.is_empty() {
                    if let Some(filter_tag) = &tag {
                        println!("No items found with tag: {filter_tag}");
                    } else {
                        println!("No items to process in recipe");
                    }
                    return Ok(());
                }

                if let Some(filter_tag) = &tag {
                    println!(
                        "Processing {} items with tag: {}",
                        items_to_process.len(),
                        filter_tag
                    );
                } else {
                    println!(
                        "Processing all {} items from recipe",
                        items_to_process.len()
                    );
                }

                // Process each fetch item concurrently using threads
                let mut handles = Vec::new();

                for fetch_item in items_to_process {
                    let profile = profile.clone();

                    let handle = std::thread::spawn(move || {
                        process_fetch_item(&fetch_item, profile.as_deref())
                    });

                    handles.push(handle);
                }

                // Wait for all downloads to complete and collect any errors
                let mut errors = Vec::new();
                for (i, handle) in handles.into_iter().enumerate() {
                    match handle.join() {
                        Ok(Ok(())) => {
                            // Download succeeded
                        }
                        Ok(Err(e)) => {
                            errors.push(format!("Item {}: {}", i + 1, e));
                        }
                        Err(_) => {
                            errors.push(format!("Item {}: Thread panicked", i + 1));
                        }
                    }
                }

                if !errors.is_empty() {
                    return Err(anyhow::anyhow!(
                        "Some downloads failed:\n{}",
                        errors.join("\n")
                    ));
                }

                println!("All downloads and extractions completed successfully!");
            }
        }
        Commands::Github {
            repo,
            binary,
            save_as,
            tag,
            unzip_to,
            files,
        } => {
            fetch_github_release(
                &repo,
                binary.as_deref(),
                save_as.as_deref(),
                tag.as_deref(),
                unzip_to.as_deref(),
                files.as_deref(),
            )?;
        }
        Commands::Fetch {
            url,
            save_as,
            unzip_to,
            files,
            profile,
        } => {
            fetch_direct_url(
                &url,
                save_as.as_deref(),
                unzip_to.as_deref(),
                files.as_deref(),
                profile.as_deref(),
            )?;
        }
        Commands::Run {
            source,
            binary,
            tag,
            files,
            profile,
            executable,
            args,
        } => {
            run_package(
                &source,
                binary.as_deref(),
                tag.as_deref(),
                files.as_deref(),
                profile.as_deref(),
                executable.as_deref(),
                &args,
            )?;
        }
        Commands::Install {
            source,
            binary,
            tag,
            files,
            profile,
            executable,
            no_shim,
        } => {
            install_package(
                &source,
                binary.as_deref(),
                tag.as_deref(),
                files.as_deref(),
                profile.as_deref(),
                executable.as_deref(),
                no_shim,
            )?;
        }
    }

    Ok(())
}

fn process_fetch_item(fetch_item: &FetchItem, global_profile: Option<&str>) -> Result<()> {
    let cache_dir = get_cache_dir()?;

    let (download_url, filename) = if let Some(url) = &fetch_item.url {
        println!("Processing URL: {url}");
        let filename = get_filename_from_url(url);
        (url.clone(), filename)
    } else if let Some(github) = &fetch_item.github {
        println!("Processing GitHub repo: {}", github.repo);

        let (download_url, filename) = if let Some(asset_name) = &github.asset {
            // User specified asset name - use the existing logic
            println!("Using specified asset: {asset_name}");
            let github_url =
                get_github_release_url(&github.repo, asset_name, github.tag.as_deref())?;
            let filename = get_filename_from_url(&github_url);
            (github_url, filename)
        } else {
            // No asset specified - use intelligent asset detection
            println!("No asset specified, analyzing available assets...");
            let (release, best_asset) =
                get_best_binary_from_release(&github.repo, github.tag.as_deref())?;

            // Find the matching asset URL
            let asset = release
                .assets
                .iter()
                .find(|asset| asset.name == best_asset)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Selected asset '{}' not found in release assets",
                        best_asset
                    )
                })?;

            println!("Selected asset: {} ({} bytes)", asset.name, asset.size);
            let filename = get_filename_from_url(&asset.browser_download_url);
            (asset.browser_download_url.clone(), filename)
        };

        (download_url, filename)
    } else {
        return Err(anyhow::anyhow!(
            "FetchItem must have either 'url' or 'github' specified"
        ));
    };

    let url_hash = format!("{:x}", md5::compute(&download_url));
    let cached_filename = format!("{url_hash}_{filename}");
    let cached_file_path = cache_dir.join(&cached_filename);

    // Use the appropriate profile - item-specific profile overrides global profile
    let profile = fetch_item.profile.as_deref().or(global_profile);

    let file_path = if cached_file_path.exists() {
        println!("Found cached file: {}", cached_file_path.display());
        cached_file_path
    } else {
        println!("Downloading: {download_url}");
        download_file(&download_url, &cached_file_path, profile)?;
        cached_file_path
    };

    // Save the file if save_as is specified
    if let Some(save_as) = &fetch_item.save_as {
        let save_path = Path::new(save_as);
        if let Some(parent) = save_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        fs::copy(&file_path, save_path)
            .with_context(|| format!("Failed to copy file to: {}", save_path.display()))?;
        println!("Saved as: {save_as}");
    }

    // Extract the archive if unzip_to is specified
    if let Some(unzip_to) = &fetch_item.unzip_to {
        println!("Extracting to: {unzip_to}");
        extract_archive(&file_path, unzip_to, fetch_item.files.as_deref())?;
    }

    Ok(())
}

fn download_file(url: &str, path: &Path, profile: Option<&str>) -> Result<()> {
    if is_s3_url(url) {
        download_s3_file(url, path, profile)
    } else {
        let response = ureq::get(url)
            .call()
            .with_context(|| format!("Failed to download: {url}"))?;

        if response.status() != 200 {
            return Err(anyhow::anyhow!(
                "Download failed with status: {}",
                response.status()
            ));
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        // Create a temporary file in the same directory as the target file
        let temp_path = path.with_extension(format!(
            "{}.tmp",
            path.extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("download")
        ));

        // Download to temporary file first
        let mut temp_file = fs::File::create(&temp_path)
            .with_context(|| format!("Failed to create temporary file: {}", temp_path.display()))?;

        std::io::copy(&mut response.into_reader(), &mut temp_file).with_context(|| {
            // Clean up temporary file on failure
            let _ = fs::remove_file(&temp_path);
            format!("Failed to write to temporary file: {}", temp_path.display())
        })?;

        // Ensure data is written to disk
        temp_file.sync_all().with_context(|| {
            let _ = fs::remove_file(&temp_path);
            format!("Failed to sync temporary file: {}", temp_path.display())
        })?;

        let file_size = temp_file.metadata()?.len();
        drop(temp_file); // Close the file handle

        // Atomically move the temporary file to the final location
        fs::rename(&temp_path, path).with_context(|| {
            let _ = fs::remove_file(&temp_path);
            format!(
                "Failed to move temporary file to final location: {} -> {}",
                temp_path.display(),
                path.display()
            )
        })?;

        println!("Downloaded: {} ({} bytes)", path.display(), file_size);
        Ok(())
    }
}

fn extract_zip(zip_path: &Path, extract_to: &str, file_pattern: Option<&str>) -> Result<()> {
    let file = fs::File::open(zip_path)
        .with_context(|| format!("Failed to open zip file: {}", zip_path.display()))?;

    let mut archive = ZipArchive::new(file).with_context(|| "Failed to read zip archive")?;

    fs::create_dir_all(extract_to)
        .with_context(|| format!("Failed to create extraction directory: {extract_to}"))?;

    let mut extracted_count = 0;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .with_context(|| format!("Failed to access zip entry {i}"))?;

        // Check if file matches the glob pattern (if specified)
        if let Some(pattern) = file_pattern {
            let filename = Path::new(file.name())
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if !glob_match(pattern, file.name()) && !glob_match(pattern, filename) {
                continue; // Skip files that don't match the pattern (checking both full path and filename)
            }
        }

        let outpath = Path::new(extract_to).join(file.mangled_name());

        if file.name().ends_with('/') {
            // Directory
            fs::create_dir_all(&outpath)
                .with_context(|| format!("Failed to create directory: {}", outpath.display()))?;
        } else {
            // File
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p).with_context(|| {
                        format!("Failed to create parent directory: {}", p.display())
                    })?;
                }
            }

            let mut outfile = fs::File::create(&outpath).with_context(|| {
                format!("Failed to create extracted file: {}", outpath.display())
            })?;

            std::io::copy(&mut file, &mut outfile)
                .with_context(|| format!("Failed to extract file: {}", outpath.display()))?;
        }

        // Set file permissions on Unix-like systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))?;
            }
        }

        extracted_count += 1;
    }

    if let Some(pattern) = file_pattern {
        println!(
            "Extracted {extracted_count} files matching pattern '{pattern}'"
        );
    } else {
        println!("Extracted {extracted_count} files");
    }
    Ok(())
}

fn extract_tar_gz(tar_path: &Path, extract_to: &str, file_pattern: Option<&str>) -> Result<()> {
    let file = fs::File::open(tar_path)
        .with_context(|| format!("Failed to open tar.gz file: {}", tar_path.display()))?;

    let tar = GzDecoder::new(file);
    let mut archive = Archive::new(tar);

    fs::create_dir_all(extract_to)
        .with_context(|| format!("Failed to create extraction directory: {extract_to}"))?;

    let mut extracted_count = 0;

    for entry in archive
        .entries()
        .with_context(|| "Failed to read tar.gz entries")?
    {
        let mut entry = entry.with_context(|| "Failed to access tar.gz entry")?;

        let path = entry.path().with_context(|| "Failed to get entry path")?;

        let path_str = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in path"))?;

        // Check if file matches the glob pattern (if specified)
        if let Some(pattern) = file_pattern {
            let filename = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if !glob_match(pattern, path_str) && !glob_match(pattern, filename) {
                continue; // Skip files that don't match the pattern (checking both full path and filename)
            }
        }

        let outpath = Path::new(extract_to).join(&path);

        // Create parent directories if they don't exist
        if let Some(parent) = outpath.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create parent directory: {}", parent.display())
                })?;
            }
        }

        // Extract the entry
        entry
            .unpack(&outpath)
            .with_context(|| format!("Failed to extract file: {}", outpath.display()))?;

        extracted_count += 1;
    }

    if let Some(pattern) = file_pattern {
        println!(
            "Extracted {extracted_count} files matching pattern '{pattern}'"
        );
    } else {
        println!("Extracted {extracted_count} files");
    }
    Ok(())
}

fn extract_archive(
    archive_path: &Path,
    extract_to: &str,
    file_pattern: Option<&str>,
) -> Result<()> {
    let filename = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    if filename.ends_with(".zip") {
        extract_zip(archive_path, extract_to, file_pattern)
    } else if filename.ends_with(".tar.gz") || filename.ends_with(".tgz") {
        extract_tar_gz(archive_path, extract_to, file_pattern)
    } else {
        // Try to detect by content or fall back to zip
        println!(
            "Warning: Unknown archive type for '{}', attempting ZIP extraction",
            archive_path.display()
        );
        extract_zip(archive_path, extract_to, file_pattern)
    }
}

fn get_filename_from_url(url: &str) -> String {
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

fn guess_binary_name() -> String {
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

fn find_best_matching_binary(assets: &[GitHubAsset]) -> Option<String> {
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

    let mut best_score = 0;
    let mut best_asset = None;

    for asset in assets {
        let name_lower = asset.name.to_lowercase();
        let mut score = 0;

        // Score OS match (higher weight)
        for (i, pattern) in os_patterns.iter().enumerate() {
            if name_lower.contains(pattern) {
                score += 100 - (i * 10); // First match gets 100, second gets 90, etc.
                break;
            }
        }

        // Score architecture match (medium weight)
        for (i, pattern) in arch_patterns.iter().enumerate() {
            if name_lower.contains(pattern) {
                score += 50 - (i * 5); // First match gets 50, second gets 45, etc.
                break;
            }
        }

        // Bonus for common binary extensions/patterns
        if name_lower.ends_with(".zip")
            || name_lower.ends_with(".tar.gz")
            || name_lower.ends_with(".tgz")
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

fn get_best_binary_from_release(repo: &str, tag: Option<&str>) -> Result<(GitHubRelease, String)> {
    let api_url = if let Some(tag) = tag {
        format!(
            "https://api.github.com/repos/{repo}/releases/tags/{tag}"
        )
    } else {
        format!("https://api.github.com/repos/{repo}/releases/latest")
    };

    println!("Analyzing available binaries from: {api_url}");

    let response = ureq::get(&api_url)
        .set("User-Agent", "zipget-rs")
        .call()
        .with_context(|| format!("Failed to fetch release info for {repo}"))?;

    if response.status() != 200 {
        return Err(anyhow::anyhow!(
            "GitHub API request failed with status: {}",
            response.status()
        ));
    }

    let release: GitHubRelease = response
        .into_json()
        .with_context(|| "Failed to parse GitHub release JSON")?;

    println!(
        "Found {} assets in release '{}':",
        release.assets.len(),
        release.name
    );
    for asset in &release.assets {
        println!("  - {}", asset.name);
    }

    let best_match = if let Some(best_match) = find_best_matching_binary(&release.assets) {
        println!("Selected best match: {best_match}");
        best_match
    } else {
        // Fallback to old behavior
        println!("No good match found, falling back to basic guess");
        guess_binary_name()
    };

    Ok((release, best_match))
}

fn fetch_github_release(
    repo: &str,
    binary_name: Option<&str>,
    save_as: Option<&str>,
    tag: Option<&str>,
    unzip_to: Option<&str>,
    files_pattern: Option<&str>,
) -> Result<()> {
    let (release, binary_name) = if let Some(name) = binary_name {
        // User specified binary name, fetch release separately
        let api_url = if let Some(tag) = tag {
            format!(
                "https://api.github.com/repos/{repo}/releases/tags/{tag}"
            )
        } else {
            format!("https://api.github.com/repos/{repo}/releases/latest")
        };

        println!("Fetching release info from: {api_url}");

        let response = ureq::get(&api_url)
            .set("User-Agent", "zipget-rs")
            .call()
            .with_context(|| format!("Failed to fetch release info for {repo}"))?;

        if response.status() != 200 {
            return Err(anyhow::anyhow!(
                "GitHub API request failed with status: {}",
                response.status()
            ));
        }

        let release: GitHubRelease = response
            .into_json()
            .with_context(|| "Failed to parse GitHub release JSON")?;

        (release, name.to_string())
    } else {
        println!(
            "No binary specified for {repo}, analyzing available assets..."
        );
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

    // Use caching mechanism (same as process_fetch_item)
    let cache_dir = get_cache_dir()?;
    let download_url = &asset.browser_download_url;
    let filename = get_filename_from_url(download_url);

    let url_hash = format!("{:x}", md5::compute(download_url));
    let cached_filename = format!("{url_hash}_{filename}");
    let cached_file_path = cache_dir.join(&cached_filename);

    let file_path = if cached_file_path.exists() {
        println!("Found cached file: {}", cached_file_path.display());
        cached_file_path
    } else {
        // Download the file
        println!("Downloading: {download_url}");
        download_file(download_url, &cached_file_path, None)?;
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
    } else {
        // If no save_as specified, copy to current directory with original filename
        let output_path = Path::new(".").join(&filename);
        fs::copy(&file_path, &output_path)
            .with_context(|| format!("Failed to copy file to: {}", output_path.display()))?;
        println!("Saved as: {}", output_path.display());
    }

    // Extract if unzip_to is specified
    if let Some(unzip_to) = unzip_to {
        println!("Extracting to: {unzip_to}");
        extract_archive(&file_path, unzip_to, files_pattern)?;
    }

    Ok(())
}

fn fetch_direct_url(
    url: &str,
    save_as: Option<&str>,
    unzip_to: Option<&str>,
    files_pattern: Option<&str>,
    profile: Option<&str>,
) -> Result<()> {
    println!("Fetching from URL: {url}");

    // Use caching mechanism (same as fetch_github_release)
    let cache_dir = get_cache_dir()?;
    let filename = get_filename_from_url(url);

    let url_hash = format!("{:x}", md5::compute(url));
    let cached_filename = format!("{url_hash}_{filename}");
    let cached_file_path = cache_dir.join(&cached_filename);

    let file_path = if cached_file_path.exists() {
        println!("Found cached file: {}", cached_file_path.display());
        cached_file_path
    } else {
        // Download the file
        println!("Downloading: {url}");
        download_file(url, &cached_file_path, profile)?;
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
    } else {
        // If no save_as specified, copy to current directory with original filename
        let output_path = Path::new(".").join(&filename);
        fs::copy(&file_path, &output_path)
            .with_context(|| format!("Failed to copy file to: {}", output_path.display()))?;
        println!("Saved as: {}", output_path.display());
    }

    // Extract if unzip_to is specified
    if let Some(unzip_to) = unzip_to {
        println!("Extracting to: {unzip_to}");
        extract_archive(&file_path, unzip_to, files_pattern)?;
    }

    Ok(())
}

fn get_github_release_url(repo: &str, binary_name: &str, tag: Option<&str>) -> Result<String> {
    let api_url = if let Some(tag) = tag {
        format!(
            "https://api.github.com/repos/{repo}/releases/tags/{tag}"
        )
    } else {
        format!("https://api.github.com/repos/{repo}/releases/latest")
    };

    let response = ureq::get(&api_url)
        .set("User-Agent", "zipget-rs")
        .call()
        .with_context(|| format!("Failed to fetch release info for {repo}"))?;

    if response.status() != 200 {
        return Err(anyhow::anyhow!(
            "GitHub API request failed with status: {}",
            response.status()
        ));
    }

    let release: GitHubRelease = response
        .into_json()
        .with_context(|| "Failed to parse GitHub release JSON")?;

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

    Ok(asset.browser_download_url.clone())
}

fn get_latest_github_tag(repo: &str) -> Result<String> {
    let api_url = format!("https://api.github.com/repos/{repo}/releases/latest");

    let response = ureq::get(&api_url)
        .set("User-Agent", "zipget-rs")
        .call()
        .with_context(|| format!("Failed to fetch latest release for {repo}"))?;

    if response.status() != 200 {
        return Err(anyhow::anyhow!(
            "GitHub API request failed with status: {}",
            response.status()
        ));
    }

    let release: GitHubRelease = response
        .into_json()
        .with_context(|| "Failed to parse GitHub release JSON")?;

    Ok(release.tag_name)
}

fn upgrade_recipe(file_path: &str) -> Result<()> {
    println!("Upgrading recipe: {file_path}");

    let recipe_content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read recipe file: {file_path}"))?;

    let mut recipe: Recipe =
        toml::from_str(&recipe_content).with_context(|| "Failed to parse recipe TOML")?;

    let mut updated = false;

    for fetch_item in recipe.values_mut() {
        if let Some(github) = &mut fetch_item.github {
            // Ensure binary field has a value for consistent processing
            if github.asset.is_none() {
                let guessed = guess_binary_name();
                println!(
                    "No binary specified for {}, setting to: {}",
                    github.repo, guessed
                );
                github.asset = Some(guessed);
                updated = true;
            }

            println!("Checking latest version for {}", github.repo);

            match get_latest_github_tag(&github.repo) {
                Ok(latest_tag) => {
                    let current_tag = github.tag.as_deref().unwrap_or("latest");
                    if github.tag.is_none() || github.tag.as_ref().unwrap() != &latest_tag {
                        println!("  {current_tag} -> {latest_tag}");
                        github.tag = Some(latest_tag);
                        updated = true;
                    } else {
                        println!("  {current_tag} (already latest)");
                    }
                }
                Err(e) => {
                    println!("  Failed to get latest version: {e}");
                }
            }
        }
    }

    if updated {
        let updated_content = toml::to_string_pretty(&recipe)
            .with_context(|| "Failed to serialize updated recipe")?;

        fs::write(file_path, updated_content)
            .with_context(|| format!("Failed to write updated recipe to {file_path}"))?;

        println!("Recipe updated successfully!");
    } else {
        println!("All GitHub releases are already at their latest versions.");
    }

    Ok(())
}

fn run_package(
    source: &str,
    binary: Option<&str>,
    tag: Option<&str>,
    files_pattern: Option<&str>,
    profile: Option<&str>,
    executable: Option<&str>,
    args: &[String],
) -> Result<()> {
    // Create a temporary directory for extraction
    let temp_dir = std::env::temp_dir().join(format!("zipget-run-{}", std::process::id()));
    fs::create_dir_all(&temp_dir)
        .with_context(|| format!("Failed to create temp directory: {}", temp_dir.display()))?;

    // Determine if source is a GitHub repo or direct URL
    let is_github_repo = !source.starts_with("http") && !source.starts_with("s3://");

    let cached_file_path = if is_github_repo {
        // Handle GitHub repository
        let binary_name = if let Some(name) = binary {
            name.to_string()
        } else {
            println!(
                "No binary specified for {source}, analyzing available assets..."
            );
            let (_release, best_match) = get_best_binary_from_release(source, tag)?;
            best_match
        };

        let download_url = get_github_release_url(source, &binary_name, tag)?;

        // Use caching mechanism
        let cache_dir = get_cache_dir()?;
        let filename = get_filename_from_url(&download_url);
        let url_hash = format!("{:x}", md5::compute(&download_url));
        let cached_filename = format!("{url_hash}_{filename}");
        let cached_file_path = cache_dir.join(&cached_filename);

        if cached_file_path.exists() {
            println!("Found cached file: {}", cached_file_path.display());
        } else {
            println!("Downloading: {download_url}");
            download_file(&download_url, &cached_file_path, profile)?;
        }

        cached_file_path
    } else {
        // Handle direct URL
        let cache_dir = get_cache_dir()?;
        let filename = get_filename_from_url(source);
        let url_hash = format!("{:x}", md5::compute(source));
        let cached_filename = format!("{url_hash}_{filename}");
        let cached_file_path = cache_dir.join(&cached_filename);

        if cached_file_path.exists() {
            println!("Found cached file: {}", cached_file_path.display());
        } else {
            println!("Downloading: {source}");
            download_file(source, &cached_file_path, profile)?;
        }

        cached_file_path
    };

    // Extract the archive to the temporary directory
    println!("Extracting to: {}", temp_dir.display());
    extract_archive(&cached_file_path, temp_dir.to_str().unwrap(), files_pattern)?;

    // Find executable files in the extracted directory
    let executables = find_executables(&temp_dir)?;

    let executable_to_run = if let Some(exe_name) = executable {
        // User specified an executable name
        let matching_exe = executables
            .iter()
            .find(|exe| exe.file_name().unwrap_or_default().to_string_lossy() == exe_name)
            .or_else(|| {
                executables
                    .iter()
                    .find(|exe| exe.to_string_lossy().contains(exe_name))
            })
            .ok_or_else(|| anyhow::anyhow!("Executable '{}' not found in package", exe_name))?;
        matching_exe.clone()
    } else if executables.len() == 1 {
        // Only one executable found, use it
        executables[0].clone()
    } else if executables.is_empty() {
        return Err(anyhow::anyhow!("No executable files found in the package"));
    } else {
        // Multiple executables found, list them and require user to specify
        println!("Multiple executables found:");
        for exe in &executables {
            println!(
                "  {}",
                exe.file_name().unwrap_or_default().to_string_lossy()
            );
        }
        return Err(anyhow::anyhow!(
            "Multiple executables found. Please specify which one to run using --exe <name>"
        ));
    };

    // Run the executable
    println!("Running executable: {}", executable_to_run.display());
    let mut command = Command::new(&executable_to_run);
    command.args(args);

    let status = command
        .status()
        .with_context(|| format!("Failed to execute: {}", executable_to_run.display()))?;

    // Clean up temporary directory
    fs::remove_dir_all(&temp_dir)
        .with_context(|| format!("Failed to clean up temp directory: {}", temp_dir.display()))?;

    if !status.success() {
        return Err(anyhow::anyhow!("Executable exited with status: {}", status));
    }

    Ok(())
}

fn find_executables(dir: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut executables = Vec::new();

    fn visit_dir(dir: &Path, executables: &mut Vec<std::path::PathBuf>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                visit_dir(&path, executables)?;
            } else if is_executable(&path)? {
                executables.push(path);
            }
        }
        Ok(())
    }

    visit_dir(dir, &mut executables)?;
    Ok(executables)
}

fn is_executable(path: &Path) -> Result<bool> {
    let metadata = fs::metadata(path)?;

    // Check if it's a regular file
    if !metadata.is_file() {
        return Ok(false);
    }

    // On Windows, check for .exe extension
    #[cfg(windows)]
    {
        if let Some(ext) = path.extension() {
            if ext.to_string_lossy().to_lowercase() == "exe" {
                return Ok(true);
            }
        }
        Ok(false)
    }

    // On Unix-like systems, check for execute permission
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode();
        return Ok(mode & 0o111 != 0); // Check if any execute bit is set
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)
        .with_context(|| format!("Failed to create destination directory: {}", dst.display()))?;

    for entry in fs::read_dir(src)
        .with_context(|| format!("Failed to read source directory: {}", src.display()))?
    {
        let entry = entry.with_context(|| "Failed to read directory entry")?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).with_context(|| {
                format!(
                    "Failed to copy {} to {}",
                    src_path.display(),
                    dst_path.display()
                )
            })?;
        }
    }

    Ok(())
}



fn install_package(
    source: &str,
    binary: Option<&str>,
    tag: Option<&str>,
    files_pattern: Option<&str>,
    profile: Option<&str>,
    executable: Option<&str>,
    no_shim: bool,
) -> Result<()> {
    if !no_shim {
        #[cfg(not(windows))]
        {
            return Err(anyhow::anyhow!(
                "The install command with shims is only available on Windows. Use --no-shim to install directly to ~/.local/bin"
            ));
        }
    }

    // Create a temporary directory for extraction
    let temp_dir = std::env::temp_dir().join(format!("zipget-install-{}", std::process::id()));
    fs::create_dir_all(&temp_dir)
        .with_context(|| format!("Failed to create temp directory: {}", temp_dir.display()))?;

    // Download and extract to temp directory
    let cache_dir = get_cache_dir()?;
    let (download_url, filename) = if source.contains('/') && !source.starts_with("http") {
        // Treat as GitHub repo
        let binary_name = if let Some(name) = binary {
            name.to_string()
        } else {
            println!(
                "No binary specified for {source}, analyzing available assets..."
            );
            let (_release, best_match) = get_best_binary_from_release(source, tag)?;
            best_match
        };

        println!(
            "Processing GitHub repo: {source}, binary: {binary_name}"
        );
        let github_url = get_github_release_url(source, &binary_name, tag)?;
        let filename = get_filename_from_url(&github_url);
        (github_url, filename)
    } else {
        // Treat as direct URL
        println!("Processing URL: {source}");
        let filename = get_filename_from_url(source);
        (source.to_string(), filename)
    };

    let url_hash = format!("{:x}", md5::compute(&download_url));
    let cached_filename = format!("{url_hash}_{filename}");
    let cached_file_path = cache_dir.join(&cached_filename);

    let file_path = if cached_file_path.exists() {
        println!("Found cached file: {}", cached_file_path.display());
        cached_file_path
    } else {
        println!("Downloading: {download_url}");
        download_file(&download_url, &cached_file_path, profile)?;
        cached_file_path
    };

    // Extract to temp directory
    println!("Extracting to temporary directory: {}", temp_dir.display());
    extract_archive(&file_path, temp_dir.to_str().unwrap(), files_pattern)?;

    // Find executables in the extracted directory
    let executables = find_executables(&temp_dir)?;

    if executables.is_empty() {
        return Err(anyhow::anyhow!(
            "No executables found in the extracted archive"
        ));
    }

    // Create ~/.local/bin directory
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let local_bin_dir = home_dir.join(".local").join("bin");
    fs::create_dir_all(&local_bin_dir).with_context(|| {
        format!(
            "Failed to create ~/.local/bin directory: {}",
            local_bin_dir.display()
        )
    })?;

    // Determine which executables to install
    let executables_to_install: Vec<&std::path::PathBuf> = if let Some(exe_name) = executable {
        // User specified an executable name
        let target_exe = executables
            .iter()
            .find(|path| {
                path.file_stem()
                    .and_then(|name| name.to_str())
                    .map(|name| name.eq_ignore_ascii_case(exe_name))
                    .unwrap_or(false)
            })
            .ok_or_else(|| anyhow::anyhow!("Executable '{}' not found in archive", exe_name))?;
        vec![target_exe]
    } else {
        // Install all executables found
        if executables.len() > 1 {
            println!("Installing {} executables:", executables.len());
            for (i, exe) in executables.iter().enumerate() {
                println!(
                    "  {}: {}",
                    i + 1,
                    exe.file_name().unwrap().to_string_lossy()
                );
            }
        }
        executables.iter().collect()
    };

    if no_shim {
        // Install directly to ~/.local/bin
        let mut installed_executables = Vec::new();
        for target_exe in executables_to_install {
            let exe_name = target_exe
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid executable name"))?;

            // Copy the executable directly to ~/.local/bin
            let installed_exe = local_bin_dir.join(exe_name);
            fs::copy(target_exe, &installed_exe).with_context(|| {
                format!("Failed to copy executable to {}", installed_exe.display())
            })?;

            // Make the file executable on Unix systems
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&installed_exe)?.permissions();
                perms.set_mode(perms.mode() | 0o755); // Add execute permissions
                fs::set_permissions(&installed_exe, perms).with_context(|| {
                    format!(
                        "Failed to set executable permissions on {}",
                        installed_exe.display()
                    )
                })?;
            }

            installed_executables.push((exe_name.to_string_lossy().to_string(), installed_exe));
        }

        // Clean up temp directory
        let _ = fs::remove_dir_all(&temp_dir);

        // Print summary
        if installed_executables.len() == 1 {
            let (exe_name, installed_exe) = &installed_executables[0];
            println!("Successfully installed {exe_name}!");
            println!("Executable: {}", installed_exe.display());
        } else {
            println!(
                "Successfully installed {} executables!",
                installed_executables.len()
            );
            for (exe_name, installed_exe) in &installed_executables {
                println!("  {}: {}", exe_name, installed_exe.display());
            }
        }
        println!(
            "Make sure {} is in your PATH to use the executables",
            local_bin_dir.display()
        );
    } else {
        // Create shims (Windows only)
        #[cfg(windows)]
        {
            use std::env;

            // Get LOCALAPPDATA directory
            let local_app_data = env::var("LOCALAPPDATA")
                .with_context(|| "LOCALAPPDATA environment variable not found")?;
            let programs_dir = Path::new(&local_app_data).join("Programs");

            // Create Programs directory if it doesn't exist
            fs::create_dir_all(&programs_dir).with_context(|| {
                format!(
                    "Failed to create Programs directory: {}",
                    programs_dir.display()
                )
            })?;

            // Determine the app directory name based on source
            let app_name = if source.contains('/') && !source.starts_with("http") {
                // GitHub repo: use owner_repo format
                source.replace('/', "_")
            } else {
                // URL: try to extract a reasonable name from the filename
                let filename = get_filename_from_url(source);
                Path::new(&filename)
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or("zipget_install")
                    .to_string()
            };

            // Create app directory in Programs
            let app_dir = programs_dir.join(&app_name);
            fs::create_dir_all(&app_dir).with_context(|| {
                format!("Failed to create app directory: {}", app_dir.display())
            })?;

            // Copy all files from temp directory to app directory
            println!("Installing all files to: {}", app_dir.display());
            copy_dir_all(&temp_dir, &app_dir)
                .with_context(|| format!("Failed to copy files to {}", app_dir.display()))?;

            // Install shims for each executable
            let mut installed_executables = Vec::new();
            for target_exe in executables_to_install {
                let exe_name = target_exe
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| anyhow::anyhow!("Invalid executable name"))?;

                // Find the installed executable path (relative to temp_dir)
                let relative_path = target_exe.strip_prefix(&temp_dir).with_context(|| {
                    format!("Failed to get relative path for {}", target_exe.display())
                })?;
                let installed_exe = app_dir.join(relative_path);

                println!("Creating shim for executable: {}", installed_exe.display());

                // Create shim file
                let shim_file = local_bin_dir.join(format!("{exe_name}.shim"));
                let shim_content = format!("path = {}\nargs =", installed_exe.display());
                fs::write(&shim_file, shim_content).with_context(|| {
                    format!("Failed to create shim file: {}", shim_file.display())
                })?;

                // Create shim executable (copy of embedded scoop shim)
                let shim_exe = local_bin_dir.join(format!("{exe_name}.exe"));
                fs::write(&shim_exe, SCOOP_SHIM_BYTES).with_context(|| {
                    format!("Failed to create shim executable: {}", shim_exe.display())
                })?;

                installed_executables.push((exe_name.to_string(), installed_exe, shim_exe));
            }

            // Clean up temp directory
            let _ = fs::remove_dir_all(&temp_dir);

            // Print summary
            if installed_executables.len() == 1 {
                let (exe_name, installed_exe, shim_exe) = &installed_executables[0];
                println!("Successfully installed {exe_name}!");
                println!("Executable: {}", installed_exe.display());
                println!("Shim: {}", shim_exe.display());
            } else {
                println!(
                    "Successfully installed {} executables!",
                    installed_executables.len()
                );
                for (exe_name, installed_exe, shim_exe) in &installed_executables {
                    println!(
                        "  {}: {} -> {}",
                        exe_name,
                        installed_exe.display(),
                        shim_exe.display()
                    );
                }
            }
            println!(
                "Add {} to your PATH to use the shims",
                local_bin_dir.display()
            );
        }
    }

    Ok(())
}
