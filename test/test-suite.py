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
            ("small-test.zip", 8000, 12000),  # ~10KB
            ("medium-test.zip", 80000, 120000),  # ~100KB
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