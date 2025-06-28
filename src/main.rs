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
    url: String,
    #[serde(rename = "unzipTo")]
    unzip_to: Option<String>,
    #[serde(rename = "saveAs")]
    save_as: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    match args.command {
        Commands::Recipe { file } => {
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
    
    Ok(())
}

async fn process_fetch_item(fetch_item: &FetchItem, archive_dirs: &[String]) -> Result<()> {
    println!("Processing URL: {}", fetch_item.url);
    
    let url_hash = format!("{:x}", md5::compute(&fetch_item.url));
    let filename = get_filename_from_url(&fetch_item.url);
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
        println!("Downloading: {}", fetch_item.url);
        let archive_dir = &archive_dirs[0]; // Use first archive directory
        let download_path = Path::new(archive_dir).join(&cached_filename);
        download_file(&fetch_item.url, &download_path).await?;
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
