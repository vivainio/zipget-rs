use crate::archive::{tar, zip};
use crate::cache::get_cache_dir;
use crate::crypto::{compute_sha256, compute_sha256_from_bytes, verify_sha256};
use crate::download::http;
use crate::models::{
    FetchItem, GitHubAsset, GitHubFetch, GitHubRelease, LockInfo, LockResult, Recipe,
};
use crate::utils::get_filename_from_url;
use crate::vars::VarContext;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Process a recipe file with the given parameters
pub fn process_recipe(
    file_path: &str,
    tag: Option<&str>,
    upgrade: bool,
    profile: Option<&str>,
    lock: bool,
    var_overrides: &[String],
    dry: bool,
) -> Result<()> {
    if upgrade {
        return upgrade_recipe(file_path);
    }

    let recipe_content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read recipe file: {file_path}"))?;

    let recipe: Recipe =
        toml::from_str(&recipe_content).with_context(|| "Failed to parse recipe TOML")?;

    // Create variable context with recipe vars and CLI overrides
    let recipe_path = Path::new(file_path);
    let var_ctx = VarContext::new(&recipe.vars, var_overrides, Some(recipe_path))
        .with_context(|| "Failed to create variable context")?;

    // Show active variables if any custom vars are defined
    if !recipe.vars.is_empty() || !var_overrides.is_empty() {
        println!("Active variables:");
        for (key, value) in var_ctx.vars() {
            // Only show non-builtin vars or overridden ones
            if recipe.vars.contains_key(key)
                || var_overrides
                    .iter()
                    .any(|o| o.starts_with(&format!("{key}=")))
            {
                println!("  {key} = {value}");
            }
        }
    }

    if dry {
        // Dry run mode - show expanded values without downloading
        return dry_run_recipe(&recipe, tag, &var_ctx);
    }

    if lock {
        // Lock mode - process sequentially and update file
        process_recipe_for_lock(file_path, &recipe, tag, profile, &var_ctx)
    } else {
        // Normal mode - process items
        process_recipe_items(&recipe, tag, profile, &var_ctx)
    }
}

/// Dry run mode - show how variables would be expanded
fn dry_run_recipe(recipe: &Recipe, tag: Option<&str>, var_ctx: &VarContext) -> Result<()> {
    // Filter items by tag if specified
    let items_to_process: Vec<(&String, &FetchItem)> = if let Some(filter_tag) = tag {
        recipe
            .items
            .iter()
            .filter(|(k, _)| k.contains(filter_tag))
            .collect()
    } else {
        recipe.items.iter().collect()
    };

    if items_to_process.is_empty() {
        if let Some(filter_tag) = tag {
            println!("No items found with tag: {filter_tag}");
        } else {
            println!("No items to process in recipe");
        }
        return Ok(());
    }

    println!("\nDry run - showing expanded values:\n");

    for (section_name, fetch_item) in items_to_process {
        println!("[{section_name}]");

        // Apply variable substitution and show results
        let substituted = substitute_fetch_item(fetch_item, var_ctx)
            .with_context(|| format!("Failed to substitute variables in {section_name}"))?;

        if let Some(url) = &substituted.url {
            println!("  url = \"{url}\"");
        }
        if let Some(github) = &substituted.github {
            print!("  github = {{ repo = \"{}\"", github.repo);
            if let Some(asset) = &github.asset {
                print!(", asset = \"{asset}\"");
            }
            if let Some(tag) = &github.tag {
                print!(", tag = \"{tag}\"");
            }
            println!(" }}");
        }
        if let Some(save_as) = &substituted.save_as {
            println!("  save_as = \"{save_as}\"");
        }
        if let Some(unzip_to) = &substituted.unzip_to {
            println!("  unzip_to = \"{unzip_to}\"");
        }
        if let Some(files) = &substituted.files {
            println!("  files = \"{files}\"");
        }
        if substituted.executable == Some(true) {
            println!("  executable = true");
        }
        println!();
    }

    Ok(())
}

