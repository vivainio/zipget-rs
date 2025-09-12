use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;

/// Compute SHA-256 hash of a file
pub fn compute_sha256(file_path: &Path) -> Result<String> {
    let mut file = fs::File::open(file_path).with_context(|| {
        format!(
            "Failed to open file for SHA verification: {}",
            file_path.display()
        )
    })?;

    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];

    loop {
        let bytes_read = file.read(&mut buffer).with_context(|| {
            format!(
                "Failed to read file for SHA verification: {}",
                file_path.display()
            )
        })?;

        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Compute SHA-256 hash of byte data
pub fn compute_sha256_from_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Verify SHA-256 hash of a file against expected hash
pub fn verify_sha256(file_path: &Path, expected_sha: &str) -> Result<bool> {
    let computed_sha = compute_sha256(file_path)?;
    let expected_sha_lower = expected_sha.to_lowercase();

    if computed_sha == expected_sha_lower {
        println!("✓ SHA-256 verification passed: {expected_sha_lower}");
        Ok(true)
    } else {
        println!("✗ SHA-256 verification failed!");
        println!("  Expected: {expected_sha_lower}");
        println!("  Computed: {computed_sha}");
        Ok(false)
    }
}
