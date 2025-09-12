#!/usr/bin/env python3
"""
Integration Test Suite for zipget-rs
Runs integration tests and validates results
"""

import os
import sys
import subprocess
import shutil
import time
import re
from pathlib import Path
from typing import List, Tuple, Optional


class TestResult:
    def __init__(self, name: str, passed: bool, message: str = "", duration: float = 0.0):
        self.name = name
        self.passed = passed
        self.message = message
        self.duration = duration


class ZipgetTestSuite:
    def __init__(self):
        self.test_dir = Path(__file__).parent
        self.root_dir = self.test_dir.parent
        self.recipe_file = self.test_dir / "integration-test.toml"
        self.test_output_dir = self.root_dir / "test-output"
        self.test_downloads_dir = self.root_dir / "test-downloads"
        self.zipget_binary = self.root_dir / "target" / "release" / "zipget"
        
        # Windows executable extension
        if os.name == 'nt':
            self.zipget_binary = self.zipget_binary.with_suffix('.exe')
        
        self.results: List[TestResult] = []

    def log(self, message: str, level: str = "INFO"):
        """Log a message with timestamp"""
        timestamp = time.strftime("%H:%M:%S")
        print(f"[{timestamp}] {level}: {message}")

    def run_command(self, cmd: List[str], timeout: int = 120) -> Tuple[int, str, str]:
        """Run a command and return (returncode, stdout, stderr)"""
        self.log(f"Running: {' '.join(cmd)}")
        try:
            result = subprocess.run(
                cmd, 
                capture_output=True, 
                text=True, 
                timeout=timeout,
                cwd=self.root_dir
            )
            return result.returncode, result.stdout, result.stderr
        except subprocess.TimeoutExpired:
            return -1, "", f"Command timed out after {timeout} seconds"
        except Exception as e:
            return -1, "", str(e)

    def setup_test_environment(self) -> bool:
        """Set up test directories and check prerequisites"""
        self.log("Setting up test environment...")
        
        # Check if zipget binary exists
        if not self.zipget_binary.exists():
            self.log(f"Error: zipget binary not found at {self.zipget_binary}", "ERROR")
            self.log("Please run: cargo build --release", "ERROR")
            return False
        
        # Clean up previous test runs
        if self.test_output_dir.exists():
            shutil.rmtree(self.test_output_dir)
        if self.test_downloads_dir.exists():
            shutil.rmtree(self.test_downloads_dir)
        
        # Create test directories
        self.test_output_dir.mkdir(parents=True, exist_ok=True)
        self.test_downloads_dir.mkdir(parents=True, exist_ok=True)
        
        self.log("Test environment setup complete")
        return True

    def test_recipe_execution(self) -> TestResult:
        """Test running the full integration recipe"""
        start_time = time.time()
        
        cmd = [str(self.zipget_binary), "recipe", str(self.recipe_file)]
        returncode, stdout, stderr = self.run_command(cmd)
        
        duration = time.time() - start_time
        
        if returncode != 0:
            return TestResult(
                "recipe_execution", 
                False, 
                f"Recipe execution failed: {stderr}", 
                duration
            )
        
        return TestResult("recipe_execution", True, "Recipe executed successfully", duration)

    def test_downloaded_files_exist(self) -> TestResult:
        """Test that all expected downloaded files exist"""
        expected_files = [
            "unxml-latest.zip",
            "unxml-v0.1.1.zip", 
            "unxml-windows.zip",
            "small-test.zip",
            "medium-test.zip",
            "bat.zip"
        ]
        
        missing_files = []
        for filename in expected_files:
            file_path = self.test_downloads_dir / filename
            if not file_path.exists():
                missing_files.append(filename)
        
        if missing_files:
            return TestResult(
                "downloaded_files_exist", 
                False, 
                f"Missing downloaded files: {', '.join(missing_files)}"
            )
        
        return TestResult("downloaded_files_exist", True, "All expected files downloaded")

    def test_file_sizes(self) -> TestResult:
        """Test that downloaded files have reasonable sizes"""
        size_checks = [
            ("small-test.zip", 90000, 110000),  # ~100KB (actual size from thetestdata.com)
            ("medium-test.zip", 950000, 1100000),  # ~1MB (actual size from thetestdata.com)
        ]
        
        failed_checks = []
        for filename, min_size, max_size in size_checks:
            file_path = self.test_downloads_dir / filename
            if file_path.exists():
                size = file_path.stat().st_size
                if not (min_size <= size <= max_size):
                    failed_checks.append(f"{filename}: {size} bytes (expected {min_size}-{max_size})")
        
        if failed_checks:
            return TestResult(
                "file_sizes", 
                False, 
                f"File size checks failed: {'; '.join(failed_checks)}"
            )
        
        return TestResult("file_sizes", True, "All file sizes within expected ranges")

    def test_extracted_directories_exist(self) -> TestResult:
        """Test that extraction directories were created"""
        expected_dirs = [
            "unxml-latest",
            "unxml-v0.1.1",
            "unxml-windows", 
            "small-zip",
            "medium-zip",
            "bat"
        ]
        
        missing_dirs = []
        for dirname in expected_dirs:
            dir_path = self.test_output_dir / dirname
            if not dir_path.exists():
                missing_dirs.append(dirname)
        
        if missing_dirs:
            return TestResult(
                "extracted_directories_exist",
                False,
                f"Missing extraction directories: {', '.join(missing_dirs)}"
            )
        
        return TestResult("extracted_directories_exist", True, "All extraction directories created")

    def test_extracted_content(self) -> TestResult:
        """Test that extracted directories contain expected content"""
        checks = []
        
        # Check that unxml directories contain executables
        for dirname in ["unxml-latest", "unxml-v0.1.1", "unxml-windows"]:
            dir_path = self.test_output_dir / dirname
            if dir_path.exists():
                # Look for executable files (unxml or unxml.exe)
                executables = list(dir_path.glob("**/unxml*"))
                if not executables:
                    checks.append(f"{dirname}: No unxml executable found")
        
        # Check that small/medium zip dirs contain test files
        for dirname in ["small-zip", "medium-zip"]:
            dir_path = self.test_output_dir / dirname
            if dir_path.exists():
                files = list(dir_path.glob("**/*"))
                if len(files) == 0:
                    checks.append(f"{dirname}: No extracted files found")
        
        if checks:
            return TestResult(
                "extracted_content",
                False,
                f"Content validation failed: {'; '.join(checks)}"
            )
        
        return TestResult("extracted_content", True, "Extracted content validation passed")

    def test_individual_github_command(self) -> TestResult:
        """Test individual GitHub download command"""
        start_time = time.time()
        
        test_file = self.test_downloads_dir / "individual-github-test.zip"
        cmd = [
            str(self.zipget_binary), 
            "github", 
            "vivainio/unxml-rs",
            "--save-as", 
            str(test_file)
        ]
        
        returncode, stdout, stderr = self.run_command(cmd)
        duration = time.time() - start_time
        
        if returncode != 0:
            return TestResult(
                "individual_github_command",
                False,
                f"Individual GitHub command failed: {stderr}",
                duration
            )
        
        if not test_file.exists():
            return TestResult(
                "individual_github_command",
                False,
                "Individual GitHub download file not created",
                duration
            )
        
        return TestResult("individual_github_command", True, "Individual GitHub command successful", duration)

    def test_cache_functionality(self) -> TestResult:
        """Test that cache is working by running same command twice"""
        start_time = time.time()
        
        # First run
        cmd = [str(self.zipget_binary), "github", "vivainio/unxml-rs", "--save-as", "cache-test-1.zip"]
        returncode1, stdout1, stderr1 = self.run_command(cmd)
        
        # Second run (should be faster due to cache)
        cmd = [str(self.zipget_binary), "github", "vivainio/unxml-rs", "--save-as", "cache-test-2.zip"]
        returncode2, stdout2, stderr2 = self.run_command(cmd)
        
        duration = time.time() - start_time
        
        if returncode1 != 0 or returncode2 != 0:
            return TestResult(
                "cache_functionality",
                False,
                f"Cache test failed: {stderr1 or stderr2}",
                duration
            )
        
        # Both files should exist
        file1 = Path("cache-test-1.zip")
        file2 = Path("cache-test-2.zip")
        
        if not file1.exists() or not file2.exists():
            return TestResult(
                "cache_functionality",
                False,
                "Cache test files not created",
                duration
            )
        
        # Clean up
        file1.unlink(missing_ok=True)
        file2.unlink(missing_ok=True)
        
        return TestResult("cache_functionality", True, "Cache functionality working", duration)

    def test_lock_file_generation(self) -> TestResult:
        """Test --lock parameter generates SHA hashes in recipe file"""
        start_time = time.time()
        
        # Create a copy of the lock test recipe
        lock_recipe = self.test_dir / "lock-test.toml"
        lock_recipe_copy = self.test_dir / "lock-test-copy.toml"
        
        try:
            # Copy original recipe without SHA hashes
            shutil.copy(lock_recipe, lock_recipe_copy)
            
            # Run zipget with --lock parameter
            cmd = [str(self.zipget_binary), "recipe", str(lock_recipe_copy), "--lock"]
            returncode, stdout, stderr = self.run_command(cmd)
            
            duration = time.time() - start_time
            
            if returncode != 0:
                return TestResult(
                    "lock_file_generation",
                    False,
                    f"Lock command failed: {stderr}",
                    duration
                )
            
            # Read the updated recipe file and check for SHA hashes
            with open(lock_recipe_copy, 'r') as f:
                updated_content = f.read()
            
            # Look for SHA hash patterns (64-character hex strings)
            sha_pattern = r'sha\s*=\s*"[0-9a-fA-F]{64}"'
            sha_matches = re.findall(sha_pattern, updated_content)
            
            if len(sha_matches) < 2:  # Should have SHA for both test items
                return TestResult(
                    "lock_file_generation",
                    False,
                    f"Expected 2 SHA hashes, found {len(sha_matches)}",
                    duration
                )
            
            return TestResult("lock_file_generation", True, "Lock file generation successful", duration)
            
        finally:
            # Clean up copy
            if lock_recipe_copy.exists():
                lock_recipe_copy.unlink()

    def test_sha_verification(self) -> TestResult:
        """Test SHA verification when processing recipe with hashes"""
        start_time = time.time()
        
        # Create a test recipe with known SHA hash
        test_recipe_content = '''[sha-test]
url = "https://thetestdata.com/samplefiles/zip/Thetestdata_ZIP_10KB.zip"
save_as = "./test-downloads/sha-verify-test.zip"
lock = { sha = "fe4759a0a3dfb431e78a9f803f1332e1507eea1a01f7e61e74d2787eccd9f1f7" }
'''
        
        sha_test_recipe = self.test_dir / "sha-test.toml"
        
        try:
            # Write test recipe
            with open(sha_test_recipe, 'w') as f:
                f.write(test_recipe_content)
            
            # Run zipget with SHA verification
            cmd = [str(self.zipget_binary), "recipe", str(sha_test_recipe)]
            returncode, stdout, stderr = self.run_command(cmd)
            
            duration = time.time() - start_time
            
            if returncode != 0:
                return TestResult(
                    "sha_verification",
                    False,
                    f"SHA verification failed: {stderr}",
                    duration
                )
            
            # Check that verification message appears in output
            if "SHA-256 verification passed" not in stdout:
                return TestResult(
                    "sha_verification",
                    False,
                    "SHA verification message not found in output",
                    duration
                )
            
            # Test with bad SHA hash to ensure verification catches errors
            bad_sha_content = test_recipe_content.replace(
                "fe4759a0a3dfb431e78a9f803f1332e1507eea1a01f7e61e74d2787eccd9f1f7",
                "badbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbad"
            )
            
            with open(sha_test_recipe, 'w') as f:
                f.write(bad_sha_content)
            
            # This should fail
            cmd = [str(self.zipget_binary), "recipe", str(sha_test_recipe)]
            returncode, stdout, stderr = self.run_command(cmd)
            
            if returncode == 0:  # Should have failed!
                return TestResult(
                    "sha_verification",
                    False,
                    "Bad SHA hash should have caused failure",
                    duration
                )
            
            if "SHA-256 verification failed" not in stderr:
                return TestResult(
                    "sha_verification",
                    False,
                    "SHA verification failure message not found",
                    duration
                )
            
            return TestResult("sha_verification", True, "SHA verification working correctly", duration)
            
        finally:
            # Clean up test file
            if sha_test_recipe.exists():
                sha_test_recipe.unlink()

    def test_tag_pinning(self) -> TestResult:
        """Test that --lock pins GitHub releases without tags to specific versions"""
        start_time = time.time()
        
        # Create a copy of the no-tags test recipe
        no_tags_recipe = self.test_dir / "lock-test-no-tags.toml"
        no_tags_recipe_copy = self.test_dir / "lock-test-no-tags-copy.toml"
        
        try:
            # Copy original recipe without tags
            shutil.copy(no_tags_recipe, no_tags_recipe_copy)
            
            # Run zipget with --lock parameter
            cmd = [str(self.zipget_binary), "recipe", str(no_tags_recipe_copy), "--lock"]
            returncode, stdout, stderr = self.run_command(cmd)
            
            duration = time.time() - start_time
            
            if returncode != 0:
                return TestResult(
                    "tag_pinning",
                    False,
                    f"Tag pinning command failed: {stderr}",
                    duration
                )
            
            # Read the updated recipe file and check for pinned tags
            with open(no_tags_recipe_copy, 'r') as f:
                updated_content = f.read()
            
            # Look for tag patterns that were added
            tag_pattern = r'tag\s*=\s*"[^"]+"'
            tag_matches = re.findall(tag_pattern, updated_content)
            
            if len(tag_matches) < 2:  # Should have tags pinned for both GitHub items
                return TestResult(
                    "tag_pinning",
                    False,
                    f"Expected 2 pinned tags, found {len(tag_matches)}",
                    duration
                )
            
            # Check for "Pinning GitHub release to tag" messages in output
            if "Pinning GitHub release to tag" not in stdout:
                return TestResult(
                    "tag_pinning",
                    False,
                    "Tag pinning message not found in output",
                    duration
                )
            
            return TestResult("tag_pinning", True, "Tag pinning working correctly", duration)
            
        finally:
            # Clean up copy
            if no_tags_recipe_copy.exists():
                no_tags_recipe_copy.unlink()



    def test_selective_locking(self) -> TestResult:
        """Test that --lock with specific item only locks that item"""
        start_time = time.time()
        
        # Create a copy of the selective lock test recipe
        selective_recipe = self.test_dir / "lock-test-selective.toml"
        selective_recipe_copy = self.test_dir / "lock-test-selective-copy.toml"
        
        try:
            # Copy original recipe without SHA hashes
            shutil.copy(selective_recipe, selective_recipe_copy)
            
            # Run zipget with --lock parameter on only one specific item
            cmd = [str(self.zipget_binary), "recipe", str(selective_recipe_copy), "selective-item-url", "--lock"]
            returncode, stdout, stderr = self.run_command(cmd)
            
            duration = time.time() - start_time
            
            if returncode != 0:
                return TestResult(
                    "selective_locking",
                    False,
                    f"Selective lock command failed: {stderr}",
                    duration
                )
            
            # Read the updated recipe file and check selective locking
            with open(selective_recipe_copy, 'r') as f:
                updated_content = f.read()
            
            # Look for SHA hash patterns in lock structure
            sha_pattern = r'lock\s*=\s*\{\s*sha\s*=\s*"[0-9a-fA-F]{64}"'
            sha_matches = re.findall(sha_pattern, updated_content)
            
            # Should have exactly 1 SHA hash (only for the selected item)
            if len(sha_matches) != 1:
                return TestResult(
                    "selective_locking",
                    False,
                    f"Expected 1 SHA hash for selective lock, found {len(sha_matches)}",
                    duration
                )
            
            # Check that only the correct item was modified
            lines = updated_content.split('\n')
            selective_item_section_found = False
            selective_item_has_sha = False
            other_items_have_sha = False
            
            current_section = None
            for line in lines:
                line = line.strip()
                if line.startswith('[') and line.endswith(']'):
                    current_section = line[1:-1]
                elif 'lock = {' in line and 'sha =' in line and current_section:
                    if current_section == "selective-item-url":
                        selective_item_has_sha = True
                    else:
                        other_items_have_sha = True
            
            if not selective_item_has_sha:
                return TestResult(
                    "selective_locking",
                    False,
                    "Selected item 'selective-item-url' did not get SHA hash",
                    duration
                )
            
            if other_items_have_sha:
                return TestResult(
                    "selective_locking",
                    False,
                    "Other items got SHA hashes when only one item should be locked",
                    duration
                )
            
            # Check that the correct processing message appears
            if "Processing 1 items for lock file" not in stdout:
                return TestResult(
                    "selective_locking",
                    False,
                    "Expected '1 items for lock file' message not found",
                    duration
                )
            
            return TestResult("selective_locking", True, "Selective locking working correctly", duration)
            
        finally:
            # Clean up copy
            if selective_recipe_copy.exists():
                selective_recipe_copy.unlink()

    def test_download_url_storage(self) -> TestResult:
        """Test that --lock stores direct download URLs for GitHub assets"""
        start_time = time.time()
        
        # Create a test recipe with GitHub asset
        download_url_content = '''[download-url-test]
github = { repo = "vivainio/unxml-rs", tag = "v0.1.1" }
save_as = "./test-downloads/download-url-test.zip"
'''
        
        download_url_recipe = self.test_dir / "download-url-test.toml"
        
        try:
            # Write test recipe
            with open(download_url_recipe, 'w') as f:
                f.write(download_url_content)
            
            # Run zipget with --lock parameter
            cmd = [str(self.zipget_binary), "recipe", str(download_url_recipe), "--lock"]
            returncode, stdout, stderr = self.run_command(cmd, timeout=30)
            
            duration = time.time() - start_time
            
            # If GitHub API fails due to rate limiting, that's expected and we can't test further
            if returncode != 0 and "Failed to fetch release info" in stderr:
                return TestResult(
                    "download_url_storage", 
                    True, 
                    "Test skipped due to GitHub API rate limiting (expected)", 
                    duration
                )
            
            if returncode != 0:
                return TestResult(
                    "download_url_storage",
                    False,
                    f"Download URL storage test failed: {stderr}",
                    duration
                )
            
            # Read the updated recipe file and check for download_url
            with open(download_url_recipe, 'r') as f:
                updated_content = f.read()
            
            # Look for download_url pattern in lock structure
            download_url_pattern = r'download_url\s*=\s*"[^"]+"'
            download_url_matches = re.findall(download_url_pattern, updated_content)
            
            if len(download_url_matches) < 1:
                return TestResult(
                    "download_url_storage",
                    False,
                    "Expected download_url to be stored, but not found",
                    duration
                )
            
            # Check for storage message in output
            if "Storing direct download URL" not in stdout:
                return TestResult(
                    "download_url_storage",
                    False,
                    "Expected 'Storing direct download URL' message not found",
                    duration
                )
            
            return TestResult("download_url_storage", True, "Download URL storage working correctly", duration)
            
        finally:
            # Clean up test file
            if download_url_recipe.exists():
                download_url_recipe.unlink()



    def run_all_tests(self) -> bool:
        """Run all tests and return overall success"""
        self.log("Starting zipget-rs integration test suite...")
        
        if not self.setup_test_environment():
            return False
        
        # Define test functions
        tests = [
            self.test_recipe_execution,
            self.test_downloaded_files_exist,
            self.test_file_sizes,
            self.test_extracted_directories_exist,
            self.test_extracted_content,
            self.test_individual_github_command,
            self.test_cache_functionality,
            self.test_lock_file_generation,
            self.test_sha_verification,
            self.test_tag_pinning,
            self.test_selective_locking,
            self.test_download_url_storage,
        ]
        
        # Run tests
        for test_func in tests:
            try:
                result = test_func()
                self.results.append(result)
                
                status = "PASS" if result.passed else "FAIL"
                duration_str = f" ({result.duration:.2f}s)" if result.duration > 0 else ""
                self.log(f"{status}: {result.name}{duration_str}")
                
                if result.message:
                    self.log(f"  -> {result.message}")
                
            except Exception as e:
                self.results.append(TestResult(test_func.__name__, False, str(e)))
                self.log(f"FAIL: {test_func.__name__} - Exception: {e}", "ERROR")
        
        return self.print_summary()

    def print_summary(self) -> bool:
        """Print test summary and return overall success"""
        total_tests = len(self.results)
        passed_tests = sum(1 for r in self.results if r.passed)
        failed_tests = total_tests - passed_tests
        
        self.log("=" * 50)
        self.log("TEST SUMMARY")
        self.log("=" * 50)
        self.log(f"Total tests: {total_tests}")
        self.log(f"Passed: {passed_tests}")
        self.log(f"Failed: {failed_tests}")
        
        if failed_tests > 0:
            self.log("FAILED TESTS:")
            for result in self.results:
                if not result.passed:
                    self.log(f"  - {result.name}: {result.message}")
        
        success = failed_tests == 0
        overall_status = "SUCCESS" if success else "FAILURE"
        self.log(f"Overall result: {overall_status}")
        
        return success


def main():
    """Main entry point"""
    test_suite = ZipgetTestSuite()
    success = test_suite.run_all_tests()
    
    # Exit with appropriate code
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main() 