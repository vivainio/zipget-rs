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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_compute_sha256_from_bytes_empty() {
        // SHA-256 of empty input
        let hash = compute_sha256_from_bytes(b"");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_compute_sha256_from_bytes_hello() {
        // SHA-256 of "hello"
        let hash = compute_sha256_from_bytes(b"hello");
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_compute_sha256_from_bytes_deterministic() {
        let data = b"test data for hashing";
        let hash1 = compute_sha256_from_bytes(data);
        let hash2 = compute_sha256_from_bytes(data);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_compute_sha256_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"hello").unwrap();
        temp_file.flush().unwrap();

        let hash = compute_sha256(temp_file.path()).unwrap();
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_verify_sha256_correct() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"hello").unwrap();
        temp_file.flush().unwrap();

        let result = verify_sha256(
            temp_file.path(),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
        )
        .unwrap();
        assert!(result);
    }

    #[test]
    fn test_verify_sha256_case_insensitive() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"hello").unwrap();
        temp_file.flush().unwrap();

        // Test uppercase input
        let result = verify_sha256(
            temp_file.path(),
            "2CF24DBA5FB0A30E26E83B2AC5B9E29E1B161E5C1FA7425E73043362938B9824",
        )
        .unwrap();
        assert!(result);
    }

    #[test]
    fn test_verify_sha256_incorrect() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"hello").unwrap();
        temp_file.flush().unwrap();

        let result = verify_sha256(
            temp_file.path(),
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .unwrap();
        assert!(!result);
    }
}
