// Video processing pipeline coordination

use crate::scanner::VideoFile;
use std::path::PathBuf;

pub struct VideoProcessor {
    // Will be populated in later tasks
}

pub struct ProcessResult {
    pub video_path: PathBuf,
    pub output_path: PathBuf,
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("Failed to process video: {0}")]
    ProcessingFailed(String),
}
