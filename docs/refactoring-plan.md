# Refactoring Plan for zipget-rs

## Current State
The project currently has a monolithic `src/main.rs` file with 2,444 lines containing all functionality. This makes the code difficult to maintain, test, and understand.

## Goals
- Improve code organization and maintainability
- Enable better unit testing
- Reduce compilation times through better modularity
- Make the codebase easier for new contributors to understand

## Proposed Module Structure

### 1. `src/lib.rs` - Library Root
- Re-export public interfaces from other modules
- Define common error types and result aliases
- Module declarations

### 2. `src/cli.rs` - Command Line Interface
**Functions to move:**
- `main()` function
- `Args`, `Commands` enum definitions
- Command argument parsing and routing

**Dependencies:**
- clap parsing structures
- Main command routing logic

### 3. `src/models.rs` - Data Structures
**Structs/Enums to move:**
- `FetchItem`
- `GitHubFetch`
- `GitHubRelease`
- `GitHubAsset` 
- `LockInfo`
- `LockResult`
- `Recipe` type alias

**Purpose:**
- Centralize all data models and serialization logic
- Keep serde derives and validation logic together

### 4. `src/download/mod.rs` - Download Module
**Submodules:**
- `src/download/github.rs`
- `src/download/s3.rs`
- `src/download/http.rs`

#### 4a. `src/download/github.rs` - GitHub Operations
**Functions to move:**
- `fetch_github_release()`
- `get_best_binary_from_release()`
- `get_github_release_url()`
- `get_latest_github_tag()`
- `find_best_matching_binary()`
- `guess_binary_name()`

#### 4b. `src/download/s3.rs` - AWS S3 Operations
**Functions to move:**
- `download_s3_file()`
- `is_s3_url()`

#### 4c. `src/download/http.rs` - HTTP Downloads
**Functions to move:**
- `download_file()`
- `get_filename_from_url()`

### 5. `src/archive/mod.rs` - Archive Handling
**Submodules:**
- `src/archive/zip.rs`
- `src/archive/tar.rs`
- `src/archive/utils.rs`

#### 5a. `src/archive/zip.rs` - ZIP Extraction
**Functions to move:**
- `extract_zip()`

#### 5b. `src/archive/tar.rs` - TAR/GZ Extraction  
**Functions to move:**
- `extract_tar_gz()`

#### 5c. `src/archive/utils.rs` - Archive Utilities
**Functions to move:**
- `extract_archive_with_options()`
- `clean_archive_name_for_directory()`
- `should_flatten_directory()`
- `flatten_directory_structure()`

### 6. `src/install/mod.rs` - Installation Module
**Submodules:**
- `src/install/executable.rs`
- `src/install/shim.rs`
- `src/install/utils.rs`

#### 6a. `src/install/executable.rs` - Executable Management
**Functions to move:**
- `install_package()`
- `find_executables()`
- `is_executable()`

#### 6b. `src/install/shim.rs` - Shim Creation (Windows)
**Functions to move:**
- `create_shim()`
- SCOOP_SHIM_BYTES static

#### 6c. `src/install/utils.rs` - Installation Utilities
**Functions to move:**
- `copy_dir_all()`
- `is_directory_in_path()`

### 7. `src/recipe/mod.rs` - Recipe Processing
**Functions to move:**
- `process_fetch_item()`
- `process_fetch_item_for_lock()`
- `upgrade_recipe()`

### 8. `src/runner.rs` - Package Runner
**Functions to move:**
- `run_package()`
- `fetch_direct_url()`

### 9. `src/cache.rs` - Caching Utilities
**Functions to move:**
- `get_cache_dir()`

### 10. `src/crypto.rs` - Cryptographic Operations
**Functions to move:**
- `compute_sha256()`
- `compute_sha256_from_bytes()`
- `verify_sha256()`

### 11. `src/utils.rs` - General Utilities
**Functions to move:**
- `is_version_like()`
- `is_platform_identifier()`

## Migration Strategy

### Phase 1: Create Module Structure
1. Create all module files with empty implementations
2. Move structs and enums to `models.rs`
3. Update `lib.rs` with module declarations

### Phase 2: Extract Utility Functions
1. Move cryptographic functions to `crypto.rs`
2. Move cache functions to `cache.rs`  
3. Move general utilities to `utils.rs`

### Phase 3: Extract Core Functionality
1. Move download functions to respective download modules
2. Move archive functions to respective archive modules
3. Move installation functions to respective install modules

### Phase 4: Extract Business Logic
1. Move recipe processing to `recipe.rs`
2. Move runner logic to `runner.rs`
3. Move CLI logic to `cli.rs`

### Phase 5: Update Dependencies
1. Update all modules to use proper imports
2. Add necessary `pub` keywords for public interfaces
3. Update tests to use modular imports

### Phase 6: Testing and Validation
1. Run existing tests to ensure no regressions
2. Add unit tests for individual modules
3. Update integration tests if needed

## Benefits After Refactoring

1. **Maintainability**: Each module has a single responsibility
2. **Testability**: Individual functions can be unit tested in isolation
3. **Reusability**: Core functionality can be reused as a library
4. **Compilation**: Faster incremental compilation due to smaller modules
5. **Documentation**: Easier to document individual modules
6. **Contributors**: New contributors can focus on specific areas

## Dependencies Between Modules

```
cli.rs -> recipe.rs, runner.rs, install.rs
recipe.rs -> download.rs, archive.rs, crypto.rs, cache.rs
runner.rs -> download.rs, archive.rs, install.rs
download.rs -> crypto.rs, cache.rs, models.rs
archive.rs -> utils.rs, models.rs
install.rs -> archive.rs, utils.rs, models.rs
```

## Implementation Notes

- Keep the existing public API unchanged to maintain backward compatibility
- Use `pub(crate)` for internal module communication
- Consider using `pub use` statements in `lib.rs` for commonly used items
- Maintain the existing error handling patterns
- Preserve all existing functionality during the migration