/// Process recipe items in normal mode
fn process_recipe_items(
    recipe: &Recipe,
    tag: Option<&str>,
    profile: Option<&str>,
    var_ctx: &VarContext,
) -> Result<()> {
    // Filter items by tag if specified
    let items_to_process: Vec<(&String, &FetchItem)> = if let Some(filter_tag) = tag {
        recipe
            .items
            .iter()
            .filter(|(k, _)| k.contains(filter_tag))
            .collect()
    } else {
        recipe.items.iter().collect()
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
        // Apply variable substitution to the fetch item
        let substituted_item = substitute_fetch_item(fetch_item, var_ctx)
            .with_context(|| format!("Failed to substitute variables in {section_name}"))?;
        if let Err(e) = process_fetch_item(&substituted_item, profile) {
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

/// Apply variable substitution to a FetchItem
fn substitute_fetch_item(item: &FetchItem, var_ctx: &VarContext) -> Result<FetchItem> {
    Ok(FetchItem {
        url: item
            .url
            .as_ref()
            .map(|s| var_ctx.substitute(s))
            .transpose()?,
        github: item
            .github
            .as_ref()
            .map(|g| substitute_github_fetch(g, var_ctx))
            .transpose()?,
        unzip_to: item
            .unzip_to
            .as_ref()
            .map(|s| var_ctx.substitute(s))
            .transpose()?,
        save_as: item
            .save_as
            .as_ref()
            .map(|s| var_ctx.substitute(s))
            .transpose()?,
        files: item
            .files
            .as_ref()
            .map(|s| var_ctx.substitute(s))
            .transpose()?,
        profile: item.profile.clone(),
        install_exes: item.install_exes.clone(),
        no_shim: item.no_shim,
        lock: item.lock.clone(),
        executable: item.executable,
    })
}

/// Apply variable substitution to GitHubFetch
fn substitute_github_fetch(github: &GitHubFetch, var_ctx: &VarContext) -> Result<GitHubFetch> {
    Ok(GitHubFetch {
        repo: var_ctx.substitute(&github.repo)?,
        asset: github
            .asset
            .as_ref()
            .map(|s| var_ctx.substitute(s))
            .transpose()?,
        tag: github
            .tag
            .as_ref()
            .map(|s| var_ctx.substitute(s))
            .transpose()?,
    })
}

/// Process recipe for lock file generation
fn process_recipe_for_lock(
    file_path: &str,
    recipe: &Recipe,
    tag: Option<&str>,
    profile: Option<&str>,
    var_ctx: &VarContext,
) -> Result<()> {
    let mut updated_recipe = recipe.clone();
    let mut any_updated = false;

    // Collect section names and items to process
    let sections_to_process: Vec<(String, FetchItem)> = recipe
        .items
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
        // Apply variable substitution before processing
        let substituted_item = substitute_fetch_item(&fetch_item, var_ctx)
            .with_context(|| format!("Failed to substitute variables in {section_name}"))?;
        match process_fetch_item_for_lock(&substituted_item, profile) {
            Ok(lock_result) => {
                // Update the recipe with computed SHA and resolved tag
                if let Some(item) = updated_recipe.items.get_mut(&section_name) {
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
                    if let Some(resolved_tag) = lock_result.resolved_tag
                        && let Some(github) = &mut item.github
                        && github.tag.is_none()
                    {
                        println!("  Pinning GitHub release to tag: {resolved_tag}");
                        github.tag = Some(resolved_tag);
                        any_updated = true;
                    }

                    // Store direct download URL for GitHub assets
                    if let Some(download_url) = lock_result.download_url
                        && lock_info.download_url.as_ref() != Some(&download_url)
                    {
                        println!("  Storing direct download URL");
                        lock_info.download_url = Some(download_url);
                        any_updated = true;
                    }
                }
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to process {}: {}", section_name, e));
            }
        }
    }

    if any_updated {
        // Write updated recipe back to file with inline lock tables
        let updated_content = serialize_recipe_with_inline_locks(&updated_recipe)
            .with_context(|| "Failed to serialize updated recipe with SHA hashes")?;

        fs::write(file_path, updated_content)
            .with_context(|| format!("Failed to write lock file to {file_path}"))?;

        println!("Lock file updated with SHA-256 hashes!");
    } else {
        println!("All SHA-256 hashes were already up to date.");
    }

    Ok(())
}

/// Serialize recipe with inline lock tables to match expected test format
fn serialize_recipe_with_inline_locks(recipe: &Recipe) -> Result<String> {
    let mut output = String::new();

    // Serialize vars section first if present
    if !recipe.vars.is_empty() {
        output.push_str("[vars]\n");
        for (key, value) in &recipe.vars {
            output.push_str(&format!("{key} = \"{value}\"\n"));
        }
        output.push('\n');
    }

    for (section_name, fetch_item) in &recipe.items {
        output.push_str(&format!("[{section_name}]\n"));

        // Serialize basic fields
        if let Some(url) = &fetch_item.url {
            output.push_str(&format!("url = \"{url}\"\n"));
        }

        if let Some(save_as) = &fetch_item.save_as {
            output.push_str(&format!("save_as = \"{save_as}\"\n"));
        }

        if let Some(unzip_to) = &fetch_item.unzip_to {
            output.push_str(&format!("unzip_to = \"{unzip_to}\"\n"));
        }

        if let Some(files) = &fetch_item.files {
            output.push_str(&format!("files = \"{files}\"\n"));
        }

        if let Some(profile) = &fetch_item.profile {
            output.push_str(&format!("profile = \"{profile}\"\n"));
        }

        if fetch_item.executable == Some(true) {
            output.push_str("executable = true\n");
        }

        // Handle GitHub configuration
        if let Some(github) = &fetch_item.github {
            let mut github_parts = vec![format!("repo = \"{}\"", github.repo)];
            if let Some(asset) = &github.asset {
                github_parts.push(format!("asset = \"{asset}\""));
            }
            if let Some(tag) = &github.tag {
                github_parts.push(format!("tag = \"{tag}\""));
            }
            output.push_str(&format!("github = {{ {} }}\n", github_parts.join(", ")));
        }

        // Handle lock information as inline table
        if let Some(lock) = &fetch_item.lock {
            let mut lock_parts = Vec::new();
            if let Some(sha) = &lock.sha {
                lock_parts.push(format!("sha = \"{sha}\""));
            }
            if let Some(download_url) = &lock.download_url {
                lock_parts.push(format!("download_url = \"{download_url}\""));
            }
            if !lock_parts.is_empty() {
                output.push_str(&format!("lock = {{ {} }}\n", lock_parts.join(", ")));
            }
        }

        output.push('\n');
    }

    Ok(output)
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

    // Verify SHA if specified in lock structure
    if let Some(expected_sha) = fetch_item.lock.as_ref().and_then(|l| l.sha.as_ref()) {
        println!("Verifying SHA-256...");
        if verify_sha256(&file_path, expected_sha)? {
            println!("SHA-256 verification passed");
        } else {
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
        let extracted_files = extract_archive(&file_path, unzip_to, fetch_item.files.as_deref())?;

        // Set executable permission if requested (Unix only)
        #[cfg(unix)]
        if fetch_item.executable == Some(true) {
            set_executable_on_files(&extracted_files)?;
        }
    }

    Ok(Some(computed_sha))
}

use std::path::PathBuf;

/// Extract archive based on file extension, returns list of extracted files
fn extract_archive(
    file_path: &Path,
    extract_to: &str,
    file_pattern: Option<&str>,
) -> Result<Vec<PathBuf>> {
    if file_path.extension().and_then(|s| s.to_str()) == Some("zip") {
        zip::extract_zip(file_path, extract_to, file_pattern)
    } else if file_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .ends_with(".tar.gz")
        || file_path.extension().and_then(|s| s.to_str()) == Some("tgz")
    {
        tar::extract_tar_gz(file_path, extract_to, file_pattern)
    } else if file_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .ends_with(".tar.zst")
    {
        tar::extract_tar_zst(file_path, extract_to, file_pattern)
    } else {
        println!("Warning: Unknown archive format, skipping extraction");
        Ok(Vec::new())
    }
}

/// Set executable permission on specific files (Unix only)
#[cfg(unix)]
fn set_executable_on_files(files: &[PathBuf]) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    for path in files {
        if path.is_file() {
            let metadata = fs::metadata(path)?;
            let mut perms = metadata.permissions();
            let mode = perms.mode();
            // Add execute bits where read bits are set
            let new_mode = mode | ((mode & 0o444) >> 2);
            perms.set_mode(new_mode);
            fs::set_permissions(path, perms)?;
        }
    }
    Ok(())
}

/// Process a fetch item for lock file generation
pub fn process_fetch_item_for_lock(
    fetch_item: &FetchItem,
    global_profile: Option<&str>,
) -> Result<LockResult> {
    let cache_dir = get_cache_dir()?;
    let profile = fetch_item.profile.as_deref().or(global_profile);

    let (download_url, filename, resolved_tag) = if let Some(url) = &fetch_item.url {
        println!("Processing URL for lock: {url}");
        let filename = get_filename_from_url(url);
        (url.clone(), filename, None)
    } else if let Some(github) = &fetch_item.github {
        println!("Processing GitHub repo for lock: {}", github.repo);

        let (download_url, filename, resolved_tag) = if let Some(asset_name) = &github.asset {
            println!("Using specified asset: {asset_name}");
            let github_url =
                get_github_release_url(&github.repo, asset_name, github.tag.as_deref())?;
            let filename = get_filename_from_url(&github_url);

            // If no tag specified, get the resolved tag for pinning
            let resolved_tag = if github.tag.is_none() {
                println!("No tag specified, fetching latest for pinning...");
                match get_latest_github_tag(&github.repo) {
                    Ok(tag) => {
                        println!("Pinning GitHub release to tag: {tag}");
                        Some(tag)
                    }
                    Err(e) => {
                        println!("Warning: Could not fetch latest tag: {e}");
                        None
                    }
                }
            } else {
                None
            };

            (github_url, filename, resolved_tag)
        } else {
            println!("No asset specified, analyzing available assets...");
            let (release, best_asset) =
                get_best_binary_from_release(&github.repo, github.tag.as_deref())?;

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
            println!(
                "Storing direct download URL: {}",
                asset.browser_download_url
            );

            let filename = get_filename_from_url(&asset.browser_download_url);

            // If no tag specified, use the resolved tag for pinning
            let resolved_tag = if github.tag.is_none() {
                println!("Pinning GitHub release to tag: {}", release.tag_name);
                Some(release.tag_name.clone())
            } else {
                None
            };

            (asset.browser_download_url.clone(), filename, resolved_tag)
        };

        (download_url, filename, resolved_tag)
    } else {
        return Err(anyhow::anyhow!(
            "FetchItem must have either 'url' or 'github' specified"
        ));
    };

    // Download and compute SHA
    let url_hash = compute_sha256_from_bytes(download_url.as_bytes());
    let cached_filename = format!("{url_hash}_{filename}");
    let cached_file_path = cache_dir.join(&cached_filename);

    let file_path = if cached_file_path.exists() {
        println!("Found cached file: {}", cached_file_path.display());
        cached_file_path
    } else {
        println!("Downloading for SHA computation: {download_url}");
        http::download_file(&download_url, &cached_file_path, profile)?;
        cached_file_path
    };

    // Compute SHA-256
    let sha = compute_sha256(&file_path)?;
    println!("SHA-256 computed: {sha}");

    Ok(LockResult {
        sha,
        resolved_tag,
        download_url: Some(download_url),
    })
}

/// Get the latest GitHub tag for a repository
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

/// Upgrade recipe to latest GitHub releases
pub fn upgrade_recipe(file_path: &str) -> Result<()> {
    // Read and parse recipe file
    let recipe_content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read recipe file: {file_path}"))?;

    let mut recipe: Recipe =
        toml::from_str(&recipe_content).with_context(|| "Failed to parse recipe TOML")?;

    let mut any_updated = false;

    // Iterate through each item and upgrade GitHub references
    for (section_name, item) in recipe.items.iter_mut() {
        if let Some(github) = &mut item.github {
            println!("Checking {}: {}", section_name, github.repo);

            // If no asset specified, guess the binary name
            if github.asset.is_none() {
                let guessed_name = guess_binary_name_from_repo(&github.repo)?;
                println!("  Guessed binary name: {guessed_name}");
                github.asset = Some(guessed_name);
                any_updated = true;
            }

            // Fetch latest tag from GitHub API
            match get_latest_github_tag(&github.repo) {
                Ok(latest_tag) => {
                    // Check if we need to update the tag
                    if let Some(current_tag) = &github.tag {
                        if current_tag != &latest_tag {
                            println!("  Updating tag: {current_tag} -> {latest_tag}");
                            github.tag = Some(latest_tag);
                            any_updated = true;
                        } else {
                            println!("  Tag is already up to date: {current_tag}");
                        }
                    } else {
                        // No tag specified, pin to latest
                        println!("  Pinning to latest tag: {latest_tag}");
                        github.tag = Some(latest_tag);
                        any_updated = true;
                    }
                }
                Err(e) => {
                    println!("  Warning: Failed to fetch latest tag: {e}");
                }
            }
        }
    }

    if any_updated {
        // Write updated recipe back to file
        let updated_content = serialize_recipe_with_inline_locks(&recipe)
            .with_context(|| "Failed to serialize updated recipe")?;

        fs::write(file_path, updated_content)
            .with_context(|| format!("Failed to write updated recipe to {file_path}"))?;

        println!("Recipe upgraded successfully!");
    } else {
        println!("All GitHub references are already up to date.");
    }

    Ok(())
}

/// Guess binary name from repository name
fn guess_binary_name_from_repo(repo: &str) -> Result<String> {
    // Extract the repo name (last part after /)
    let repo_name = repo
        .rsplit('/')
        .next()
        .ok_or_else(|| anyhow::anyhow!("Invalid repository name: {repo}"))?;

    // Return the repo name as the guessed binary name
    Ok(repo_name.to_string())
}

/// Get GitHub release URL for a specific asset
fn get_github_release_url(repo: &str, asset_name: &str, tag: Option<&str>) -> Result<String> {
    let api_url = if let Some(tag) = tag {
        format!("https://api.github.com/repos/{repo}/releases/tags/{tag}")
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
                .contains(&asset_name.to_lowercase())
        })
        .ok_or_else(|| anyhow::anyhow!("Asset '{}' not found in release assets", asset_name))?;

    Ok(asset.browser_download_url.clone())
}

/// Get the best binary from a GitHub release automatically
fn get_best_binary_from_release(repo: &str, tag: Option<&str>) -> Result<(GitHubRelease, String)> {
    let api_url = if let Some(tag) = tag {
        format!("https://api.github.com/repos/{repo}/releases/tags/{tag}")
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
        // Fallback to basic guess
        println!("No good match found, falling back to basic guess");
        guess_binary_name()
    };

    Ok((release, best_match))
}

/// Find the best matching binary from available assets
fn find_best_matching_binary(assets: &[GitHubAsset]) -> Option<String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    // Define priority patterns for different OS/arch combinations
    let patterns = match (os, arch) {
        ("windows", "x86_64") => vec![
            "windows-x86_64",
            "win64",
            "windows-amd64",
            "x86_64-pc-windows",
            "windows",
            "win",
            "x64",
            "amd64",
        ],
        ("windows", "x86") => vec![
            "windows-i686",
            "win32",
            "windows-x86",
            "i686-pc-windows",
            "windows",
            "win",
            "x86",
            "i386",
        ],
        ("linux", "x86_64") => vec![
            "linux-x86_64",
            "linux-amd64",
            "x86_64-unknown-linux",
            "linux64",
            "linux",
            "x64",
            "amd64",
        ],
        ("linux", "aarch64") => vec![
            "linux-aarch64",
            "linux-arm64",
            "aarch64-unknown-linux",
            "linux",
            "arm64",
        ],
        ("macos", "x86_64") => vec![
            "darwin-x86_64",
            "macos-x86_64",
            "x86_64-apple-darwin",
            "osx-x64",
            "darwin",
            "macos",
            "osx",
            "mac",
        ],
        ("macos", "aarch64") => vec![
            "darwin-aarch64",
            "macos-aarch64",
            "aarch64-apple-darwin",
            "darwin-arm64",
            "darwin",
            "macos",
            "osx",
            "mac",
            "arm64",
        ],
        _ => vec!["universal", "any"],
    };

    // Score each asset based on pattern matches
    let mut scored_assets: Vec<(i32, &GitHubAsset)> = assets
        .iter()
        .filter(|asset| {
            // Skip non-archive files unless they're executables
            let name_lower = asset.name.to_lowercase();
            name_lower.ends_with(".zip")
                || name_lower.ends_with(".tar.gz")
                || name_lower.ends_with(".tgz")
                || (!name_lower.contains(".") && asset.size > 1000) // Likely executable
        })
        .map(|asset| {
            let name_lower = asset.name.to_lowercase();
            let mut score = 0;

            // Give points based on pattern matches (earlier patterns get higher scores)
            for (i, pattern) in patterns.iter().enumerate() {
                if name_lower.contains(pattern) {
                    score += 100 - i as i32; // Earlier patterns get higher scores
                }
            }

            // Prefer zip files on Windows, tar.gz on others
            if (os == "windows" && name_lower.ends_with(".zip"))
                || (os != "windows"
                    && (name_lower.ends_with(".tar.gz") || name_lower.ends_with(".tgz")))
            {
                score += 10;
            }

            // Prefer smaller files (likely stripped binaries)
            if asset.size < 50_000_000 {
                // Less than 50MB
                score += 5;
            }

            (score, asset)
        })
        .collect();

    // Sort by score (highest first)
    scored_assets.sort_by(|a, b| b.0.cmp(&a.0));

    if let Some((score, asset)) = scored_assets.first()
        && *score > 0
    {
        return Some(asset.name.clone());
    }

    None
}

/// Guess a basic binary name as fallback
fn guess_binary_name() -> String {
    if std::env::consts::OS == "windows" {
        "windows".to_string()
    } else {
        "linux".to_string()
    }
}
