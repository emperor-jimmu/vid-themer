// Video processing pipeline coordination

use crate::ffmpeg::FFmpegExecutor;
use crate::scanner::VideoFile;
use crate::selector::ClipSelector;
use std::path::PathBuf;

pub struct VideoProcessor {
    selector: Box<dyn ClipSelector>,
    ffmpeg: FFmpegExecutor,
}

impl VideoProcessor {
    pub fn new(selector: Box<dyn ClipSelector>, ffmpeg: FFmpegExecutor) -> Self {
        Self { selector, ffmpeg }
    }

    /// Process a video file: detect duration, select segment, create output directory, and extract clip
    /// Returns ProcessResult with success/failure status
    /// Handles errors gracefully by logging and continuing
    pub fn process_video(&self, video: &VideoFile) -> ProcessResult {
        let video_path = video.path.clone();
        
        // Step 1: Get video duration
        let duration = match self.ffmpeg.get_duration(&video.path) {
            Ok(d) => d,
            Err(e) => {
                return ProcessResult {
                    video_path,
                    output_path: PathBuf::new(),
                    success: false,
                    error_message: Some(format!("Failed to get video duration: {}", e)),
                };
            }
        };
        
        // Step 2: Select segment using ClipSelector strategy
        let time_range = match self.selector.select_segment(&video.path, duration) {
            Ok(tr) => tr,
            Err(e) => {
                return ProcessResult {
                    video_path,
                    output_path: PathBuf::new(),
                    success: false,
                    error_message: Some(format!("Failed to select clip segment: {}", e)),
                };
            }
        };
        
        // Step 3: Create output directory
        let output_path = match self.create_output_directory(video) {
            Ok(path) => path,
            Err(e) => {
                return ProcessResult {
                    video_path,
                    output_path: PathBuf::new(),
                    success: false,
                    error_message: Some(format!("Failed to create output directory: {}", e)),
                };
            }
        };
        
        // Step 4: Extract clip using FFmpegExecutor
        match self.ffmpeg.extract_clip(&video.path, &time_range, &output_path) {
            Ok(_) => ProcessResult {
                video_path,
                output_path,
                success: true,
                error_message: None,
            },
            Err(e) => ProcessResult {
                video_path,
                output_path: output_path.clone(),
                success: false,
                error_message: Some(format!("Failed to extract clip: {}", e)),
            },
        }
    }

    /// Create the backdrops subdirectory and return the full output path
    /// Returns the path to backdrops/backdrop.mp4 relative to the video's parent directory
    fn create_output_directory(&self, video: &VideoFile) -> Result<PathBuf, ProcessError> {
        // Create backdrops subdirectory in video's parent directory
        let backdrops_dir = video.parent_dir.join("backdrops");
        
        std::fs::create_dir_all(&backdrops_dir).map_err(|e| {
            ProcessError::OutputDirectoryCreationFailed(format!(
                "Failed to create directory {:?}: {}",
                backdrops_dir, e
            ))
        })?;
        
        // Return full output path (backdrops/backdrop.mp4)
        Ok(backdrops_dir.join("backdrop.mp4"))
    }
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
    
    #[error("Failed to get video duration: {0}")]
    DurationDetectionFailed(String),
    
    #[error("Failed to select clip segment: {0}")]
    SegmentSelectionFailed(String),
    
    #[error("Failed to create output directory: {0}")]
    OutputDirectoryCreationFailed(String),
    
    #[error("Failed to extract clip: {0}")]
    ClipExtractionFailed(String),
}
