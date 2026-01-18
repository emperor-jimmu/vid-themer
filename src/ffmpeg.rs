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

    /// Get the video resolution (width, height) of a video file
    pub fn get_video_resolution(&self, video_path: &Path) -> Result<(u32, u32), FFmpegError> {
        // Execute ffprobe to get video width and height
        // Command: ffprobe -v error -select_streams v:0 -show_entries stream=width,height -of csv=s=x:p=0 <video>
        let output = Command::new("ffprobe")
            .arg("-v")
            .arg("error")
            .arg("-select_streams")
            .arg("v:0")
            .arg("-show_entries")
            .arg("stream=width,height")
            .arg("-of")
            .arg("csv=s=x:p=0")
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

        // Parse the output to (u32, u32)
        // Expected format: "1920x1080"
        let resolution_str = String::from_utf8_lossy(&output.stdout);
        let resolution_str = resolution_str.trim();

        // Split by 'x' to get width and height
        let parts: Vec<&str> = resolution_str.split('x').collect();
        if parts.len() != 2 {
            return Err(FFmpegError::ParseError(format!(
                "Invalid resolution format '{}', expected 'WIDTHxHEIGHT'",
                resolution_str
            )));
        }

        let width = parts[0]
            .parse::<u32>()
            .map_err(|e| FFmpegError::ParseError(format!(
                "Failed to parse width '{}': {}",
                parts[0], e
            )))?;

        let height = parts[1]
            .parse::<u32>()
            .map_err(|e| FFmpegError::ParseError(format!(
                "Failed to parse height '{}': {}",
                parts[1], e
            )))?;

        Ok((width, height))
    }

    /// Calculate the scale filter for FFmpeg based on target resolution
    /// Returns None if source resolution is smaller than target (no upscaling)
    /// Returns Some(filter_string) with letterboxing if scaling is needed
    pub fn calculate_scale_filter(&self, source_resolution: (u32, u32)) -> Option<String> {
        let (source_width, source_height) = source_resolution;
        
        // Determine target resolution based on configuration
        let (target_width, target_height) = match self.resolution {
            Resolution::Hd720 => (1280u32, 720u32),
            Resolution::Hd1080 => (1920u32, 1080u32),
        };

        // No upscaling: if source is smaller than target, return None
        if source_width <= target_width && source_height <= target_height {
            return None;
        }

        // Generate scale filter with letterboxing
        // scale=W:H:force_original_aspect_ratio=decrease,pad=W:H:(ow-iw)/2:(oh-ih)/2
        // This scales down to fit within target dimensions while maintaining aspect ratio,
        // then pads with black bars to reach exact target dimensions
        let filter = format!(
            "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2",
            target_width, target_height, target_width, target_height
        );

        Some(filter)
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

    #[test]
    fn test_get_video_resolution_with_nonexistent_file() {
        // Test error handling when video file doesn't exist
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let nonexistent_path = PathBuf::from("nonexistent_video.mp4");
        
        let result = executor.get_video_resolution(&nonexistent_path);
        
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
    fn test_get_video_resolution_with_invalid_file() {
        // Test error handling when file is not a valid video
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        
        // Create a temporary invalid file (text file pretending to be video)
        use std::fs;
        use std::env;
        
        let temp_dir = env::temp_dir();
        let invalid_video_path = temp_dir.join("invalid_video_test.mp4");
        
        // Write some non-video content
        fs::write(&invalid_video_path, b"This is not a video file").ok();
        
        let result = executor.get_video_resolution(&invalid_video_path);
        
        // Clean up
        fs::remove_file(&invalid_video_path).ok();
        
        // Should return an error (either ExecutionFailed or ParseError)
        assert!(result.is_err());
    }

    // Tests for calculate_scale_filter

    #[test]
    fn test_calculate_scale_filter_no_upscaling_smaller_source() {
        // Test that no scaling is applied when source is smaller than target
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        
        // Source: 1280x720 (smaller than 1920x1080)
        let result = executor.calculate_scale_filter((1280, 720));
        
        // Should return None (no upscaling)
        assert_eq!(result, None);
    }

    #[test]
    fn test_calculate_scale_filter_no_upscaling_equal_source() {
        // Test that no scaling is applied when source equals target
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        
        // Source: 1920x1080 (equal to target)
        let result = executor.calculate_scale_filter((1920, 1080));
        
        // Should return None (no upscaling)
        assert_eq!(result, None);
    }

    #[test]
    fn test_calculate_scale_filter_downscaling_1080p() {
        // Test that scaling is applied when source is larger than target (1080p)
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        
        // Source: 3840x2160 (4K, larger than 1920x1080)
        let result = executor.calculate_scale_filter((3840, 2160));
        
        // Should return scale filter with letterboxing
        assert!(result.is_some());
        let filter = result.unwrap();
        assert!(filter.contains("scale=1920:1080"));
        assert!(filter.contains("force_original_aspect_ratio=decrease"));
        assert!(filter.contains("pad=1920:1080"));
        assert!(filter.contains("(ow-iw)/2:(oh-ih)/2"));
    }

    #[test]
    fn test_calculate_scale_filter_downscaling_720p() {
        // Test that scaling is applied when source is larger than target (720p)
        let executor = FFmpegExecutor::new(Resolution::Hd720, true);
        
        // Source: 1920x1080 (larger than 1280x720)
        let result = executor.calculate_scale_filter((1920, 1080));
        
        // Should return scale filter with letterboxing
        assert!(result.is_some());
        let filter = result.unwrap();
        assert!(filter.contains("scale=1280:720"));
        assert!(filter.contains("force_original_aspect_ratio=decrease"));
        assert!(filter.contains("pad=1280:720"));
        assert!(filter.contains("(ow-iw)/2:(oh-ih)/2"));
    }

    #[test]
    fn test_calculate_scale_filter_no_upscaling_720p_smaller() {
        // Test no upscaling for 720p target with smaller source
        let executor = FFmpegExecutor::new(Resolution::Hd720, true);
        
        // Source: 640x480 (smaller than 1280x720)
        let result = executor.calculate_scale_filter((640, 480));
        
        // Should return None (no upscaling)
        assert_eq!(result, None);
    }

    #[test]
    fn test_calculate_scale_filter_partial_upscaling_width() {
        // Test no upscaling when only width is smaller
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        
        // Source: 1280x2160 (width smaller, height larger)
        let result = executor.calculate_scale_filter((1280, 2160));
        
        // Should return scale filter (height is larger, so we scale down)
        assert!(result.is_some());
    }

    #[test]
    fn test_calculate_scale_filter_partial_upscaling_height() {
        // Test no upscaling when only height is smaller
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        
        // Source: 3840x720 (width larger, height smaller)
        let result = executor.calculate_scale_filter((3840, 720));
        
        // Should return scale filter (width is larger, so we scale down)
        assert!(result.is_some());
    }

    #[test]
    fn test_calculate_scale_filter_format_correctness() {
        // Test that the filter string format is exactly as expected
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        
        // Source: 3840x2160
        let result = executor.calculate_scale_filter((3840, 2160));
        
        assert_eq!(
            result,
            Some("scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2".to_string())
        );
    }

    #[test]
    fn test_calculate_scale_filter_format_correctness_720p() {
        // Test that the filter string format is exactly as expected for 720p
        let executor = FFmpegExecutor::new(Resolution::Hd720, true);
        
        // Source: 1920x1080
        let result = executor.calculate_scale_filter((1920, 1080));
        
        assert_eq!(
            result,
            Some("scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2".to_string())
        );
    }
}
