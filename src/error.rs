// Top-level application error types

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Directory not found: {0}")]
    DirectoryNotFound(PathBuf),
    
    #[error("FFmpeg not found in PATH")]
    FFmpegNotFound,
    
    #[error("Scan error: {0}")]
    ScanError(String),
    
    #[error("Process error: {0}")]
    ProcessError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("Failed to scan directory: {0}")]
    DirectoryScanFailed(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),
}
