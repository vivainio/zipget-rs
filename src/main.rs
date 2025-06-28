use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use flate2::read::GzDecoder;
use glob_match::glob_match;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tar::Archive;
use zip::ZipArchive;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Process a recipe file to download and extract packages
    Recipe {
        /// Recipe file path
        file: String,
        /// Optional tag to filter items by
        tag: Option<String>,
        /// Upgrade all GitHub releases to latest versions
        #[arg(long)]
        upgrade: bool,
    },
    /// Fetch the latest release binary from a GitHub repository
    Github {
        /// GitHub repository in format "owner/repo"
        repo: String,
        /// Name of the binary to download from release assets (auto-detected if not specified)
        #[arg(value_name = "BINARY")]
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
}

#[derive(Debug, Deserialize, Serialize)]
struct Recipe {
    fetch: Vec<FetchItem>,
}

#[derive(Debug, Deserialize, Serialize)]
struct FetchItem {
    url: Option<String>,
    github: Option<GitHubFetch>,
    #[serde(rename = "unzipTo")]
    unzip_to: Option<String>,
    #[serde(rename = "saveAs")]
    save_as: Option<String>,
    /// Optional glob pattern for files to extract from ZIP (extracts all if not specified)
    files: Option<String>,
    /// Optional tags for filtering
    tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct GitHubFetch {
    repo: String,
    binary: Option<String>,
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

fn main() -> Result<()> {
    let args = Args::parse();
    
    match args.command {
        Commands::Recipe { file, tag, upgrade } => {
            if upgrade {
                upgrade_recipe(&file)?;
            } else {
                let recipe_content = fs::read_to_string(&file)
                    .with_context(|| format!("Failed to read recipe file: {}", file))?;
                
                let recipe: Recipe = serde_json::from_str(&recipe_content)
                    .with_context(|| "Failed to parse recipe JSON")?;
                
                // Filter items by tag if specified
                let items_to_process: Vec<&FetchItem> = if let Some(filter_tag) = &tag {
                    recipe.fetch.iter()
                        .filter(|item| {
                            item.tags.as_ref()
                                .map(|tags| tags.contains(filter_tag))
                                .unwrap_or(false)
                        })
                        .collect()
                } else {
                    recipe.fetch.iter().collect()
                };
                
                if items_to_process.is_empty() {
                    if let Some(filter_tag) = &tag {
                        println!("No items found with tag: {}", filter_tag);
                    } else {
                        println!("No items to process in recipe");
                    }
                    return Ok(());
                }
                
                if let Some(filter_tag) = &tag {
                    println!("Processing {} items with tag: {}", items_to_process.len(), filter_tag);
                } else {
                    println!("Processing all {} items from recipe", items_to_process.len());
                }
                
                // Process each fetch item
                for fetch_item in items_to_process {
                    process_fetch_item(fetch_item)?;
                }
                
                println!("All downloads and extractions completed successfully!");
            }
        }
        Commands::Github { repo, binary, save_as, tag, unzip_to, files } => {
            fetch_github_release(&repo, binary.as_deref(), save_as.as_deref(), tag.as_deref(), unzip_to.as_deref(), files.as_deref())?;
        }
    }
    
    Ok(())
}

fn process_fetch_item(fetch_item: &FetchItem) -> Result<()> {
    let cache_dir = get_cache_dir()?;
    
    let (download_url, filename) = if let Some(url) = &fetch_item.url {
        println!("Processing URL: {}", url);
        let filename = get_filename_from_url(url);
        (url.clone(), filename)
    } else if let Some(github) = &fetch_item.github {
        let binary_name = github.binary.as_deref().unwrap_or_else(|| {
            let guessed = guess_binary_name();
            println!("No binary specified for {}, guessing: {}", github.repo, guessed);
            Box::leak(guessed.into_boxed_str())
        });
        
        println!("Processing GitHub repo: {}, binary: {}", github.repo, binary_name);
        
        // Get the release download URL
        let github_url = get_github_release_url(&github.repo, binary_name, github.tag.as_deref())?;
        let filename = get_filename_from_url(&github_url);
        (github_url, filename)
    } else {
        return Err(anyhow::anyhow!("FetchItem must have either 'url' or 'github' specified"));
    };
    
    let url_hash = format!("{:x}", md5::compute(&download_url));
    let cached_filename = format!("{}_{}", url_hash, filename);
    let cached_file_path = cache_dir.join(&cached_filename);
    
    let file_path = if cached_file_path.exists() {
        println!("Found cached file: {}", cached_file_path.display());
        cached_file_path
    } else {
        // Download the file
        println!("Downloading: {}", download_url);
        download_file(&download_url, &cached_file_path)?;
        cached_file_path
    };
    
    // Save as specified file if requested
    if let Some(save_as) = &fetch_item.save_as {
        let save_path = Path::new(save_as);
        if let Some(parent) = save_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
        fs::copy(&file_path, save_path)
            .with_context(|| format!("Failed to save file as: {}", save_as))?;
        println!("Saved as: {}", save_as);
    }
    
    // Extract if unzipTo is specified
    if let Some(unzip_to) = &fetch_item.unzip_to {
        println!("Extracting to: {}", unzip_to);
        extract_archive(&file_path, unzip_to, fetch_item.files.as_deref())?;
    }
    
    Ok(())
}

fn download_file(url: &str, path: &Path) -> Result<()> {
    let response = ureq::get(url).call()
        .with_context(|| format!("Failed to download: {}", url))?;
    
    if response.status() != 200 {
        return Err(anyhow::anyhow!("Download failed with status: {}", response.status()));
    }
    
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }
    
    let mut file = fs::File::create(path)
        .with_context(|| format!("Failed to create file: {}", path.display()))?;
    
    std::io::copy(&mut response.into_reader(), &mut file)
        .with_context(|| format!("Failed to write to file: {}", path.display()))?;
    
    let file_size = file.metadata()?.len();
    println!("Downloaded: {} ({} bytes)", path.display(), file_size);
    Ok(())
}

fn extract_zip(zip_path: &Path, extract_to: &str, file_pattern: Option<&str>) -> Result<()> {
    let file = fs::File::open(zip_path)
        .with_context(|| format!("Failed to open zip file: {}", zip_path.display()))?;
    
    let mut archive = ZipArchive::new(file)
        .with_context(|| "Failed to read zip archive")?;
    
    fs::create_dir_all(extract_to)
        .with_context(|| format!("Failed to create extraction directory: {}", extract_to))?;
    
    let mut extracted_count = 0;
    
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .with_context(|| format!("Failed to access zip entry {}", i))?;
        
        // Check if file matches the glob pattern (if specified)
        if let Some(pattern) = file_pattern {
            let filename = Path::new(file.name()).file_name()
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
                    fs::create_dir_all(p)
                        .with_context(|| format!("Failed to create parent directory: {}", p.display()))?;
                }
            }
            
