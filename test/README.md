# zipget-rs Integration Tests

This directory contains integration tests for the zipget-rs project.

## Files

- `integration-test.toml` - Test recipe that covers various zipget-rs functionality
- `lock-test.toml` - Test recipe for --lock functionality and SHA verification
- `lock-test-no-tags.toml` - Test recipe for GitHub releases without explicit tags
- `lock-test-selective.toml` - Test recipe for selective locking of individual items
- `test-suite.py` - Python test runner that executes tests and validates results
- `ci-test-suite.py` - CI-specific test runner
- `README.md` - This documentation file

## Test Coverage

The integration tests cover:

- **GitHub Releases**: Download from vivainio/unxml-rs (latest and specific versions)
- **HTTP Downloads**: Download test ZIP files from thetestdata.com
- **Asset Detection**: Automatic platform-specific asset selection
- **Extraction**: ZIP and tar.gz archive extraction
- **File Filtering**: Using glob patterns to extract specific files
- **Caching**: Verify that downloaded files are cached properly
- **Individual Commands**: Test standalone GitHub download commands
- **Lock File Generation**: Test `--lock` parameter generates SHA-256 hashes
- **SHA Verification**: Test SHA-256 verification of downloaded files  
- **Tag Pinning**: Test `--lock` automatically pins GitHub releases without tags to specific versions
- **Selective Locking**: Test `--lock` with specific item only locks that item
- **Download URL Storage**: Test `--lock` stores direct download URLs for faster GitHub asset access

## Prerequisites

1. Build the release binary:
   ```bash
   cargo build --release
   ```

2. Install Python 3.6+ (the test suite uses standard library only)

## Running Tests

### Run All Tests
```bash
cd test
python test-suite.py
```

### Run Individual Recipe
```bash
# From project root
./target/release/zipget recipe test/integration-test.toml
```

### Run Specific Items from Recipe
```bash
# Only run unxml tests
./target/release/zipget recipe test/integration-test.toml unxml-latest unxml-specific-version

# Only run HTTP download tests
./target/release/zipget recipe test/integration-test.toml small-test-zip medium-test-zip
```

## Test Output

The test suite will:
1. Clean up any previous test runs
2. Execute the integration recipe
3. Validate that files were downloaded to `./test-downloads/`
4. Validate that files were extracted to `./test-output/`
5. Check file sizes and content
6. Test individual commands
7. Verify caching functionality
8. Test lock file generation with `--lock` parameter
9. Test SHA-256 verification (both success and failure cases)
10. Test automatic tag pinning for GitHub releases without explicit tags
11. Test selective locking of individual recipe items
12. Test download URL storage for GitHub assets
13. Print a detailed summary

### Example Output
```
[12:34:56] INFO: Starting zipget-rs integration test suite...
[12:34:56] INFO: Setting up test environment...
[12:34:56] INFO: Test environment setup complete
[12:34:56] INFO: Running: ./target/release/zipget recipe ./test/integration-test.toml
[12:35:15] INFO: PASS: recipe_execution (18.45s)
[12:35:15] INFO: PASS: downloaded_files_exist
[12:35:15] INFO: PASS: file_sizes
[12:35:15] INFO: PASS: extracted_directories_exist
[12:35:15] INFO: PASS: extracted_content
[12:35:20] INFO: PASS: individual_github_command (4.23s)
[12:35:25] INFO: PASS: cache_functionality (5.12s)
[12:35:25] INFO: PASS: lock_file_generation (0.29s)
[12:35:25] INFO: PASS: sha_verification (0.03s)
[12:35:25] INFO: PASS: tag_pinning (0.65s)
[12:35:25] INFO: PASS: selective_locking (0.22s)
[12:35:25] INFO: PASS: download_url_storage (0.15s)
==================================================
TEST SUMMARY
==================================================
Total tests: 12
Passed: 12
Failed: 0
Overall result: SUCCESS
```

## Test Directories

When tests run, they create:
- `../test-downloads/` - Downloaded ZIP/tar.gz files
- `../test-output/` - Extracted content from archives

These directories are cleaned up at the start of each test run.

## Troubleshooting

### Binary Not Found
```
Error: zipget binary not found at ./target/release/zipget
Please run: cargo build --release
```
Solution: Build the project first with `cargo build --release`

### Network Issues
If downloads fail due to network issues, you can:
1. Check your internet connection
2. Verify the test endpoints are accessible:
   ```bash
   curl -I https://thetestdata.com/samplefiles/zip/Thetestdata_ZIP_10KB.zip
   curl -I https://api.github.com/repos/vivainio/unxml-rs/releases/latest
   ```

### Permission Issues
On Unix systems, ensure the binary is executable:
```bash
chmod +x target/release/zipget
```

## Adding New Tests

To add new test cases:

1. **Add to recipe**: Edit `integration-test.toml` with new download items
2. **Update test expectations**: Modify the expected files/directories lists in `test-suite.py`
3. **Add validation**: Create new test methods in the `ZipgetTestSuite` class if needed

Example new recipe item:
```toml
[my-new-test]
url = "https://example.com/test.zip"
unzip_to = "./test-output/my-test"
save_as = "./test-downloads/my-test.zip"
``` 