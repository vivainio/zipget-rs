use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::Path;
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
        /// Upgrade all GitHub releases to latest versions
        #[arg(long)]
        upgrade: bool,
    },
    /// Fetch the latest release binary from a GitHub repository
    Github {
        /// GitHub repository in format "owner/repo"
        repo: String,
        /// Name of the binary to download from release assets
        binary: String,
        /// Optional directory to save the binary (defaults to current directory)
        #[arg(short, long)]
        output: Option<String>,
        /// Optional tag to download specific release (defaults to latest)
        #[arg(short, long)]
        tag: Option<String>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
struct Recipe {
    config: Config,
    fetch: Vec<FetchItem>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Config {
    archive: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct FetchItem {
    url: Option<String>,
    github: Option<GitHubFetch>,
    #[serde(rename = "unzipTo")]
    unzip_to: Option<String>,
    #[serde(rename = "saveAs")]
    save_as: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct GitHubFetch {
    repo: String,
    binary: String,
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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    match args.command {
        Commands::Recipe { file, upgrade } => {
            if upgrade {
                upgrade_recipe(&file).await?;
            } else {
                let recipe_content = fs::read_to_string(&file)
                    .with_context(|| format!("Failed to read recipe file: {}", file))?;
                
                let recipe: Recipe = serde_json::from_str(&recipe_content)
                    .with_context(|| "Failed to parse recipe JSON")?;
                
                // Create archive directories
                for archive_dir in &recipe.config.archive {
                    fs::create_dir_all(archive_dir)
                        .with_context(|| format!("Failed to create archive directory: {}", archive_dir))?;
                }
                
                // Process each fetch item
                for fetch_item in &recipe.fetch {
                    process_fetch_item(fetch_item, &recipe.config.archive).await?;
                }
                
                println!("All downloads and extractions completed successfully!");
            }
        }
        Commands::Github { repo, binary, output, tag } => {
            if let Some(tag_name) = &tag {
                println!("Fetching release {} for {}", tag_name, repo);
            } else {
                println!("Fetching latest release for {}", repo);
            }
            fetch_github_release(&repo, &binary, output.as_deref(), tag.as_deref()).await?;
        }
    }
    
    Ok(())
}

async fn process_fetch_item(fetch_item: &FetchItem, archive_dirs: &[String]) -> Result<()> {
    let (download_url, filename) = if let Some(url) = &fetch_item.url {
        println!("Processing URL: {}", url);
        let filename = get_filename_from_url(url);
        (url.clone(), filename)
    } else if let Some(github) = &fetch_item.github {
        println!("Processing GitHub repo: {}, binary: {}", github.repo, github.binary);
        
        // Get the release download URL
        let github_url = get_github_release_url(&github.repo, &github.binary, github.tag.as_deref()).await?;
        let filename = get_filename_from_url(&github_url);
        (github_url, filename)
    } else {
        return Err(anyhow::anyhow!("FetchItem must have either 'url' or 'github' specified"));
    };
    
    let url_hash = format!("{:x}", md5::compute(&download_url));
    let cached_filename = format!("{}_{}", url_hash, filename);
    
    let mut cached_file_path = None;
    
    // Check if file exists in any archive directory
    for archive_dir in archive_dirs {
        let potential_path = Path::new(archive_dir).join(&cached_filename);
        if potential_path.exists() {
            println!("Found cached file: {}", potential_path.display());
            cached_file_path = Some(potential_path);
            break;
        }
    }
    
    let file_path = if let Some(cached_path) = cached_file_path {
        cached_path
    } else {
        // Download the file
        println!("Downloading: {}", download_url);
        let archive_dir = &archive_dirs[0]; // Use first archive directory
        let download_path = Path::new(archive_dir).join(&cached_filename);
        download_file(&download_url, &download_path).await?;
        download_path
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
        extract_zip(&file_path, unzip_to)?;
    }
    
    Ok(())
}

async fn download_file(url: &str, path: &Path) -> Result<()> {
    let response = reqwest::get(url).await
        .with_context(|| format!("Failed to download: {}", url))?;
    
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Download failed with status: {}", response.status()));
    }
    
    let bytes = response.bytes().await
        .with_context(|| "Failed to read response bytes")?;
    
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }
    
    let mut file = fs::File::create(path)
        .with_context(|| format!("Failed to create file: {}", path.display()))?;
    
    file.write_all(&bytes)
        .with_context(|| format!("Failed to write to file: {}", path.display()))?;
    
    println!("Downloaded: {} ({} bytes)", path.display(), bytes.len());
    Ok(())
}

fn extract_zip(zip_path: &Path, extract_to: &str) -> Result<()> {
    let file = fs::File::open(zip_path)
        .with_context(|| format!("Failed to open zip file: {}", zip_path.display()))?;
    
    let mut archive = ZipArchive::new(file)
        .with_context(|| "Failed to read zip archive")?;
    
    fs::create_dir_all(extract_to)
        .with_context(|| format!("Failed to create extraction directory: {}", extract_to))?;
    
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .with_context(|| format!("Failed to access zip entry {}", i))?;
        
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
    }
    
    println!("Extracted {} files", archive.len());
    Ok(())
}

fn get_filename_from_url(url: &str) -> String {
    url.split('/')
        .last()
        .unwrap_or("download")
        .to_string()
}

async fn fetch_github_release(repo: &str, binary_name: &str, output_dir: Option<&str>, tag: Option<&str>) -> Result<()> {
    let api_url = if let Some(tag) = tag {
        format!("https://api.github.com/repos/{}/releases/tags/{}", repo, tag)
    } else {
        format!("https://api.github.com/repos/{}/releases/latest", repo)
    };
    
    println!("Fetching release info from: {}", api_url);
    
    let client = reqwest::Client::new();
    let response = client
        .get(&api_url)
        .header("User-Agent", "zipget-rs")
        .send()
        .await
        .with_context(|| format!("Failed to fetch release info for {}", repo))?;
    
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("GitHub API request failed with status: {}", response.status()));
    }
    
    let release: GitHubRelease = response.json().await
        .with_context(|| "Failed to parse GitHub release JSON")?;
    
    println!("Found release: {} ({})", release.name, release.tag_name);
    
    // Find the matching asset
    let asset = release.assets.iter()
        .find(|asset| asset.name.contains(binary_name))
        .ok_or_else(|| anyhow::anyhow!("Binary '{}' not found in release assets", binary_name))?;
    
    println!("Found asset: {} ({} bytes)", asset.name, asset.size);
    
    // Determine output path
    let output_path = if let Some(dir) = output_dir {
        fs::create_dir_all(dir)
            .with_context(|| format!("Failed to create output directory: {}", dir))?;
        Path::new(dir).join(&asset.name)
    } else {
        Path::new(&asset.name).to_path_buf()
    };
    
    // Download the asset
    println!("Downloading: {}", asset.browser_download_url);
    download_file(&asset.browser_download_url, &output_path).await?;
    
    println!("Successfully downloaded: {}", output_path.display());
    Ok(())
}

