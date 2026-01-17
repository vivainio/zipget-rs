use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

// Embed the scoop shim executable at compile time (Windows only)
#[cfg(windows)]
pub static SCOOP_SHIM_BYTES: &[u8] = include_bytes!("../../shims/shim_scoop.exe");

/// Create a shim/launcher for an executable or JAR file
pub fn create_shim(target: &str, name: Option<&str>, java_opts: Option<&str>) -> Result<()> {
    let target_path = PathBuf::from(target);
    let abs_target = target_path
        .canonicalize()
        .context("Failed to get absolute path of target file")?;

    if !abs_target.exists() {
        return Err(anyhow::anyhow!(
            "Target file does not exist: {}",
            abs_target.display()
        ));
    }

    // Determine the launcher name
    let default_name = abs_target
        .file_stem()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Could not determine file name"))?;
    let launcher_name = name.unwrap_or(default_name);

    // Create ~/.local/bin directory
    let local_bin = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join(".local")
        .join("bin");
    fs::create_dir_all(&local_bin).context("Failed to create ~/.local/bin directory")?;

    // Check if target is a JAR file
    let is_jar = abs_target
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("jar"))
        .unwrap_or(false);

    if is_jar {
        create_jar_launcher(&local_bin, launcher_name, &abs_target, java_opts)?;
    } else {
        create_executable_shim(&local_bin, launcher_name, &abs_target)?;
    }

    Ok(())
}

/// Create a launcher script for a JAR file
fn create_jar_launcher(
    local_bin: &PathBuf,
    name: &str,
    jar_path: &PathBuf,
    java_opts: Option<&str>,
) -> Result<()> {
    let java_opts_str = java_opts.unwrap_or("");

    #[cfg(windows)]
    {
        // Create a .cmd batch file on Windows
        let launcher_path = local_bin.join(format!("{name}.cmd"));
        let script = if java_opts_str.is_empty() {
            format!("@java -jar \"{}\" %*\r\n", jar_path.display())
        } else {
            format!(
                "@java {java_opts_str} -jar \"{}\" %*\r\n",
                jar_path.display()
            )
        };
        fs::write(&launcher_path, &script).context("Failed to write launcher script")?;
        println!("Created JAR launcher: {}", launcher_path.display());
    }

    #[cfg(not(windows))]
    {
        // Create a shell script on Unix
        let launcher_path = local_bin.join(name);
        let script = if java_opts_str.is_empty() {
            format!(
                "#!/bin/sh\nexec java -jar \"{}\" \"$@\"\n",
                jar_path.display()
            )
        } else {
            format!(
                "#!/bin/sh\nexec java {java_opts_str} -jar \"{}\" \"$@\"\n",
                jar_path.display()
            )
        };
        fs::write(&launcher_path, &script).context("Failed to write launcher script")?;

        // Set executable permission
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&launcher_path, perms).context("Failed to set executable permission")?;

        println!("Created JAR launcher: {}", launcher_path.display());
    }

    Ok(())
}

/// Create a shim for a native executable
fn create_executable_shim(
    local_bin: &PathBuf,
    name: &str,
    exe_path: &PathBuf,
) -> Result<()> {
    #[cfg(windows)]
    {
        // Use Scoop-style shim on Windows
        let shim_config_path = local_bin.join(format!("{name}.shim"));
        let shim_config = format!("path = {}\nargs =", exe_path.display());
        fs::write(&shim_config_path, &shim_config).context("Failed to write .shim config file")?;
        println!("Created shim config: {}", shim_config_path.display());

        let shim_exe_path = local_bin.join(format!("{name}.exe"));
        fs::write(&shim_exe_path, SCOOP_SHIM_BYTES).context("Failed to write shim executable")?;
        println!("Created shim executable: {}", shim_exe_path.display());
    }

    #[cfg(not(windows))]
    {
        // Create a shell script wrapper on Unix
        let launcher_path = local_bin.join(name);
        let script = format!(
            "#!/bin/sh\nexec \"{}\" \"$@\"\n",
            exe_path.display()
        );
        fs::write(&launcher_path, &script).context("Failed to write launcher script")?;

        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&launcher_path, perms).context("Failed to set executable permission")?;

        println!("Created shim: {}", launcher_path.display());
    }

    println!("Shim created successfully for: {name}");
    Ok(())
}
