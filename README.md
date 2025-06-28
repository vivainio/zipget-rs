# zipget-rs

A Rust clone of [zipget](https://github.com/vivainio/zipget) - a tool for downloading and extracting files with intelligent caching, now with enhanced GitHub releases support and tar.gz extraction.

## Problem

You want to download and extract files from URLs or GitHub releases, with intelligent caching to avoid re-downloading, and the ability to keep your toolchain up-to-date automatically.

## Features

- **Intelligent Caching**: Files are cached using MD5 hash of the URL to avoid re-downloading
- **Multi-Format Archive Support**: Extract both ZIP and tar.gz (.tgz) archives automatically
- **GitHub Releases Integration**: Download latest or specific tagged releases from GitHub repositories
- **S3 Support**: Download files from AWS S3 buckets using `s3://` URLs with AWS CLI integration
- **Recipe-Based Batch Processing**: Process multiple downloads from JSON recipes
- **Version Management**: Automatically upgrade GitHub releases to latest versions
- **Mixed Sources**: Combine URL downloads, GitHub releases, and S3 downloads in a single recipe  
- **Flexible Output**: Extract to directories and/or save files with custom names
- **Cross-Platform**: Works on Windows, macOS, and Linux

## Installation

### From Source

```bash
git clone <this-repository>
cd zipget-rs
cargo build --release
```

The binary will be available at `target/release/zipget`.

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

## Recipe Format

Zipget uses JSON recipe files to define what to download and where to put it. Recipes support both direct URLs and GitHub releases:

```json
{
    "fetch": [
        {
            "url": "https://example.com/some-file.zip",
            "unzipTo": "./downloads"
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

## How It Works

1. **Caching**: Each URL is hashed using MD5, and the downloaded file is stored as `{hash}_{filename}` in a system temporary cache directory (`%TEMP%\zipget-cache` on Windows, `/tmp/zipget-cache` on Unix)
2. **Cache Check**: Before downloading, zipget checks if the file already exists in the cache directory
3. **GitHub API**: For GitHub releases, the tool queries the GitHub API to get download URLs
4. **S3 Downloads**: For S3 URLs, the tool uses AWS CLI (`aws s3 cp`) to download files using your configured credentials
5. **Download**: If not cached, the file is downloaded and stored in the cache directory
6. **Extract**: If `unzipTo` is specified, the archive is extracted to the target directory (auto-detects ZIP and tar.gz formats)
7. **Save**: If `saveAs` is specified, the downloaded file is copied to the specified path

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

### S3 Download

Download files from S3 buckets using s3:// URLs:

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