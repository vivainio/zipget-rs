#[cfg(windows)]
use anyhow::Context;
use anyhow::Result;
#[cfg(windows)]
use std::fs;

// Embed the scoop shim executable at compile time
#[cfg(windows)]
pub static SCOOP_SHIM_BYTES: &[u8] = include_bytes!("../../shims/shim_scoop.exe");

/// Create a shim for an executable
pub fn create_shim(#[cfg_attr(not(windows), allow(unused))] target_executable: &str) -> Result<()> {
    #[cfg(not(windows))]
    {
        Err(anyhow::anyhow!(
            "Shim creation is only supported on Windows"
        ))
    }

    #[cfg(windows)]
    {
        use std::path::PathBuf;

        // Convert target path to absolute
        let target_path = PathBuf::from(target_executable);
        let abs_target = target_path
            .canonicalize()
            .context("Failed to get absolute path of target executable")?;

        // Verify target exists
        if !abs_target.exists() {
            return Err(anyhow::anyhow!(
                "Target executable does not exist: {}",
                abs_target.display()
            ));
        }

        // Get the executable name without extension
        let exe_name = abs_target
            .file_stem()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("Could not determine executable name"))?;

        // Create ~/.local/bin directory
        let local_bin = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
            .join(".local")
            .join("bin");

        fs::create_dir_all(&local_bin).context("Failed to create ~/.local/bin directory")?;

        // Create .shim config file
        let shim_path = local_bin.join(format!("{exe_name}.shim"));
        let shim_config = format!("path = {}\nargs =", abs_target.display());

        fs::write(&shim_path, &shim_config).context("Failed to write .shim config file")?;

        println!("Created shim config: {}", shim_path.display());

        // Copy SCOOP_SHIM_BYTES to <name>.exe
        let exe_path = local_bin.join(format!("{exe_name}.exe"));

        fs::write(&exe_path, SCOOP_SHIM_BYTES).context("Failed to write shim executable")?;

        println!("Created shim executable: {}", exe_path.display());
        println!("Shim created successfully for: {exe_name}");

        Ok(())
    }
}
