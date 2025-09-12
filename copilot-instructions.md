# GitHub Copilot Project Instructions for zipget-rs (Rust)

## General Guidelines
- Follow idiomatic Rust style (use `rustfmt` for formatting).
- Prefer `Result` and error handling over panics.
- Use `clippy` to lint code and follow its suggestions unless there is a strong reason not to.
- Write clear, concise documentation for public functions and modules.
- Use descriptive variable and function names.
- Prefer immutable variables unless mutation is necessary.
- Use pattern matching and enums idiomatically.
- Avoid unsafe code unless absolutely necessary and document its use.
- Write unit tests for all new functionality in the `tests` or `test` directory.

## Pre-Commit Quality Checks
Before committing any changes, always run these required commands:

1. **Format code with rustfmt:**
   ```bash
   cargo fmt
   ```

2. **Run clippy for linting:**
   ```bash
   cargo clippy -- -D warnings
   ```

3. **Run tests to ensure nothing is broken:**
   ```bash
   cargo test
   ```

### Recommended Pre-Commit Workflow:
```bash
# Format code
cargo fmt

# Check for linting issues
cargo clippy -- -D warnings

# Run tests
cargo test

# If all checks pass, commit
git add .
git commit -m "Your commit message"
git push
```

## Project-Specific Notes
- Main entry point: `src/main.rs`
- Test files are in the `test/` directory.
- Use the `cache/` and `downloads/` directories for temporary and downloaded files, not for source code.
- Do not commit files in `target/`, `cache/`, or `downloads/`.

## Copilot Usage
- Use Copilot suggestions as a starting point, but always review and edit for correctness and idiomatic Rust.
- Do not accept large code blocks without understanding them.
- Prefer smaller, incremental completions.
- When in doubt, refer to the Rust documentation or the official book: https://doc.rust-lang.org/book/

## File/Directory Ignore Patterns
- Ignore completions for files in `target/`, `cache/`, `downloads/`, and `test-output/`.
- Do not generate code for binary or archive files (e.g., `.zip`, `.exe`).

## Example Copilot Prompts
- "Implement a function to extract a file from a zip archive."
- "Write a test for the download logic in `main.rs`."
- "Suggest a Rust enum for representing download status."

---

_This file provides Copilot and contributors with project-specific instructions and best practices. Update as needed._
