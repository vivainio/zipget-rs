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
