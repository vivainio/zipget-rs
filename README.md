# zipget-rs

A Rust clone of [zipget](https://github.com/vivainio/zipget) - a powerful tool for downloading and extracting files from multiple sources including URLs, GitHub releases, and **AWS S3 buckets**, with intelligent caching and advanced features.

## Problem

You want to download and extract files from multiple sources - public URLs, GitHub releases, and private S3 buckets - with intelligent caching to avoid re-downloading, and the ability to keep your toolchain up-to-date automatically. Whether you're managing development tools, deploying applications, or distributing private assets through S3, you need a unified solution that handles authentication, caching, and extraction seamlessly.

## Features

- **Intelligent Caching**: Files are cached using MD5 hash of the URL to avoid re-downloading
- **Multi-Format Archive Support**: Extract both ZIP and tar.gz (.tgz) archives automatically
- **GitHub Releases Integration**: Download latest or specific tagged releases from GitHub repositories
- **S3 Support**: Download files from AWS S3 buckets using `s3://` URLs with AWS CLI integration
- **Recipe-Based Batch Processing**: Process multiple downloads from JSON recipes
- **Version Management**: Automatically upgrade GitHub releases to latest versions
- **Mixed Sources**: Combine URL downloads, GitHub releases, and S3 downloads in a single recipe  
- **Flexible Output**: Extract to directories and/or save files with custom names
- **Direct Execution**: Download and run executables directly with the `run` command
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
# Download from HTTP
zipget recipe demo_recipe.json

# Download from GitHub releases  
zipget github sharkdp/bat windows --unzip-to ./tools
```

### S3 Quick Start
```bash
# 1. Install and configure AWS CLI
winget install Amazon.AWSCLI  # Windows
aws configure                 # Set up credentials

# 2. Create an S3 recipe
echo '{
    "fetch": [
        {
            "url": "s3://my-bucket/app.zip",
            "unzipTo": "./app"
        }
    ]
}' > s3-recipe.json

# 3. Download from S3
zipget recipe s3-recipe.json

# 4. Use different AWS profiles
zipget recipe s3-recipe.json --profile my-profile
```

## Commands

### Recipe Command

Process a recipe file to download and extract multiple packages:

```bash
# Process a recipe file
zipget recipe my_recipe.json

# Process recipe with specific AWS profile for S3 downloads
zipget recipe my_recipe.json --profile my-aws-profile

# Upgrade all GitHub releases in recipe to latest versions
zipget recipe my_recipe.json --upgrade
```

### GitHub Command

Download the latest release binary from a GitHub repository:

```bash
# Download latest release
zipget github sharkdp/bat windows

# Download specific tagged release
zipget github sharkdp/bat windows --tag v0.24.0

# Save to specific file path
zipget github BurntSushi/ripgrep windows --save-as ./tools/ripgrep.zip
```

### Run Command

Download and run an executable from a package with intelligent executable detection:

```bash
# Run a single executable from a GitHub release
zipget run BurntSushi/ripgrep windows -- --version

# Run a specific executable if multiple are found
zipget run sharkdp/bat windows --exe bat -- --help

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

Zipget uses JSON recipe files to define what to download and where to put it. Recipes support HTTP URLs, GitHub releases, and S3 buckets:

```json
{
    "fetch": [
        {
            "url": "https://example.com/some-file.zip",
            "unzipTo": "./downloads"
        },
        {
            "url": "s3://private-bucket/internal-tool.tar.gz",
            "profile": "company-profile",
            "unzipTo": "./tools",
            "files": "*.exe"
        },
        {
            "github": {
                "repo": "sharkdp/bat",
                "binary": "windows",
                "tag": "v0.24.0"
            },
            "unzipTo": "./tools",
            "saveAs": "./downloads/bat.zip",
            "files": "*.exe"
        },
        {
            "github": {
                "repo": "BurntSushi/ripgrep",
                "binary": "windows"
            },
            "saveAs": "./tools/ripgrep.zip"
        }
    ]
}
```

### Recipe Schema

- **fetch**: Array of items to download, each item can have:
  - **url**: Direct URL to download from (supports HTTP/HTTPS and S3 URLs)
  - **github**: GitHub release specification
    - **repo**: Repository in "owner/repo" format
    - **binary**: Name pattern to match in release assets
    - **tag** (optional): Specific release tag (defaults to latest)
  - **unzipTo** (optional): Directory where archives should be extracted (supports ZIP and tar.gz files)
  - **saveAs** (optional): Path where the downloaded file should be saved
  - **files** (optional): Glob pattern for files to extract from archives (extracts all if not specified)
  - **profile** (optional): AWS profile to use for S3 downloads (overrides global --profile)

## GitHub Integration

### Latest Releases

Download the latest release without specifying a tag:

```json
{
    "github": {
        "repo": "sharkdp/bat",
        "binary": "windows"
    }
}
```

### Specific Versions

Pin to a specific release tag:

```json
{
    "github": {
        "repo": "sharkdp/bat",
        "binary": "windows", 
        "tag": "v0.24.0"
    }
}
```

### Version Upgrading

Automatically update all GitHub releases to their latest versions:

