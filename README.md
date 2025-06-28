# zipget-rs

A Rust clone of [zipget](https://github.com/vivainio/zipget) - a tool for downloading and unzipping files with intelligent caching.

## Problem

You want to download and unzip a bunch of files somewhere, possibly getting them from offline cache if they were downloaded earlier.

## Features

- **Intelligent Caching**: Files are cached using MD5 hash of the URL to avoid re-downloading
- **Batch Processing**: Process multiple downloads from a single JSON recipe
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

## Usage

```bash
zipget recipe.json
```

## Recipe Format

Zipget uses JSON recipe files to define what to download and where to put it:

```json
{
    "config": {
        "archive": ["./cache"]
    },
    "fetch": [
        {
            "url": "https://example.com/some-file.zip",
            "unzipTo": "./downloads"
        },
        {
            "url": "https://example.com/another-file.zip",
            "unzipTo": "./downloads",
            "saveAs": "./downloads/renamed-file.zip"
        }
    ]
}
```

### Recipe Schema

- **config.archive**: Array of directories where downloaded files are cached
- **fetch**: Array of items to download
  - **url**: The URL to download from
  - **unzipTo** (optional): Directory where the ZIP file should be extracted
  - **saveAs** (optional): Path where the downloaded file should be saved

## How It Works

1. **Caching**: Each URL is hashed using MD5, and the downloaded file is stored as `{hash}_{filename}` in the archive directory
2. **Cache Check**: Before downloading, zipget checks if the file already exists in any archive directory
3. **Download**: If not cached, the file is downloaded and stored in the first archive directory
4. **Extract**: If `unzipTo` is specified, the ZIP file is extracted to the target directory
5. **Save**: If `saveAs` is specified, the downloaded file is copied to the specified path

## Example

Using the included `demo_recipe.json`:

```bash
cargo run -- demo_recipe.json
```

This will:
1. Create a `./cache` directory for cached downloads
2. Download and cache files with MD5-prefixed names
3. Extract ZIP contents to `./downloads`
4. Save a copy of the second file as `./downloads/hashibuild-master.zip`

## Dependencies

- `reqwest`: HTTP client for downloading files
- `serde`: JSON serialization/deserialization
- `zip`: ZIP file extraction
- `md5`: URL hashing for cache keys
- `tokio`: Async runtime
- `anyhow`: Error handling
- `clap`: CLI argument parsing

## License

MIT License - same as the original zipget project. 