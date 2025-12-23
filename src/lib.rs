use anyhow::Result;

// Public modules
pub mod archive;
pub mod cache;
pub mod cli;
pub mod crypto;
pub mod download;
pub mod install;
pub mod models;
pub mod recipe;
pub mod runner;
pub mod utils;
pub mod vars;

// Re-export commonly used types
pub use anyhow::{Context, Result as AnyhowResult};
pub use models::*;

// Common type aliases
pub type ZipgetResult<T> = Result<T>;