            let mut outfile = fs::File::create(&outpath)
                .with_context(|| format!("Failed to create extracted file: {}", outpath.display()))?;
            
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
        println!("Extracted {} files matching pattern '{}'", extracted_count, pattern);
    } else {
        println!("Extracted {} files", extracted_count);
    }
    Ok(())
}

fn extract_tar_gz(tar_path: &Path, extract_to: &str, file_pattern: Option<&str>) -> Result<()> {
    let file = fs::File::open(tar_path)
        .with_context(|| format!("Failed to open tar.gz file: {}", tar_path.display()))?;
    
    let tar = GzDecoder::new(file);
    let mut archive = Archive::new(tar);
    
    fs::create_dir_all(extract_to)
        .with_context(|| format!("Failed to create extraction directory: {}", extract_to))?;
    
    let mut extracted_count = 0;
    
    for entry in archive.entries()
        .with_context(|| "Failed to read tar.gz entries")? {
        
        let mut entry = entry
            .with_context(|| "Failed to access tar.gz entry")?;
        
        let path = entry.path()
            .with_context(|| "Failed to get entry path")?;
        
        let path_str = path.to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in path"))?;
        
        // Check if file matches the glob pattern (if specified)
        if let Some(pattern) = file_pattern {
            let filename = path.file_name()
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
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create parent directory: {}", parent.display()))?;
            }
        }
        
        // Extract the entry
        entry.unpack(&outpath)
            .with_context(|| format!("Failed to extract file: {}", outpath.display()))?;
        
        extracted_count += 1;
    }
    
    if let Some(pattern) = file_pattern {
        println!("Extracted {} files matching pattern '{}'", extracted_count, pattern);
    } else {
        println!("Extracted {} files", extracted_count);
    }
    Ok(())
}

