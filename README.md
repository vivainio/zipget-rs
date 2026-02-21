# zipget-rs

[![Build and Publish](https://github.com/vivainio/zipget-rs/actions/workflows/build-and-publish.yml/badge.svg)](https://github.com/vivainio/zipget-rs/actions/workflows/build-and-publish.yml)

A tool for downloading and extracting files from URLs, GitHub releases, and S3 buckets, with caching and a TOML recipe format.

## Features

- **Caching**: Files are cached by URL hash to avoid re-downloading
- **Multi-Format Archive Support**: Extract both ZIP and tar.gz (.tgz) archives automatically
- **GitHub Releases Integration**: Download latest or specific tagged releases from GitHub repositories
- **S3 Support**: Download files from AWS S3 buckets using `s3://` URLs
- **Semantic TOML Recipes**: Process multiple downloads from TOML recipes with meaningful section names
- **Version Management**: Automatically upgrade GitHub releases to latest versions
- **Flexible Output**: Extract to directories and/or save files with custom names
- **Direct Execution**: Download and run executables directly with the `run` command
- **Cross-Platform Installation**: Install executables directly to `~/.local/bin` on any platform with `--no-shim`, or use Windows shims
- **Java JAR Support**: Download and create launchers for Java JAR applications
- **Cross-Platform**: Works on Windows, macOS, and Linux

## Installation

### Linux / macOS

```bash
sudo curl -fsSL https://github.com/vivainio/zipget-rs/releases/latest/download/zipget-linux-x64-musl -o /usr/local/bin/zipget && sudo chmod +x /usr/local/bin/zipget
```

For macOS ARM (Apple Silicon):
```bash
curl -fsSL https://github.com/vivainio/zipget-rs/releases/latest/download/zipget-macos-arm64 -o ~/.local/bin/zipget && chmod +x ~/.local/bin/zipget
```

### Windows (PowerShell)

```powershell
iwr https://github.com/vivainio/zipget-rs/releases/latest/download/zipget-windows-x64.exe -OutFile ~/.local/bin/zipget.exe
```

### Self-Update

Once installed, zipget can update itself:
```bash
zipget update
```

### GitHub Actions

```yaml
- name: Install zipget
  run: |
    curl -fsSL https://github.com/vivainio/zipget-rs/releases/latest/download/zipget-linux-x64-musl -o /usr/local/bin/zipget
    chmod +x /usr/local/bin/zipget

- name: Download tools
  run: zipget recipe tools.toml
```

For Windows runners:
```yaml
- name: Install zipget
  run: |
    Invoke-WebRequest -Uri "https://github.com/vivainio/zipget-rs/releases/latest/download/zipget-windows-x64.exe" -OutFile "$env:USERPROFILE\.local\bin\zipget.exe"
    echo "$env:USERPROFILE\.local\bin" | Out-File -FilePath $env:GITHUB_PATH -Append
```

For macOS runners (ARM):
```yaml
- name: Install zipget
  run: |
    curl -fsSL https://github.com/vivainio/zipget-rs/releases/latest/download/zipget-macos-arm64 -o /usr/local/bin/zipget
    chmod +x /usr/local/bin/zipget
```

### From Source

```bash
git clone https://github.com/vivainio/zipget-rs
cd zipget-rs
cargo build --release
```

The binary will be available at `target/release/zipget`.

## Quick Start

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

# Create launcher for a Java JAR
zipget shim ./myapp.jar
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
```

**Shims vs Direct Installation:**
- **Shims (Windows only)**: Creates wrapper executables that can handle different versions and provide additional functionality
- **Direct Installation (`--no-shim`)**: Copies executables directly to `~/.local/bin`, works on all platforms

### Shim Command

Create launchers/shims for executables or Java JAR files:

```bash
# Create a launcher for a JAR file
zipget shim ./plantuml.jar

# Create with a custom name
zipget shim ./plantuml.jar --name plantuml

# Create with Java options (for JARs)
zipget shim ./myapp.jar --java-opts="-Xmx1g -Xms256m"

# Create a shim for a native executable
zipget shim ./mytool
```

**How it works:**
- For JAR files: Creates a shell script (Unix) or batch file (Windows) that runs `java -jar`
- For executables: Creates a shell script wrapper (Unix) or Scoop-style shim (Windows)
- Launchers are created in `~/.local/bin`

**Generated JAR launcher (Unix):**
```bash
#!/bin/sh
exec java -Xmx1g -jar "/path/to/myapp.jar" "$@"
```

**Generated JAR launcher (Windows):**
```batch
@java -Xmx1g -jar "C:\path\to\myapp.jar" %*
```

### Recipe Command

Process a TOML recipe file to download and extract multiple packages:

```bash
# Process a TOML recipe file
zipget recipe my_recipe.toml

# Upgrade all GitHub releases in recipe to latest versions
zipget recipe my_recipe.toml --upgrade

# Process only specific items by their section names (tags)
zipget recipe my_recipe.toml ripgrep
```

### GitHub Command

Download the latest release binary from a GitHub repository:

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

Download and run an executable from a package:

```bash
# Run a single executable from a GitHub release
zipget run BurntSushi/ripgrep -- --version

# Run a specific executable if multiple are found
zipget run sharkdp/bat --exe bat -- --help

# Run from a direct URL
zipget run https://example.com/tool.zip --exe mytool -- arg1 arg2
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
[bat]
github = { repo = "sharkdp/bat", tag = "v0.24.0" }
unzip_to = "./tools"
save_as = "./downloads/bat.zip"
files = "*.exe"

[ripgrep]
github = { repo = "BurntSushi/ripgrep" }
save_as = "./tools/ripgrep.zip"

[public-tool]
url = "https://example.com/some-file.zip"
unzip_to = "./downloads"
```

### Recipe Schema

Each section represents a download item and can have:
- **url**: Direct URL to download from (supports HTTP/HTTPS, S3 URLs, and local file paths starting with `/` or `.`)
- **github**: GitHub release specification (inline table format)
  - **repo**: Repository in "owner/repo" format
  - **asset**: Name pattern to match in release assets (optional, auto-detected if not specified)
  - **tag**: Specific release tag (optional, defaults to latest)
- **unzip_to**: Directory where archives should be extracted (supports ZIP and tar.gz files)
- **save_as**: Path where the downloaded file should be saved
- **files**: Glob pattern for files to extract from archives (extracts all if not specified)
- **profile**: AWS profile to use for S3 downloads
- **executable**: Set to `true` to add executable permission to extracted files (Unix only)
- **install_exes**: List of executables or JAR files to install to `~/.local/bin` (supports glob patterns)
- **no_shim**: Set to `true` to copy executables directly instead of creating shims/launchers

## Java JAR Support

Zipget can download Java JAR applications and create launcher scripts for them.

### Installing JARs from Recipes

```toml
[plantuml]
github = { repo = "plantuml/plantuml", asset = "plantuml.jar" }
save_as = "./tools/plantuml.jar"
install_exes = ["plantuml.jar"]
```

This will:
1. Download `plantuml.jar` from the GitHub release
2. Save it to `./tools/plantuml.jar`
3. Create a launcher at `~/.local/bin/plantuml`

### Installing JARs with the Shim Command

```bash
curl -LO https://github.com/plantuml/plantuml/releases/latest/download/plantuml.jar
zipget shim ./plantuml.jar
# Now you can run: plantuml -version
```

### JARs with Custom Java Options

```bash
zipget shim ./memory-intensive-app.jar --java-opts="-Xmx4g -XX:+UseG1GC"
```

## GitHub Integration

### Latest Releases

```toml
[bat]
github = { repo = "sharkdp/bat" }
```

### Specific Versions

```toml
[bat]
github = { repo = "sharkdp/bat", tag = "v0.24.0" }
```

### Manual Asset Selection (Optional)

```toml
[bat]
github = { repo = "sharkdp/bat", asset = "windows-x86_64", tag = "v0.24.0" }
```

### Version Upgrading

```bash
zipget recipe my_recipe.toml --upgrade
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
4. **S3 Downloads**: For S3 URLs, the tool uses AWS CLI (`aws s3 cp`) to download files using your configured credentials
5. **Download**: If not cached, the file is downloaded and stored in the cache directory
6. **Extract**: If `unzip_to` is specified, the archive is extracted to the target directory (auto-detects ZIP and tar.gz formats)
7. **Save**: If `save_as` is specified, the downloaded file is copied to the specified path
8. **Run**: The `run` command additionally extracts to a temporary directory, finds executables, and executes them with provided arguments

## Selective File Extraction

Use the `files` field to extract only specific files from archives using glob patterns.

When `files` is specified, the directory structure is flattened — files are extracted directly to `unzip_to` without preserving subdirectories:

```toml
[ripgrep]
# Archive contains: ripgrep-15.1.0-x86_64-unknown-linux-musl/rg
# Result: ./tools/rg (flattened, not ./tools/ripgrep-15.1.0-.../rg)
github = { repo = "BurntSushi/ripgrep", asset = "*x86_64-unknown-linux-musl*" }
unzip_to = "./tools"
files = "*/rg"

[bat-windows]
github = { repo = "sharkdp/bat", asset = "windows" }
unzip_to = "./tools"
files = "{bat.exe,LICENSE*}"
```

Common glob patterns:
- `*.exe` - Extract only .exe files
- `*.{exe,dll}` - Extract .exe and .dll files
- `*/rg` - Extract `rg` binary from any subdirectory (flattened)
- `{LICENSE,README*}` - Extract LICENSE and README files

## Setting Executable Permissions (Unix)

Use the `executable` field to automatically set executable permissions on extracted files:

```toml
[my-scripts]
url = "/path/to/scripts.tar.gz"
unzip_to = "./bin"
files = "*.sh"
executable = true
```

Local file paths (starting with `/` or `.`) are also supported in the `url` field.

## License

MIT License
