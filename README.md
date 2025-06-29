# zipget-rs

[![Integration Tests](https://github.com/vivainio/zipget-rs/actions/workflows/integration-tests.yml/badge.svg)](https://github.com/vivainio/zipget-rs/actions/workflows/integration-tests.yml)

A Rust clone of [zipget](https://github.com/vivainio/zipget) - a powerful tool for downloading and extracting files from multiple sources including URLs, GitHub releases, and **AWS S3 buckets**, with intelligent caching and advanced features.

## Problem

You want to download and extract files from multiple sources - public URLs, GitHub releases, and private S3 buckets - with intelligent caching to avoid re-downloading, and the ability to keep your toolchain up-to-date automatically. Whether you're managing development tools, deploying applications, or distributing private assets through S3, you need a unified solution that handles authentication, caching, and extraction seamlessly.

## Features

- **Intelligent Caching**: Files are cached using MD5 hash of the URL to avoid re-downloading
- **Multi-Format Archive Support**: Extract both ZIP and tar.gz (.tgz) archives automatically
- **GitHub Releases Integration**: Download latest or specific tagged releases from GitHub repositories
- **S3 Support**: Download files from AWS S3 buckets using `s3://` URLs with AWS CLI integration
- **Semantic TOML Recipes**: Process multiple downloads from TOML recipes with meaningful section names
- **Version Management**: Automatically upgrade GitHub releases to latest versions
- **Mixed Sources**: Combine URL downloads, GitHub releases, and S3 downloads in a single recipe  
- **Flexible Output**: Extract to directories and/or save files with custom names
- **Direct Execution**: Download and run executables directly with the `run` command
- **Cross-Platform Installation**: Install executables directly to `~/.local/bin` on any platform with `--no-shim`, or use Windows shims
- **Cross-Platform**: Works on Windows, macOS, and Linux

## Installation

### From Source

```bash
git clone <this-repository>
cd zipget-rs
cargo build --release
```

The binary will be available at `target/release/zipget`.

## Quick Start

### Basic Usage
```bash
# Download from TOML recipe
zipget recipe demo_recipe.toml

# Download from GitHub releases (auto-detects best binary for your platform)
zipget github sharkdp/bat --unzip-to ./tools

# Download and run executables directly
zipget run BurntSushi/ripgrep -- --version

# Install tools with shims (Windows only)
zipget install google/go-jsonnet

# Install tools directly (cross-platform)
zipget install google/go-jsonnet --no-shim
```

### S3 Quick Start
```bash
# 1. Install and configure AWS CLI
winget install Amazon.AWSCLI  # Windows
aws configure                 # Set up credentials

# 2. Create an S3 recipe
echo '[my-app]
url = "s3://my-bucket/app.zip"
unzip_to = "./app"' > s3-recipe.toml

# 3. Download from S3
zipget recipe s3-recipe.toml

# 4. Use different AWS profiles
zipget recipe s3-recipe.toml --profile my-profile
```

### Install Quick Start (Windows)
```bash
# 1. Install a single tool with automatic binary detection (creates shims)
zipget install google/go-jsonnet

# 2. Install a specific executable from a multi-binary package
zipget install google/go-jsonnet --exe jsonnet

# 3. Add ~/.local/bin to your PATH (one-time setup)
$env:PATH += ";$env:USERPROFILE\.local\bin"

# 4. Use the installed tools directly
jsonnet --version
jsonnetfmt --help
```

### Install Quick Start (Cross-Platform)
```bash
# 1. Install directly to ~/.local/bin without shims (works on all platforms)
zipget install google/go-jsonnet --no-shim

# 2. Install specific executable directly
zipget install google/go-jsonnet --exe jsonnet --no-shim

# 3. Add ~/.local/bin to your PATH (one-time setup)
export PATH="$HOME/.local/bin:$PATH"  # Linux/macOS
$env:PATH += ";$env:USERPROFILE\.local\bin"  # Windows PowerShell

# 4. Use the installed tools directly
jsonnet --version
jsonnetfmt --help
```

## Commands

### Install Command

Install executables from packages to your local system:

```bash
# Install with shims (Windows only) - creates shims in ~/.local/bin
zipget install sharkdp/bat

# Install directly to ~/.local/bin (cross-platform)
zipget install sharkdp/bat --no-shim

# Install specific executable from multi-binary package
zipget install google/go-jsonnet --exe jsonnet --no-shim

# Install from specific release tag
zipget install sharkdp/bat --tag v0.24.0 --no-shim

# Install from direct URL
zipget install https://example.com/tool.zip --no-shim

# Install from S3 with AWS profile
zipget install s3://my-bucket/app.zip --profile my-profile --no-shim
```

**Shims vs Direct Installation:**
- **Shims (Windows only)**: Creates wrapper executables that can handle different versions and provide additional functionality
- **Direct Installation (`--no-shim`)**: Copies executables directly to `~/.local/bin`, works on all platforms

### Recipe Command

Process a TOML recipe file to download and extract multiple packages:

```bash
# Process a TOML recipe file
zipget recipe my_recipe.toml

# Process recipe with specific AWS profile for S3 downloads
zipget recipe my_recipe.toml --profile my-aws-profile

# Upgrade all GitHub releases in recipe to latest versions
zipget recipe my_recipe.toml --upgrade

# Process only specific items by their section names (tags)
zipget recipe my_recipe.toml ripgrep
```

### GitHub Command

Download the latest release binary from a GitHub repository with intelligent asset detection:

```bash
# Download latest release (auto-detects best binary for your platform)
zipget github sharkdp/bat

# Download specific tagged release
zipget github sharkdp/bat --tag v0.24.0

# Save to specific file path
zipget github BurntSushi/ripgrep --save-as ./tools/ripgrep.zip

# Manually specify asset if needed (rarely required)
zipget github sharkdp/bat --asset windows-x86_64
```

### Run Command

Download and run an executable from a package with intelligent executable detection:

```bash
# Run a single executable from a GitHub release (auto-detects best binary)
zipget run BurntSushi/ripgrep -- --version

# Run a specific executable if multiple are found
zipget run sharkdp/bat --exe bat -- --help

# Run from a direct URL
zipget run https://example.com/tool.zip --exe mytool -- arg1 arg2

# Run from S3 with AWS profile
zipget run s3://my-bucket/app.zip --profile my-profile --exe app -- --config config.json
```

The `run` command:
- Downloads and caches the package (honoring existing cache)
- Extracts the package to a temporary directory
- Automatically finds executable files in the extracted content
- If only one executable is found, runs it directly
- If multiple executables are found, prompts you to specify which one using `--exe`
- Passes all arguments after `--` to the executable
- Cleans up temporary files after execution

## Recipe Format

Zipget uses TOML recipe files with semantic section names. Each section name becomes an implicit tag for that download item:

```toml
# Each section name becomes a meaningful tag
[bat]
github = { repo = "sharkdp/bat", tag = "v0.24.0" }
unzip_to = "./tools"
save_as = "./downloads/bat.zip"
files = "*.exe"

[ripgrep]
github = { repo = "BurntSushi/ripgrep" }
save_as = "./tools/ripgrep.zip"

[company-app]
url = "s3://private-bucket/internal-tool.tar.gz"
profile = "company-profile"
unzip_to = "./tools"
files = "*.exe"

[public-tool]
url = "https://example.com/some-file.zip"
unzip_to = "./downloads"
```

### Recipe Schema

Each section represents a download item and can have:
- **url**: Direct URL to download from (supports HTTP/HTTPS and S3 URLs)
- **github**: GitHub release specification (inline table format)
  - **repo**: Repository in "owner/repo" format
  - **asset**: Name pattern to match in release assets (optional, auto-detected if not specified)
  - **tag**: Specific release tag (optional, defaults to latest)
- **unzip_to**: Directory where archives should be extracted (supports ZIP and tar.gz files)
- **save_as**: Path where the downloaded file should be saved
- **files**: Glob pattern for files to extract from archives (extracts all if not specified)
- **profile**: AWS profile to use for S3 downloads (overrides global --profile)

## GitHub Integration

### Latest Releases

Download the latest release with automatic asset detection:

```toml
[bat]
github = { repo = "sharkdp/bat" }
```

### Specific Versions

Pin to a specific release tag:

```toml
[bat]
github = { repo = "sharkdp/bat", tag = "v0.24.0" }
```

### Manual Asset Selection (Optional)

Specify a particular asset if automatic detection doesn't meet your needs:

```toml
[bat]
github = { repo = "sharkdp/bat", asset = "windows-x86_64", tag = "v0.24.0" }
```

### Version Upgrading

Automatically update all GitHub releases to their latest versions:

```bash
zipget recipe my_recipe.toml --upgrade
```

This will:
- Check the latest release for each GitHub repository
- Update tags to the latest version
- Save the updated recipe file
- Show which versions were upgraded

## S3 Integration

### Prerequisites

S3 support requires AWS CLI to be installed and configured:

```bash
# Install AWS CLI (example for Windows)
winget install Amazon.AWSCLI

# Configure AWS credentials
aws configure
```

### S3 URLs

Use standard S3 URL format in recipes:

```toml
[my-app]
url = "s3://my-bucket/path/to/file.zip"
unzip_to = "./downloads"

[company-tool]
url = "s3://private-bucket/releases/app-v1.2.3.tar.gz"
unzip_to = "./tools"
files = "*.exe"
```

### Authentication

S3 downloads use your configured AWS CLI credentials and support:
- AWS credentials file (`~/.aws/credentials`)
- Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`)
- IAM roles (for EC2/ECS environments)
- AWS profiles (`aws configure --profile myprofile`)

### AWS Profile Support

You can specify AWS profiles in two ways:

**Global profile (applies to all S3 downloads in recipe):**
```bash
zipget recipe my_recipe.toml --profile production-profile
```

**Per-item profile (overrides global profile):**
```toml
[prod-app]
url = "s3://prod-bucket/app.zip"
profile = "production-profile"
unzip_to = "./app"

[dev-data]
url = "s3://dev-bucket/test-data.zip"
profile = "development-profile"
unzip_to = "./test"
```

### Mixed Sources

Combine S3, HTTP, and GitHub sources in a single recipe:

```toml
[public-tool]
url = "https://example.com/public-tool.zip"
unzip_to = "./tools"

[private-tool]
url = "s3://private-bucket/internal-tool.zip"
profile = "company-profile"
unzip_to = "./tools"

[ripgrep]
github = { repo = "BurntSushi/ripgrep", asset = "windows" }
unzip_to = "./tools"
```

### S3 Troubleshooting

**Common S3 Issues and Solutions:**

```bash
# Issue: "NoCredentialsError" or "Unable to locate credentials"
# Solution: Configure AWS credentials
aws configure

# Issue: "The config profile (profile-name) could not be found"
# Solution: List available profiles and configure missing one
aws configure list-profiles
aws configure --profile profile-name

# Issue: "NoSuchBucket" 
# Solution: Check bucket name and permissions
aws s3 ls s3://your-bucket-name

# Issue: "AccessDenied"
# Solution: Verify IAM permissions for s3:GetObject action
aws iam get-user
```

**Testing S3 Access:**
```bash
# Test S3 access with AWS CLI first
aws s3 ls s3://your-bucket/
aws s3 cp s3://your-bucket/test-file.zip ./test-download.zip

# Then use zipget-rs
zipget recipe your-s3-recipe.toml
```

### S3 Use Cases

**Enterprise Software Distribution:**
- Store private application releases in S3 buckets
- Use different profiles for production, staging, and development environments
- Implement secure software deployment pipelines

**CI/CD Pipeline Assets:**
- Download build artifacts from S3 during deployment
- Cache frequently used dependencies and tools
- Distribute configuration files and secrets securely

**Multi-Cloud Development:**
- Access assets from different cloud providers (AWS, DigitalOcean, MinIO)
- Maintain consistent tooling across hybrid environments
- Support air-gapped deployments with private S3-compatible storage

**Development Team Workflows:**
- Share development tools and SDKs through private buckets
- Distribute large binary assets that shouldn't be in git repositories
- Manage different tool versions per team or project

## How It Works

1. **Caching**: Each URL is hashed using MD5, and the downloaded file is stored as `{hash}_{filename}` in a system temporary cache directory (`%TEMP%\zipget-cache` on Windows, `/tmp/zipget-cache` on Unix)
2. **Cache Check**: Before downloading, zipget checks if the file already exists in the cache directory
3. **GitHub API**: For GitHub releases, the tool queries the GitHub API to get download URLs
4. **S3 Downloads**: For S3 URLs, the tool uses AWS CLI (`aws s3 cp`) to download files using your configured credentials
5. **Download**: If not cached, the file is downloaded and stored in the cache directory
6. **Extract**: If `unzip_to` is specified, the archive is extracted to the target directory (auto-detects ZIP and tar.gz formats)
7. **Save**: If `save_as` is specified, the downloaded file is copied to the specified path
8. **Run**: The `run` command additionally extracts to a temporary directory, finds executables, and executes them with provided arguments

## Examples

### Basic Recipe Usage

Using the included `demo_recipe.toml`:

```bash
zipget recipe demo_recipe.toml
```

### GitHub Download

Download the latest ripgrep with automatic platform detection:

```bash
zipget github BurntSushi/ripgrep --save-as ./tools/ripgrep.zip
```

### Recipe Upgrade

Keep all your tools up-to-date:

```bash
zipget recipe my_toolchain.toml --upgrade
```

### Run Executable

Download and run tools directly with intelligent platform and executable detection:

```bash
# Run ripgrep to search for text (auto-detects best binary and executable)
zipget run BurntSushi/ripgrep -- --help

# Run bat to display a file (specify executable name if multiple found)
zipget run sharkdp/bat --exe bat -- README.md

# Run a tool from a direct URL with arguments
zipget run https://example.com/mytool.zip --exe mytool -- --input data.txt --output result.txt
```

### S3 Downloads

Download files from S3 buckets using s3:// URLs:

**Basic S3 download:**
```toml
[company-tool]
url = "s3://my-company-tools/releases/internal-tool.zip"
unzip_to = "./tools"
files = "*.exe"
```

**Multi-environment S3 setup with profiles:**
```toml
[prod-app]
url = "s3://production-releases/app-v2.1.0.tar.gz"
profile = "prod-profile"
unzip_to = "./prod-app"

[staging-app]
url = "s3://staging-releases/app-v2.2.0-beta.tar.gz"
profile = "staging-profile"
unzip_to = "./staging-app"

[dev-data]
url = "s3://dev-assets/test-data.zip"
profile = "dev-profile"
save_as = "./test/data.zip"
```

**Enterprise deployment recipe:**
```bash
# Deploy to production with specific profile
zipget recipe enterprise-deploy.toml --profile production-profile

# Deploy only specific components by tag
zipget recipe enterprise-deploy.toml staging-app
```

### Selective File Extraction

Use the `files` field to extract only specific files from archives using glob patterns:

```toml
[tools]
url = "https://example.com/tools.zip"
unzip_to = "./tools"
files = "*.exe"

[bat-windows]
github = { repo = "sharkdp/bat", asset = "windows" }
unzip_to = "./tools"
files = "{bat.exe,LICENSE*}"
```

Common glob patterns:
- `*.exe` - Extract only .exe files
- `*.{exe,dll}` - Extract .exe and .dll files  
- `bin/*` - Extract files in the bin/ directory
- `{LICENSE,README*}` - Extract LICENSE and README files

### Complete Recipe Example

```toml
# Complete example showing all features
[modulize]
url = "https://github.com/vivainio/Modulize/releases/download/v2.1/Modulize.zip"
unzip_to = "./downloads"

[bat]
github = { repo = "sharkdp/bat" }
unzip_to = "./tools"
files = "*.exe"

[ripgrep-specific]
github = { repo = "BurntSushi/ripgrep", tag = "14.1.0" }
save_as = "./tools/ripgrep.zip"

[internal-tool]
url = "s3://company-bucket/internal-tool.tar.gz"
profile = "company-profile"
unzip_to = "./tools"
files = "*.exe"
```

## Help

Get help for any command:

```bash
zipget --help
zipget recipe --help
zipget github --help
zipget run --help
zipget install --help
```

## Dependencies

- `ureq`: HTTP client for downloading files and GitHub API
- `serde`: Serialization/deserialization framework
- `toml`: TOML parsing and serialization
- `zip`: ZIP file extraction
- `tar`: TAR archive extraction
- `flate2`: Gzip compression/decompression for tar.gz files
- `md5`: URL hashing for cache keys
- `anyhow`: Error handling
- `clap`: CLI argument parsing
- `glob-match`: Pattern matching for selective file extraction

## License

MIT License - same as the original zipget project. 