fn extract_archive(archive_path: &Path, extract_to: &str, file_pattern: Option<&str>) -> Result<()> {
    let filename = archive_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    if filename.ends_with(".zip") {
        extract_zip(archive_path, extract_to, file_pattern)
    } else if filename.ends_with(".tar.gz") || filename.ends_with(".tgz") {
        extract_tar_gz(archive_path, extract_to, file_pattern)
    } else {
        // Try to detect by content or fall back to zip
        println!("Warning: Unknown archive type for '{}', attempting ZIP extraction", archive_path.display());
        extract_zip(archive_path, extract_to, file_pattern)
    }
}

fn get_filename_from_url(url: &str) -> String {
    url.split('/')
        .last()
        .unwrap_or("download")
        .to_string()
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

fn fetch_github_release(repo: &str, binary_name: Option<&str>, save_as: Option<&str>, tag: Option<&str>, unzip_to: Option<&str>, files_pattern: Option<&str>) -> Result<()> {
    let binary_name = binary_name.unwrap_or_else(|| {
        let guessed = guess_binary_name();
        println!("No binary specified for {}, guessing: {}", repo, guessed);
        Box::leak(guessed.into_boxed_str())
    });

    let api_url = if let Some(tag) = tag {
        format!("https://api.github.com/repos/{}/releases/tags/{}", repo, tag)
    } else {
        format!("https://api.github.com/repos/{}/releases/latest", repo)
    };
    
    println!("Fetching release info from: {}", api_url);
    
    let response = ureq::get(&api_url)
        .set("User-Agent", "zipget-rs")
        .call()
        .with_context(|| format!("Failed to fetch release info for {}", repo))?;
    
    if response.status() != 200 {
        return Err(anyhow::anyhow!("GitHub API request failed with status: {}", response.status()));
    }
    
    let release: GitHubRelease = response.into_json()
        .with_context(|| "Failed to parse GitHub release JSON")?;
    
    println!("Found release: {} ({})", release.name, release.tag_name);
    
    // Find the matching asset
    let asset = release.assets.iter()
        .find(|asset| asset.name.contains(binary_name))
        .ok_or_else(|| anyhow::anyhow!("Binary '{}' not found in release assets", binary_name))?;
    
    println!("Found asset: {} ({} bytes)", asset.name, asset.size);
    
    // Use caching mechanism (same as process_fetch_item)
    let cache_dir = get_cache_dir()?;
    let download_url = &asset.browser_download_url;
    let filename = get_filename_from_url(download_url);
    
    let url_hash = format!("{:x}", md5::compute(download_url));
    let cached_filename = format!("{}_{}", url_hash, filename);
    let cached_file_path = cache_dir.join(&cached_filename);
    
    let file_path = if cached_file_path.exists() {
        println!("Found cached file: {}", cached_file_path.display());
        cached_file_path
    } else {
        // Download the file
        println!("Downloading: {}", download_url);
        download_file(download_url, &cached_file_path)?;
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
            .with_context(|| format!("Failed to save file as: {}", save_as))?;
        println!("Saved as: {}", save_as);
    } else {
        // If no save_as specified, copy to current directory with original filename
        let output_path = Path::new(".").join(&filename);
        fs::copy(&file_path, &output_path)
            .with_context(|| format!("Failed to copy file to: {}", output_path.display()))?;
        println!("Saved as: {}", output_path.display());
    }
    
    // Extract if unzip_to is specified
    if let Some(unzip_to) = unzip_to {
        println!("Extracting to: {}", unzip_to);
        extract_archive(&file_path, unzip_to, files_pattern)?;
    }
    
    Ok(())
}

fn get_github_release_url(repo: &str, binary_name: &str, tag: Option<&str>) -> Result<String> {
    let api_url = if let Some(tag) = tag {
        format!("https://api.github.com/repos/{}/releases/tags/{}", repo, tag)
    } else {
        format!("https://api.github.com/repos/{}/releases/latest", repo)
    };
    
    let response = ureq::get(&api_url)
        .set("User-Agent", "zipget-rs")
        .call()
        .with_context(|| format!("Failed to fetch release info for {}", repo))?;
    
    if response.status() != 200 {
        return Err(anyhow::anyhow!("GitHub API request failed with status: {}", response.status()));
    }
    
    let release: GitHubRelease = response.into_json()
        .with_context(|| "Failed to parse GitHub release JSON")?;
    
    // Find the matching asset
    let asset = release.assets.iter()
        .find(|asset| asset.name.contains(binary_name))
        .ok_or_else(|| anyhow::anyhow!("Binary '{}' not found in release assets", binary_name))?;
    
    Ok(asset.browser_download_url.clone())
}

fn get_latest_github_tag(repo: &str) -> Result<String> {
    let api_url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    
    let response = ureq::get(&api_url)
        .set("User-Agent", "zipget-rs")
        .call()
        .with_context(|| format!("Failed to fetch latest release for {}", repo))?;
    
    if response.status() != 200 {
        return Err(anyhow::anyhow!("GitHub API request failed with status: {}", response.status()));
    }
    
    let release: GitHubRelease = response.into_json()
        .with_context(|| "Failed to parse GitHub release JSON")?;
    
    Ok(release.tag_name)
}

fn upgrade_recipe(file_path: &str) -> Result<()> {
    println!("Upgrading recipe: {}", file_path);
    
    let recipe_content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read recipe file: {}", file_path))?;
    
    let mut recipe: Recipe = serde_json::from_str(&recipe_content)
        .with_context(|| "Failed to parse recipe JSON")?;
    
    let mut updated = false;
    
    for fetch_item in &mut recipe.fetch {
        if let Some(github) = &mut fetch_item.github {
            // Ensure binary field has a value for consistent processing
            if github.binary.is_none() {
                let guessed = guess_binary_name();
                println!("No binary specified for {}, setting to: {}", github.repo, guessed);
                github.binary = Some(guessed);
                updated = true;
            }
            
            println!("Checking latest version for {}", github.repo);
            
            match get_latest_github_tag(&github.repo) {
                Ok(latest_tag) => {
                    let current_tag = github.tag.as_deref().unwrap_or("latest");
                    if github.tag.is_none() || github.tag.as_ref().unwrap() != &latest_tag {
                        println!("  {} -> {}", current_tag, latest_tag);
                        github.tag = Some(latest_tag);
                        updated = true;
                    } else {
                        println!("  {} (already latest)", current_tag);
                    }
                }
                Err(e) => {
                    println!("  Failed to get latest version: {}", e);
                }
            }
        }
    }
    
    if updated {
        let updated_content = serde_json::to_string_pretty(&recipe)
            .with_context(|| "Failed to serialize updated recipe")?;
        
        fs::write(file_path, updated_content)
            .with_context(|| format!("Failed to write updated recipe to {}", file_path))?;
        
        println!("Recipe updated successfully!");
    } else {
        println!("All GitHub releases are already at their latest versions.");
    }
    
    Ok(())
}
