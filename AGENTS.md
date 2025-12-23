# Agent Instructions

## Before Committing

Always run these checks before creating a git commit:

1. `cargo fmt --check` - Ensure code is formatted
2. `cargo clippy --all-targets` - Ensure no clippy warnings

If either fails, fix the issues before committing.
