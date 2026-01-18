// FFmpeg command execution and video processing

use crate::cli::Resolution;
use std::path::Path;
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

    /// Get the duration of a video file in seconds
    pub fn get_duration(&self, video_path: &Path) -> Result<f64, FFmpegError> {
        // Execute ffprobe to get video duration
        // Command: ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 <video>
        let output = Command::new("ffprobe")
            .arg("-v")
            .arg("error")
            .arg("-show_entries")
            .arg("format=duration")
            .arg("-of")
            .arg("default=noprint_wrappers=1:nokey=1")
            .arg(video_path)
            .output()
            .map_err(|e| FFmpegError::ExecutionFailed(format!("Failed to execute ffprobe: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FFmpegError::ExecutionFailed(format!(
                "ffprobe failed: {}",
                stderr
            )));
        }

        // Parse the output to f64
        let duration_str = String::from_utf8_lossy(&output.stdout);
        let duration_str = duration_str.trim();

        duration_str
            .parse::<f64>()
            .map_err(|e| FFmpegError::ParseError(format!(
                "Failed to parse duration '{}': {}",
                duration_str, e
            )))
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
    use std::path::PathBuf;

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

    #[test]
    fn test_get_duration_with_nonexistent_file() {
        // Test error handling when video file doesn't exist
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let nonexistent_path = PathBuf::from("nonexistent_video.mp4");
        
        let result = executor.get_duration(&nonexistent_path);
        
        // Should return an error
        assert!(result.is_err());
        
        // Verify it's an ExecutionFailed error
        match result {
            Err(FFmpegError::ExecutionFailed(_)) => {
                // Expected error type
            }
            _ => panic!("Expected ExecutionFailed error for nonexistent file"),
        }
    }

    #[test]
    fn test_ffmpeg_error_variants() {
        // Test that all error variants can be created and have correct messages
        let execution_error = FFmpegError::ExecutionFailed("test error".to_string());
        assert!(execution_error.to_string().contains("test error"));
        
        let parse_error = FFmpegError::ParseError("invalid format".to_string());
        assert!(parse_error.to_string().contains("invalid format"));
        
        let no_audio_error = FFmpegError::NoAudioTrack;
        assert_eq!(no_audio_error.to_string(), "Video has no audio track");
    }
}
