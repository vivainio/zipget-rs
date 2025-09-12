use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use crate::models::{FetchItem, LockResult, Recipe, LockInfo};
use crate::download::http;
use crate::archive::{zip, tar};
use crate::cache::get_cache_dir;
use crate::crypto::{compute_sha256_from_bytes, compute_sha256, verify_sha256};
use crate::utils::get_filename_from_url;

/// Process a recipe file with the given parameters
pub fn process_recipe(
    file_path: &str,
    tag: Option<&str>,
    upgrade: bool,
    profile: Option<&str>,
    lock: bool,
) -> Result<()> {
    if upgrade {
        return upgrade_recipe(file_path);
    }

    let recipe_content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read recipe file: {file_path}"))?;

    let recipe: Recipe = toml::from_str(&recipe_content)
        .with_context(|| "Failed to parse recipe TOML")?;

    if lock {
        // Lock mode - process sequentially and update file
        process_recipe_for_lock(file_path, &recipe, tag, profile)
    } else {
        // Normal mode - process items
        process_recipe_items(&recipe, tag, profile)
    }
}

/// Process recipe items in normal mode
fn process_recipe_items(
    recipe: &Recipe,
    tag: Option<&str>,
    profile: Option<&str>,
) -> Result<()> {
    // Filter items by tag if specified
    let items_to_process: Vec<(&String, &FetchItem)> = if let Some(filter_tag) = tag {
        recipe
            .iter()
            .filter(|(k, _)| k.contains(filter_tag))
            .collect()
    } else {
        recipe.iter().collect()
    };

    if items_to_process.is_empty() {
        if let Some(filter_tag) = tag {
            println!("No items found with tag: {filter_tag}");
        } else {
            println!("No items to process in recipe");
        }
        return Ok(());
    }

    if let Some(filter_tag) = tag {
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

    // Process each fetch item (sequentially for now, could be made concurrent later)
    let mut errors = Vec::new();
    
    for (section_name, fetch_item) in items_to_process {
        println!("Processing {section_name}...");
        if let Err(e) = process_fetch_item(fetch_item, profile) {
            println!("Error processing {section_name}: {e}");
            errors.push(format!("{section_name}: {e}"));
        }
    }

    if !errors.is_empty() {
        return Err(anyhow::anyhow!(
            "Some downloads failed:\n{}",
            errors.join("\n")
        ));
    }

    Ok(())
}

/// Process recipe for lock file generation
fn process_recipe_for_lock(
    file_path: &str,
    recipe: &Recipe,
    tag: Option<&str>,
    profile: Option<&str>,
) -> Result<()> {
    let mut updated_recipe = recipe.clone();
    let mut any_updated = false;

    // Collect section names and items to process
    let sections_to_process: Vec<(String, FetchItem)> = recipe
        .iter()
        .filter(|(section_name, _)| {
            if let Some(filter_tag) = tag {
                section_name.contains(filter_tag)
            } else {
                true
            }
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    if sections_to_process.is_empty() {
        if let Some(filter_tag) = tag {
            println!("No items found with tag: {filter_tag}");
        } else {
            println!("No items to process in recipe");
        }
        return Ok(());
    }

    println!(
        "Processing {} items for lock file...",
        sections_to_process.len()
    );

    for (section_name, fetch_item) in sections_to_process {
        println!("Processing {section_name} for lock file...");
        match process_fetch_item_for_lock(&fetch_item, profile) {
            Ok(lock_result) => {
                // Update the recipe with computed SHA and resolved tag
                if let Some(item) = updated_recipe.get_mut(&section_name) {
                    // Initialize lock info if not present
                    if item.lock.is_none() {
                        item.lock = Some(LockInfo {
                            sha: None,
                            download_url: None,
                        });
                    }

                    let lock_info = item.lock.as_mut().unwrap();
                    let old_sha = lock_info.sha.clone().unwrap_or_else(|| "none".to_string());
                    println!("  SHA-256: {old_sha} -> {}", lock_result.sha);

                    if old_sha != lock_result.sha {
                        lock_info.sha = Some(lock_result.sha);
                        any_updated = true;
                    }

                    // If we resolved a tag for a GitHub release without explicit tag, pin it
                    if let Some(resolved_tag) = lock_result.resolved_tag {
                        if let Some(github) = &mut item.github {
                            if github.tag.is_none() {
                                println!("  Pinning GitHub release to tag: {resolved_tag}");
                                github.tag = Some(resolved_tag);
                                any_updated = true;
                            }
                        }
                    }

                    // Store direct download URL for GitHub assets
                    if let Some(download_url) = lock_result.download_url {
                        if lock_info.download_url.as_ref() != Some(&download_url) {
                            println!("  Storing direct download URL");
                            lock_info.download_url = Some(download_url);
                            any_updated = true;
                        }
                    }
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to process {}: {}", section_name, e));
            }
        }
    }

    if any_updated {
        // Write updated recipe back to file
        let updated_content = toml::to_string_pretty(&updated_recipe)
            .with_context(|| "Failed to serialize updated recipe with SHA hashes")?;

        fs::write(file_path, updated_content)
            .with_context(|| format!("Failed to write lock file to {file_path}"))?;

        println!("Lock file updated with SHA-256 hashes!");
    } else {
        println!("All SHA-256 hashes were already up to date.");
    }

    Ok(())
}

/// Process a single fetch item from a recipe
pub fn process_fetch_item(
    fetch_item: &FetchItem,
    global_profile: Option<&str>,
) -> Result<Option<String>> {
    let cache_dir = get_cache_dir()?;

    let (download_url, filename) = if let Some(stored_url) = fetch_item
        .lock
        .as_ref()
        .and_then(|l| l.download_url.as_ref())
    {
        // Use stored direct download URL (from lock file) - skip GitHub API
        println!("Using stored download URL: {stored_url}");
        let filename = get_filename_from_url(stored_url);
        (stored_url.clone(), filename)
    } else if let Some(url) = &fetch_item.url {
        println!("Processing URL: {url}");
        let filename = get_filename_from_url(url);
        (url.clone(), filename)
    } else if let Some(_github) = &fetch_item.github {
        // TODO: Implement GitHub processing
        return Err(anyhow::anyhow!(
            "GitHub processing not yet implemented in refactored version"
        ));
    } else {
        return Err(anyhow::anyhow!(
            "FetchItem must have either 'url' or 'github' specified"
        ));
    };

    let url_hash = compute_sha256_from_bytes(download_url.as_bytes());
    let cached_filename = format!("{url_hash}_{filename}");
    let cached_file_path = cache_dir.join(&cached_filename);

    // Use the appropriate profile - item-specific profile overrides global profile
    let profile = fetch_item.profile.as_deref().or(global_profile);

    let file_path = if cached_file_path.exists() {
        println!("Found cached file: {}", cached_file_path.display());
        cached_file_path
    } else {
        println!("Downloading: {download_url}");
        http::download_file(&download_url, &cached_file_path, profile)?;
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

    // Verify SHA if specified
    if let Some(expected_sha) = fetch_item.lock.as_ref().and_then(|l| l.sha.as_ref()) {
        println!("Verifying SHA-256...");
        if !verify_sha256(&file_path, expected_sha)? {
            return Err(anyhow::anyhow!(
                "SHA-256 verification failed for downloaded file"
            ));
        }
    }

    // Compute SHA for lock file generation (always compute, return for potential use)
    let computed_sha = compute_sha256(&file_path)?;

    // Extract the archive if unzip_to is specified
    if let Some(unzip_to) = &fetch_item.unzip_to {
        println!("Extracting to: {unzip_to}");
        extract_archive(&file_path, unzip_to, fetch_item.files.as_deref())?;
    }

    Ok(Some(computed_sha))
}

/// Extract archive based on file extension
fn extract_archive(file_path: &Path, extract_to: &str, file_pattern: Option<&str>) -> Result<()> {
    if file_path.extension().and_then(|s| s.to_str()) == Some("zip") {
        zip::extract_zip(file_path, extract_to, file_pattern)?;
    } else if file_path.file_name().and_then(|s| s.to_str()).unwrap_or("").ends_with(".tar.gz") 
           || file_path.extension().and_then(|s| s.to_str()) == Some("tgz") {
        tar::extract_tar_gz(file_path, extract_to, file_pattern)?;
    } else {
        println!("Warning: Unknown archive format, skipping extraction");
    }
    Ok(())
}

/// Process a fetch item for lock file generation
pub fn process_fetch_item_for_lock(
    fetch_item: &FetchItem,
    global_profile: Option<&str>,
) -> Result<LockResult> {
    // For now, use the regular processing but only return the hash
    if let Some(sha) = process_fetch_item(fetch_item, global_profile)? {
        Ok(LockResult {
            sha,
            resolved_tag: None, // TODO: Implement for GitHub releases
            download_url: fetch_item.url.clone(), // Direct URL for now
        })
    } else {
        Err(anyhow::anyhow!("Failed to compute SHA for item"))
    }
}

/// Upgrade recipe to latest GitHub releases
pub fn upgrade_recipe(file_path: &str) -> Result<()> {
    // TODO: Implement recipe upgrade logic from main.rs
    println!("Recipe upgrade not yet implemented in refactored version");
    println!("File path: {}", file_path);
    Ok(())
}
