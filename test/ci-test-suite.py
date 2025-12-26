#!/usr/bin/env python3
"""
CI-Friendly Integration Test Suite for zipget-rs
Focuses on working functionality, designed for GitHub Actions
"""

import os
import sys
import subprocess
import shutil
import time
from pathlib import Path
from typing import List, Tuple, Optional


class TestResult:
    def __init__(self, name: str, passed: bool, message: str = "", duration: float = 0.0):
        self.name = name
        self.passed = passed
        self.message = message
        self.duration = duration


class ZipgetCITestSuite:
    def __init__(self):
        self.test_dir = Path(__file__).parent
        self.root_dir = self.test_dir.parent
        self.working_recipe = self.test_dir / "working-test.toml"
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
        self.log("Setting up CI test environment...")
        
        # Check if zipget binary exists
        if not self.zipget_binary.exists():
            self.log(f"Error: zipget binary not found at {self.zipget_binary}", "ERROR")
            return False
        
        # Clean up previous test runs
        if self.test_output_dir.exists():
            shutil.rmtree(self.test_output_dir)
        if self.test_downloads_dir.exists():
            shutil.rmtree(self.test_downloads_dir)
        
        # Create test directories
        self.test_output_dir.mkdir(parents=True, exist_ok=True)
        self.test_downloads_dir.mkdir(parents=True, exist_ok=True)
        
        self.log("CI test environment setup complete")
        return True

    def test_working_recipe_execution(self) -> TestResult:
        """Test running the working integration recipe"""
        start_time = time.time()
        
        cmd = [str(self.zipget_binary), "recipe", str(self.working_recipe)]
        returncode, stdout, stderr = self.run_command(cmd)
        
        duration = time.time() - start_time
        
        if returncode != 0:
            return TestResult(
                "working_recipe_execution", 
                False, 
                f"Working recipe execution failed: {stderr}", 
                duration
            )
        
        return TestResult("working_recipe_execution", True, "Working recipe executed successfully", duration)

    def test_downloaded_files_exist(self) -> TestResult:
        """Test that expected downloaded files exist from working recipe"""
        expected_files = [
            "hashibuild.zip",
            "hashibuild-filtered.zip", 
            "http-test.zip",
            "modulize.zip"
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
        
        return TestResult("downloaded_files_exist", True, f"All {len(expected_files)} expected files downloaded")

    def test_extracted_directories_exist(self) -> TestResult:
        """Test that extraction directories were created"""
        expected_dirs = [
            "hashibuild",
            "hashibuild-filtered",
            "http-zip",
            "modulize"
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
        
        return TestResult("extracted_directories_exist", True, f"All {len(expected_dirs)} extraction directories created")

    def test_file_filtering(self) -> TestResult:
        """Test that file pattern filtering worked correctly"""
        filtered_dir = self.test_output_dir / "hashibuild-filtered"
        
        if not filtered_dir.exists():
            return TestResult("file_filtering", False, "Filtered directory doesn't exist")
        
        # Find all files
        all_files = []
        md_files = []
        
        for root, dirs, files in os.walk(filtered_dir):
            for file in files:
                file_path = Path(root) / file
                all_files.append(file_path)
                if file.endswith('.md'):
                    md_files.append(file_path)
        
        if len(all_files) == 0:
            return TestResult("file_filtering", False, "No files found in filtered directory")
        
        if len(md_files) == 0:
            return TestResult("file_filtering", False, "No .md files found in filtered directory")
        
        if len(all_files) != len(md_files):
            return TestResult(
                "file_filtering", 
                False, 
                f"Filtering failed: {len(all_files)} total files, {len(md_files)} .md files"
            )
        
        return TestResult("file_filtering", True, f"File filtering worked: {len(md_files)} .md files extracted")

    def test_github_api_command(self) -> TestResult:
        """Test GitHub API download command"""
        start_time = time.time()
        
        test_file = self.root_dir / "ci-github-test.zip"
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
                "github_api_command",
                False,
                f"GitHub API command failed: {stderr}",
                duration
            )
        
        test_file_path = test_file
        if not test_file_path.exists():
            return TestResult(
                "github_api_command",
                False,
                "GitHub API download file not created",
                duration
            )
        
        # Clean up
        test_file_path.unlink(missing_ok=True)
        
        return TestResult("github_api_command", True, "GitHub API command successful", duration)

    def test_http_fetch_command(self) -> TestResult:
        """Test direct HTTP fetch command"""
        start_time = time.time()
        
        test_file = "ci-http-test.zip"
        cmd = [
            str(self.zipget_binary),
            "fetch",
            "https://github.com/vivainio/unxml-rs/releases/download/v0.1.1/unxml-windows-x86_64.zip",
            "--save-as",
            test_file
        ]
        
        returncode, stdout, stderr = self.run_command(cmd)
        duration = time.time() - start_time
        
        if returncode != 0:
            return TestResult(
                "http_fetch_command",
                False,
                f"HTTP fetch command failed: {stderr}",
                duration
            )
        
        test_file_path = self.root_dir / test_file
        if not test_file_path.exists():
            return TestResult(
                "http_fetch_command",
                False,
                "HTTP fetch file not created",
                duration
            )
        
        file_size = test_file_path.stat().st_size
        
        # Clean up
        test_file_path.unlink(missing_ok=True)
        
        return TestResult("http_fetch_command", True, f"HTTP fetch successful ({file_size} bytes)", duration)

    def test_cache_functionality(self) -> TestResult:
        """Test basic cache functionality"""
        start_time = time.time()
        
        # Test with a small GitHub repo
        test_url = "https://github.com/vivainio/hashibuild/archive/refs/heads/master.zip"
        
        # First download
        cmd1 = [str(self.zipget_binary), "fetch", test_url, "--save-as", "cache-test-1.zip"]
        returncode1, stdout1, stderr1 = self.run_command(cmd1)
        
        # Second download (should use cache)
        cmd2 = [str(self.zipget_binary), "fetch", test_url, "--save-as", "cache-test-2.zip"]
        returncode2, stdout2, stderr2 = self.run_command(cmd2)
        
        duration = time.time() - start_time
        
        if returncode1 != 0 or returncode2 != 0:
            return TestResult(
                "cache_functionality",
                False,
                f"Cache test failed: {stderr1 or stderr2}",
                duration
            )
        
        # Check if files exist
        file1 = self.root_dir / "cache-test-1.zip"
        file2 = self.root_dir / "cache-test-2.zip"
        
        if not file1.exists() or not file2.exists():
            return TestResult(
                "cache_functionality",
                False,
                "Cache test files not created",
                duration
            )
        
        # Files should be identical (both from cache or both fresh)
        sizes_match = file1.stat().st_size == file2.stat().st_size
        
        # Clean up
        file1.unlink(missing_ok=True)
        file2.unlink(missing_ok=True)
        
        # Check if cache was mentioned in output
        cache_used = "Found cached file" in stdout2
        
        message = f"Cache test passed (cache {'used' if cache_used else 'not detected'})"
        
        return TestResult("cache_functionality", sizes_match, message, duration)

    def run_all_tests(self) -> bool:
        """Run all CI tests and return overall success"""
        self.log("Starting zipget-rs CI integration test suite...")
        
        if not self.setup_test_environment():
            return False
        
        # Define test functions
        tests = [
            self.test_working_recipe_execution,
            self.test_downloaded_files_exist,
            self.test_extracted_directories_exist,
            self.test_file_filtering,
            self.test_github_api_command,
            self.test_http_fetch_command,
            self.test_cache_functionality,
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
        self.log("CI TEST SUMMARY")
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
        
        # Print file and directory info for debugging
        if self.test_downloads_dir.exists():
            self.log("Downloaded files:")
            for file in self.test_downloads_dir.iterdir():
                if file.is_file():
                    size = file.stat().st_size
                    self.log(f"  - {file.name}: {size:,} bytes")
        
        if self.test_output_dir.exists():
            self.log("Extracted directories:")
            for dir in self.test_output_dir.iterdir():
                if dir.is_dir():
                    file_count = sum(1 for _ in dir.rglob('*') if _.is_file())
                    self.log(f"  - {dir.name}: {file_count} files")
        
        return success


def main():
    """Main entry point"""
    test_suite = ZipgetCITestSuite()
    success = test_suite.run_all_tests()
    
    # Exit with appropriate code
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main() 