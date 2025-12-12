use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Check if a file is executable
pub fn is_executable(path: &Path) -> Result<bool> {
    let metadata = fs::metadata(path)?;

    if !metadata.is_file() {
        return Ok(false);
    }

    #[cfg(windows)]
    {
        if let Some(ext) = path.extension() {
            if ext.to_string_lossy().to_lowercase() == "exe" {
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode();
        Ok(mode & 0o111 != 0)
    }
}

/// Find all executable files in a directory recursively
pub fn find_executables(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut executables = Vec::new();

    fn visit_dir(dir: &Path, executables: &mut Vec<PathBuf>) -> Result<()> {
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

/// Install a package (executables) to the system
pub fn install_package(
    source: &str,
    binary: Option<&str>,
    tag: Option<&str>,
    files_pattern: Option<&str>,
    profile: Option<&str>,
    executable: Option<&str>,
    no_shim: bool,
) -> Result<()> {
    use crate::archive::utils as archive_utils;
    use crate::download::github;
    use crate::install::shim;

    // Check if no_shim is used on non-Windows
    #[cfg(not(windows))]
    {
        if no_shim {
            return Err(anyhow::anyhow!(
                "--no-shim is only supported on Windows. On Unix systems, use default shim installation."
            ));
        }
    }

    // Create temporary directory for extraction
    let temp_base = std::env::temp_dir();
    let temp_dir_name = format!("zipget_install_{}", std::process::id());
    let temp_path = temp_base.join(&temp_dir_name);
    fs::create_dir_all(&temp_path).context("Failed to create temporary directory")?;

    // Determine if source is a GitHub repository or direct URL
    if source.contains("github.com/") {
        // Extract user/repo from GitHub URL
        let repo_path = if source.contains("github.com/") {
            source
                .strip_prefix("https://")
                .or_else(|| source.strip_prefix("http://"))
                .unwrap_or(source)
                .strip_prefix("github.com/")
                .unwrap_or(source)
                .split('/')
                .take(2)
                .collect::<Vec<_>>()
                .join("/")
        } else {
            source.to_string()
        };

        // Download from GitHub release
        println!("Downloading from GitHub: {repo_path}");
        github::fetch_github_release(
            &repo_path,
            binary,
            None,
            tag,
            Some(temp_path.to_str().unwrap()),
            files_pattern,
        )?;
    } else {
        // Direct URL download
        crate::runner::fetch_direct_url(
            source,
            None,
            Some(temp_path.to_str().unwrap()),
            files_pattern,
            profile,
        )?;
    }

    // Flatten directory if needed
    if let Ok(Some(dir_name)) = archive_utils::should_flatten_directory(&temp_path) {
        archive_utils::flatten_directory_structure(&temp_path, &dir_name)?;
    }

    // Find executables in extracted content
    let executables = find_executables(&temp_path).context("Failed to find executables")?;

    if executables.is_empty() {
        return Err(anyhow::anyhow!("No executables found in package"));
    }

    // Select which executable to install
    let exe_to_install = if let Some(exe_name) = executable {
        executables
            .iter()
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.contains(exe_name))
                    .unwrap_or(false)
            })
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Executable '{}' not found", exe_name))?
    } else if executables.len() == 1 {
        executables[0].clone()
    } else {
        return Err(anyhow::anyhow!(
            "Multiple executables found, please specify which one with --executable"
        ));
    };

    #[cfg(windows)]
    {
        if no_shim {
            // Copy to ~/.local/bin
            let local_bin = dirs::home_dir()
                .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
                .join(".local")
                .join("bin");

            fs::create_dir_all(&local_bin).context("Failed to create ~/.local/bin directory")?;

            let filename = exe_to_install
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Could not determine executable name"))?;
            let install_path = local_bin.join(filename);

            fs::copy(&exe_to_install, &install_path)
                .context("Failed to copy executable to ~/.local/bin")?;

            println!("Installed to: {}", install_path.display());
        } else {
            // Use shim installation
            let exe_path = exe_to_install
                .canonicalize()
                .context("Failed to get absolute path of executable")?;

            shim::create_shim(
                exe_path
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid path"))?,
            )?;

            println!("Shim created for: {}", exe_to_install.display());
        }
    }

    #[cfg(unix)]
    {
        use crate::install::utils::is_directory_in_path;

        // On Unix, install to ~/.local/bin
        let local_bin = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
            .join(".local")
            .join("bin");

        fs::create_dir_all(&local_bin).context("Failed to create ~/.local/bin directory")?;

        let filename = exe_to_install
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Could not determine executable name"))?;
        let install_path = local_bin.join(filename);

        fs::copy(&exe_to_install, &install_path)
            .context("Failed to copy executable to ~/.local/bin")?;

        // Make executable on Unix
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&install_path, perms)
            .context("Failed to set executable permissions")?;

        println!("Installed to: {}", install_path.display());

        // Check if in PATH
        if !is_directory_in_path(local_bin.as_path()) {
            println!(
                "Warning: {} is not in your PATH. Add it with: export PATH=\"$PATH:{}\"",
                local_bin.display(),
                local_bin.display()
            );
        }
    }

    // Cleanup temporary directory
    let _ = fs::remove_dir_all(&temp_path);

    Ok(())
}