```bash
zipget recipe my_recipe.json --upgrade
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

```json
{
    "fetch": [
        {
            "url": "s3://my-bucket/path/to/file.zip",
            "unzipTo": "./downloads"
        },
        {
            "url": "s3://private-bucket/releases/app-v1.2.3.tar.gz",
            "unzipTo": "./tools",
            "files": "*.exe"
        }
    ]
}
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
zipget recipe my_recipe.json --profile production-profile
```

**Per-item profile (overrides global profile):**
```json
{
    "fetch": [
        {
            "url": "s3://prod-bucket/app.zip",
            "profile": "production-profile",
            "unzipTo": "./app"
        },
        {
            "url": "s3://dev-bucket/test-data.zip", 
            "profile": "development-profile",
            "unzipTo": "./test"
        }
    ]
}
```

### Mixed Sources

Combine S3, HTTP, and GitHub sources in a single recipe:

```json
{
    "fetch": [
        {
            "url": "https://example.com/public-tool.zip",
            "unzipTo": "./tools"
        },
        {
            "url": "s3://private-bucket/internal-tool.zip",
            "profile": "company-profile",
            "unzipTo": "./tools"
        },
        {
            "github": {
                "repo": "user/repo",
                "binary": "windows"
            },
            "unzipTo": "./tools"
        }
    ]
}
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
zipget recipe your-s3-recipe.json
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
6. **Extract**: If `unzipTo` is specified, the archive is extracted to the target directory (auto-detects ZIP and tar.gz formats)
7. **Save**: If `saveAs` is specified, the downloaded file is copied to the specified path
8. **Run**: The `run` command additionally extracts to a temporary directory, finds executables, and executes them with provided arguments

## Examples

### Basic Recipe Usage

Using the included `demo_recipe.json`:

```bash
zipget recipe demo_recipe.json
```

### GitHub Download

Download the latest ripgrep for Windows:

```bash
zipget github BurntSushi/ripgrep windows --save-as ./tools/ripgrep.zip
```

### Recipe Upgrade

Keep all your tools up-to-date:

```bash
zipget recipe my_toolchain.json --upgrade
```

### Run Executable

Download and run tools directly without manual extraction:

```bash
# Run ripgrep to search for text (single executable, auto-detected)
zipget run BurntSushi/ripgrep windows -- --help

# Run bat to display a file (specify executable name)
zipget run sharkdp/bat windows --exe bat -- README.md

# Run a tool from a direct URL with arguments
zipget run https://example.com/mytool.zip --exe mytool -- --input data.txt --output result.txt
```

### S3 Downloads

Download files from S3 buckets using s3:// URLs:

**Basic S3 download:**
```json
{
    "fetch": [
        {
            "url": "s3://my-company-tools/releases/internal-tool.zip",
            "unzipTo": "./tools",
            "files": "*.exe"
        }
    ]
}
```

**Multi-environment S3 setup with profiles:**
```json
{
    "fetch": [
        {
            "url": "s3://production-releases/app-v2.1.0.tar.gz",
            "profile": "prod-profile",
            "unzipTo": "./prod-app",
            "tags": ["production"]
        },
        {
            "url": "s3://staging-releases/app-v2.2.0-beta.tar.gz", 
            "profile": "staging-profile",
            "unzipTo": "./staging-app",
            "tags": ["staging"]
        },
        {
            "url": "s3://dev-assets/test-data.zip",
            "profile": "dev-profile",
            "saveAs": "./test/data.zip",
            "tags": ["development"]
        }
    ]
}
```

**Enterprise deployment recipe:**
```bash
# Deploy to production with specific profile
zipget recipe enterprise-deploy.json --profile production-profile

# Deploy only staging components
zipget recipe enterprise-deploy.json staging
```

### Selective File Extraction

Use the `files` field to extract only specific files from archives using glob patterns:

```json
{
    "fetch": [
        {
            "url": "https://example.com/tools.zip",
            "unzipTo": "./tools",
            "files": "*.exe"
        },
        {
            "github": {
                "repo": "sharkdp/bat",
                "binary": "windows"
            },
            "unzipTo": "./tools",
            "files": "{bat.exe,LICENSE*}"
        }
    ]
}
```

Common glob patterns:
- `*.exe` - Extract only .exe files
- `*.{exe,dll}` - Extract .exe and .dll files  
- `bin/*` - Extract files in the bin/ directory
- `{LICENSE,README*}` - Extract LICENSE and README files

### Mixed Recipe Example

```json
{
    "fetch": [
        {
            "url": "https://github.com/vivainio/Modulize/releases/download/v2.1/Modulize.zip",
            "unzipTo": "./downloads"
        },
        {
            "github": {
                "repo": "sharkdp/bat",
                "binary": "windows"
            },
            "unzipTo": "./tools",
            "files": "*.exe"
        },
        {
            "github": {
                "repo": "BurntSushi/ripgrep",
                "binary": "windows",
                "tag": "14.1.0"
            },
            "saveAs": "./tools/ripgrep.zip"
        }
    ]
}
```

## Help

Get help for any command:

```bash
zipget --help
zipget recipe --help
zipget github --help
zipget run --help
```

## Dependencies

- `ureq`: HTTP client for downloading files and GitHub API
- `serde`: JSON serialization/deserialization  
- `zip`: ZIP file extraction
- `tar`: TAR archive extraction
- `flate2`: Gzip compression/decompression for tar.gz files
- `md5`: URL hashing for cache keys
- `anyhow`: Error handling
- `clap`: CLI argument parsing
- `glob-match`: Pattern matching for selective file extraction

## License

MIT License - same as the original zipget project. 