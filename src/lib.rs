use anyhow::Result;

// Public modules
pub mod models;
pub mod cli;
pub mod cache;
pub mod crypto;
pub mod utils;
pub mod download;
pub mod archive;
pub mod install;
pub mod recipe;
pub mod runner;

// Re-export commonly used types
pub use models::*;
pub use anyhow::{Context, Result as AnyhowResult};

// Common type aliases
pub type ZipgetResult<T> = Result<T>;
