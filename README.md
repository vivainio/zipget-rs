# zipget-rs

A Rust clone of [zipget](https://github.com/vivainio/zipget) - a tool for downloading and extracting files with intelligent caching, now with enhanced GitHub releases support.

## Problem

You want to download and extract files from URLs or GitHub releases, with intelligent caching to avoid re-downloading, and the ability to keep your toolchain up-to-date automatically.

## Features

- **Intelligent Caching**: Files are cached using MD5 hash of the URL to avoid re-downloading
- **GitHub Releases Integration**: Download latest or specific tagged releases from GitHub repositories
- **Recipe-Based Batch Processing**: Process multiple downloads from JSON recipes
- **Version Management**: Automatically upgrade GitHub releases to latest versions
- **Mixed Sources**: Combine URL downloads and GitHub releases in a single recipe
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

# Save to specific directory
zipget github BurntSushi/ripgrep windows --output ./tools
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
            "saveAs": "./downloads/bat.zip"
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
  - **url**: Direct URL to download from
  - **github**: GitHub release specification
    - **repo**: Repository in "owner/repo" format
    - **binary**: Name pattern to match in release assets
    - **tag** (optional): Specific release tag (defaults to latest)
  - **unzipTo** (optional): Directory where the ZIP file should be extracted
  - **saveAs** (optional): Path where the downloaded file should be saved

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

## How It Works

1. **Caching**: Each URL is hashed using MD5, and the downloaded file is stored as `{hash}_{filename}` in a system temporary cache directory (`%TEMP%\zipget-cache` on Windows, `/tmp/zipget-cache` on Unix)
2. **Cache Check**: Before downloading, zipget checks if the file already exists in the cache directory
3. **GitHub API**: For GitHub releases, the tool queries the GitHub API to get download URLs
4. **Download**: If not cached, the file is downloaded and stored in the cache directory
5. **Extract**: If `unzipTo` is specified, the ZIP file is extracted to the target directory
6. **Save**: If `saveAs` is specified, the downloaded file is copied to the specified path

## Examples

### Basic Recipe Usage

Using the included `demo_recipe.json`:

```bash
zipget recipe demo_recipe.json
```

### GitHub Download

Download the latest ripgrep for Windows:

```bash
zipget github BurntSushi/ripgrep windows --output ./tools
```

### Recipe Upgrade

Keep all your tools up-to-date:

```bash
zipget recipe my_toolchain.json --upgrade
```

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
            "unzipTo": "./tools"
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

- `reqwest`: HTTP client for downloading files and GitHub API
- `serde`: JSON serialization/deserialization  
- `zip`: ZIP file extraction
- `md5`: URL hashing for cache keys
- `tokio`: Async runtime
- `anyhow`: Error handling
- `clap`: CLI argument parsing

## License

MIT License - same as the original zipget project. 