async fn get_github_release_url(repo: &str, binary_name: &str, tag: Option<&str>) -> Result<String> {
    let api_url = if let Some(tag) = tag {
        format!("https://api.github.com/repos/{}/releases/tags/{}", repo, tag)
    } else {
        format!("https://api.github.com/repos/{}/releases/latest", repo)
    };
    
    let client = reqwest::Client::new();
    let response = client
        .get(&api_url)
        .header("User-Agent", "zipget-rs")
        .send()
        .await
        .with_context(|| format!("Failed to fetch release info for {}", repo))?;
    
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("GitHub API request failed with status: {}", response.status()));
    }
    
    let release: GitHubRelease = response.json().await
        .with_context(|| "Failed to parse GitHub release JSON")?;
    
    // Find the matching asset
    let asset = release.assets.iter()
        .find(|asset| asset.name.contains(binary_name))
        .ok_or_else(|| anyhow::anyhow!("Binary '{}' not found in release assets", binary_name))?;
    
    Ok(asset.browser_download_url.clone())
}

async fn get_latest_github_tag(repo: &str) -> Result<String> {
    let api_url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    
    let client = reqwest::Client::new();
    let response = client
        .get(&api_url)
        .header("User-Agent", "zipget-rs")
        .send()
        .await
        .with_context(|| format!("Failed to fetch latest release for {}", repo))?;
    
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("GitHub API request failed with status: {}", response.status()));
    }
    
    let release: GitHubRelease = response.json().await
        .with_context(|| "Failed to parse GitHub release JSON")?;
    
    Ok(release.tag_name)
}

async fn upgrade_recipe(file_path: &str) -> Result<()> {
    println!("Upgrading recipe: {}", file_path);
    
    let recipe_content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read recipe file: {}", file_path))?;
    
    let mut recipe: Recipe = serde_json::from_str(&recipe_content)
        .with_context(|| "Failed to parse recipe JSON")?;
    
    let mut updated = false;
    
    for fetch_item in &mut recipe.fetch {
        if let Some(github) = &mut fetch_item.github {
            println!("Checking latest version for {}", github.repo);
            
            match get_latest_github_tag(&github.repo).await {
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
