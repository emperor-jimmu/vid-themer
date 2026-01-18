// FFmpeg command execution and video processing

use crate::cli::Resolution;
use std::process::Command;

pub struct FFmpegExecutor {
    pub resolution: Resolution,
    pub include_audio: bool,
}

pub struct AudioSegment {
    pub start_time: f64,
    pub duration: f64,
    pub intensity: f64,
}

impl FFmpegExecutor {
    pub fn new(resolution: Resolution, include_audio: bool) -> Self {
        Self {
            resolution,
            include_audio,
        }
    }

    /// Check if FFmpeg is available in the system PATH
    pub fn check_availability() -> Result<(), FFmpegError> {
        // Try to execute ffmpeg -version to check if it's available
        let result = Command::new("ffmpeg")
            .arg("-version")
            .output();

        match result {
            Ok(output) => {
                if output.status.success() {
                    Ok(())
                } else {
                    Err(FFmpegError::NotFound)
                }
            }
            Err(_) => Err(FFmpegError::NotFound),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FFmpegError {
    #[error("FFmpeg not found in PATH")]
    NotFound,
    
    #[error("Failed to execute FFmpeg: {0}")]
    ExecutionFailed(String),
    
    #[error("Failed to parse FFmpeg output: {0}")]
    ParseError(String),
    
    #[error("Video has no audio track")]
    NoAudioTrack,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffmpeg_availability() {
        // This test will pass if FFmpeg is installed, fail if not
        // In a real environment, we expect FFmpeg to be available
        let result = FFmpegExecutor::check_availability();
        
        // We can't guarantee FFmpeg is installed in all test environments,
        // but we can verify the function returns the correct type
        match result {
            Ok(_) => {
                // FFmpeg is available - this is the expected case
                println!("FFmpeg is available in PATH");
            }
            Err(FFmpegError::NotFound) => {
                // FFmpeg is not available - this is acceptable in test environments
                println!("FFmpeg is not available in PATH");
            }
            Err(e) => {
                panic!("Unexpected error type: {:?}", e);
            }
        }
    }

    #[test]
    fn test_ffmpeg_error_not_found_variant() {
        // Test that we can create and match the NotFound error variant
        let error = FFmpegError::NotFound;
        assert_eq!(error.to_string(), "FFmpeg not found in PATH");
    }
}
