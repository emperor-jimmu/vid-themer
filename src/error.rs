// Top-level application error types

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Directory not found: {0}")]
    DirectoryNotFound(PathBuf),
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("Failed to scan directory: {0}")]
    DirectoryScanFailed(String),
}
