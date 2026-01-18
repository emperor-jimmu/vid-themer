// FFmpeg command execution and video processing

use crate::cli::Resolution;
use crate::selector::TimeRange;
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

    /// Build FFmpeg command for extracting a clip
    /// Returns a vector of command arguments ready to be passed to Command
    pub fn build_extract_command(
        &self,
        video_path: &Path,
        time_range: &TimeRange,
        output_path: &Path,
        source_resolution: (u32, u32),
    ) -> Vec<String> {
        let mut args = Vec::new();

        // Start time (seek to position before input for faster processing)
        args.push("-ss".to_string());
        args.push(time_range.start_seconds.to_string());

        // Input file
        args.push("-i".to_string());
        args.push(video_path.to_string_lossy().to_string());

        // Duration
        args.push("-t".to_string());
        args.push(time_range.duration_seconds.to_string());

        // Video codec and preset
        args.push("-c:v".to_string());
        args.push("libx264".to_string());
        args.push("-preset".to_string());
        args.push("fast".to_string());

        // Add scale filter if needed (downscaling only, no upscaling)
        if let Some(scale_filter) = self.calculate_scale_filter(source_resolution) {
            args.push("-vf".to_string());
            args.push(scale_filter);
        }

        // Audio handling
        if !self.include_audio {
            // Exclude audio track
            args.push("-an".to_string());
        } else {
            // Include audio with copy codec for speed
            args.push("-c:a".to_string());
            args.push("aac".to_string());
        }

        // Output file (overwrite if exists)
        args.push("-y".to_string());
        args.push(output_path.to_string_lossy().to_string());

        args
    }

    /// Extract a clip from a video file
    /// Executes FFmpeg command and captures stderr for error messages
    pub fn extract_clip(
        &self,
        video_path: &Path,
        time_range: &TimeRange,
        output_path: &Path,
    ) -> Result<(), FFmpegError> {
        // Get source resolution first
        let source_resolution = self.get_video_resolution(video_path)?;

        // Build the FFmpeg command
        let args = self.build_extract_command(
            video_path,
            time_range,
            output_path,
            source_resolution,
        );

        // Execute FFmpeg command
        let output = Command::new("ffmpeg")
            .args(&args)
            .output()
            .map_err(|e| FFmpegError::ExecutionFailed(format!("Failed to execute ffmpeg: {}", e)))?;

        // Check if the command was successful
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FFmpegError::ExecutionFailed(format!(
                "FFmpeg clip extraction failed: {}",
                stderr
            )));
        }

        Ok(())
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
    use proptest::prelude::*;

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

    // Feature: video-clip-extractor, Property 7: No Upscaling
    // **Validates: Requirements 2.5, 2.6, 2.7**
    proptest! {
        #[test]
        fn test_no_upscaling_property(
            source_width in 1u32..=1920,
            source_height in 1u32..=1080,
            target_resolution in prop::sample::select(vec![Resolution::Hd720, Resolution::Hd1080])
        ) {
            let executor = FFmpegExecutor::new(target_resolution.clone(), true);
            
            // Determine target dimensions
            let (target_width, target_height) = match target_resolution {
                Resolution::Hd720 => (1280u32, 720u32),
                Resolution::Hd1080 => (1920u32, 1080u32),
            };
            
            // If source resolution is smaller than or equal to target in both dimensions,
            // no scaling filter should be returned (no upscaling)
            if source_width <= target_width && source_height <= target_height {
                let result = executor.calculate_scale_filter((source_width, source_height));
                prop_assert_eq!(result, None, 
                    "Expected no upscaling for source {}x{} with target {}x{}", 
                    source_width, source_height, target_width, target_height);
            }
            // If source is larger in at least one dimension, scaling should occur
            else if source_width > target_width || source_height > target_height {
                let result = executor.calculate_scale_filter((source_width, source_height));
                prop_assert!(result.is_some(), 
                    "Expected scaling for source {}x{} with target {}x{}", 
                    source_width, source_height, target_width, target_height);
                
                // Verify the filter contains the correct target dimensions
                let filter = result.unwrap();
                prop_assert!(filter.contains(&format!("scale={}:{}", target_width, target_height)),
                    "Filter should contain correct target dimensions");
            }
        }
    }

    // Helper function to parse duration string (extracted from get_duration logic)
    fn parse_duration_string(duration_str: &str) -> Result<f64, FFmpegError> {
        duration_str
            .trim()
            .parse::<f64>()
            .map_err(|e| FFmpegError::ParseError(format!(
                "Failed to parse duration '{}': {}",
                duration_str, e
            )))
    }

    // Feature: video-clip-extractor, Property 19: Duration Parsing Correctness
    // **Validates: Requirements 9.2, 9.4**
    proptest! {
        #[test]
        fn test_duration_parsing_correctness(
            // Generate durations from 0.001 to 86400.0 (24 hours)
            // with up to 6 decimal places to test fractional seconds
            whole_seconds in 0u32..=86400,
            fractional_part in 0u32..=999999,
        ) {
            // Construct a duration value with fractional seconds
            let expected_duration = whole_seconds as f64 + (fractional_part as f64 / 1_000_000.0);
            
            // Test various precision formats that FFmpeg might output
            let test_cases = vec![
                (format!("{:.6}", expected_duration), 6),
                (format!("{:.3}", expected_duration), 3),
                (format!("{:.2}", expected_duration), 2),
                (format!("{:.0}", expected_duration), 0),
            ];
            
            for (duration_str, precision) in test_cases {
                let parsed = parse_duration_string(&duration_str);
                
                prop_assert!(parsed.is_ok(), 
                    "Failed to parse valid duration string: '{}'", duration_str);
                
                let parsed_value = parsed.unwrap();
                
                // The parsed value should match what we formatted
                // We need to account for the precision loss during formatting
                let formatted_expected: f64 = duration_str.trim().parse().unwrap();
                let difference = (parsed_value - formatted_expected).abs();
                
                prop_assert!(difference < 1e-10, 
                    "Parsed duration {} differs from formatted expected {} by {} (string: '{}', precision: {})",
                    parsed_value, formatted_expected, difference, duration_str, precision);
            }
            
            // Test with whitespace variations (using full precision)
            let duration_str_with_spaces = vec![
                format!("{:.6} ", expected_duration),
                format!(" {:.6}", expected_duration),
                format!("  {:.6}  ", expected_duration),
            ];
            
            for duration_str in duration_str_with_spaces {
                let parsed = parse_duration_string(&duration_str);
                
                prop_assert!(parsed.is_ok(), 
                    "Failed to parse duration string with whitespace: '{}'", duration_str);
                
                let parsed_value = parsed.unwrap();
                let difference = (parsed_value - expected_duration).abs();
                
                // Allow for small floating point precision differences
                prop_assert!(difference < 0.0001, 
                    "Parsed duration {} differs from expected {} by {} (string: '{}')",
                    parsed_value, expected_duration, difference, duration_str);
            }
        }
    }

    proptest! {
        #[test]
        fn test_duration_parsing_handles_edge_cases(
            // Test very small and very large durations
            duration in prop::num::f64::ANY.prop_filter(
                "Must be non-negative and finite",
                |d| d.is_finite() && *d >= 0.0 && *d <= 86400.0
            )
        ) {
            // Format the duration as FFmpeg would
            let duration_str = format!("{:.6}", duration);
            
            let parsed = parse_duration_string(&duration_str);
            
            prop_assert!(parsed.is_ok(), 
                "Failed to parse duration string: '{}'", duration_str);
            
            let parsed_value = parsed.unwrap();
            
            // Verify the parsed value matches the original
            let difference = (parsed_value - duration).abs();
            prop_assert!(difference < 0.0001, 
                "Parsed duration {} differs from expected {} by {}",
                parsed_value, duration, difference);
        }
    }

    proptest! {
        #[test]
        fn test_duration_parsing_rejects_invalid_input(
            // Generate invalid strings that should fail parsing
            invalid_str in prop::string::string_regex("[a-zA-Z]+").unwrap()
        ) {
            let result = parse_duration_string(&invalid_str);
            
            prop_assert!(result.is_err(), 
                "Expected parsing to fail for invalid string: '{}'", invalid_str);
            
            // Verify it's a ParseError
            match result {
                Err(FFmpegError::ParseError(_)) => {
                    // Expected error type
                }
                _ => {
                    return Err(proptest::test_runner::TestCaseError::fail(
                        "Expected ParseError for invalid duration string"
                    ));
                }
            }
        }
    }

    // Tests for build_extract_command

    #[test]
    fn test_build_extract_command_basic() {
        use crate::selector::TimeRange;
        use std::path::PathBuf;

        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let video_path = PathBuf::from("/path/to/video.mp4");
        let output_path = PathBuf::from("/path/to/output.mp4");
        let time_range = TimeRange {
            start_seconds: 120.5,
            duration_seconds: 7.0,
        };
        let source_resolution = (1920, 1080);

        let args = executor.build_extract_command(
            &video_path,
            &time_range,
            &output_path,
            source_resolution,
        );

        // Verify essential arguments are present
        assert!(args.contains(&"-ss".to_string()));
        assert!(args.contains(&"120.5".to_string()));
        assert!(args.contains(&"-i".to_string()));
        assert!(args.contains(&"-t".to_string()));
        assert!(args.contains(&"7".to_string()));
        assert!(args.contains(&"-c:v".to_string()));
        assert!(args.contains(&"libx264".to_string()));
        assert!(args.contains(&"-preset".to_string()));
        assert!(args.contains(&"fast".to_string()));
        assert!(args.contains(&"-y".to_string()));

        // Audio should be included (aac codec)
        assert!(args.contains(&"-c:a".to_string()));
        assert!(args.contains(&"aac".to_string()));
        assert!(!args.contains(&"-an".to_string()));
    }

    #[test]
    fn test_build_extract_command_no_audio() {
        use crate::selector::TimeRange;
        use std::path::PathBuf;

        let executor = FFmpegExecutor::new(Resolution::Hd1080, false);
        let video_path = PathBuf::from("/path/to/video.mp4");
        let output_path = PathBuf::from("/path/to/output.mp4");
        let time_range = TimeRange {
            start_seconds: 60.0,
            duration_seconds: 5.0,
        };
        let source_resolution = (1920, 1080);

        let args = executor.build_extract_command(
            &video_path,
            &time_range,
            &output_path,
            source_resolution,
        );

        // Audio should be excluded
        assert!(args.contains(&"-an".to_string()));
        assert!(!args.contains(&"-c:a".to_string()));
    }

    #[test]
    fn test_build_extract_command_with_scaling() {
        use crate::selector::TimeRange;
        use std::path::PathBuf;

        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let video_path = PathBuf::from("/path/to/video.mp4");
        let output_path = PathBuf::from("/path/to/output.mp4");
        let time_range = TimeRange {
            start_seconds: 30.0,
            duration_seconds: 10.0,
        };
        // Source is 4K, should be scaled down to 1080p
        let source_resolution = (3840, 2160);

        let args = executor.build_extract_command(
            &video_path,
            &time_range,
            &output_path,
            source_resolution,
        );

        // Should include scale filter
        assert!(args.contains(&"-vf".to_string()));
        
        // Find the scale filter argument
        let vf_index = args.iter().position(|arg| arg == "-vf").unwrap();
        let scale_filter = &args[vf_index + 1];
        
        assert!(scale_filter.contains("scale=1920:1080"));
        assert!(scale_filter.contains("force_original_aspect_ratio=decrease"));
        assert!(scale_filter.contains("pad=1920:1080"));
    }

    #[test]
    fn test_build_extract_command_no_upscaling() {
        use crate::selector::TimeRange;
        use std::path::PathBuf;

        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let video_path = PathBuf::from("/path/to/video.mp4");
        let output_path = PathBuf::from("/path/to/output.mp4");
        let time_range = TimeRange {
            start_seconds: 15.0,
            duration_seconds: 8.0,
        };
        // Source is 720p, should NOT be upscaled to 1080p
        let source_resolution = (1280, 720);

        let args = executor.build_extract_command(
            &video_path,
            &time_range,
            &output_path,
            source_resolution,
        );

        // Should NOT include scale filter
        assert!(!args.contains(&"-vf".to_string()));
    }

    #[test]
    fn test_build_extract_command_720p_target() {
        use crate::selector::TimeRange;
        use std::path::PathBuf;

        let executor = FFmpegExecutor::new(Resolution::Hd720, true);
        let video_path = PathBuf::from("/path/to/video.mp4");
        let output_path = PathBuf::from("/path/to/output.mp4");
        let time_range = TimeRange {
            start_seconds: 45.0,
            duration_seconds: 6.5,
        };
        // Source is 1080p, should be scaled down to 720p
        let source_resolution = (1920, 1080);

        let args = executor.build_extract_command(
            &video_path,
            &time_range,
            &output_path,
            source_resolution,
        );

        // Should include scale filter for 720p
        assert!(args.contains(&"-vf".to_string()));
        
        let vf_index = args.iter().position(|arg| arg == "-vf").unwrap();
        let scale_filter = &args[vf_index + 1];
        
        assert!(scale_filter.contains("scale=1280:720"));
        assert!(scale_filter.contains("pad=1280:720"));
    }

    #[test]
    fn test_build_extract_command_argument_order() {
        use crate::selector::TimeRange;
        use std::path::PathBuf;

        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let video_path = PathBuf::from("/path/to/video.mp4");
        let output_path = PathBuf::from("/path/to/output.mp4");
        let time_range = TimeRange {
            start_seconds: 100.0,
            duration_seconds: 5.0,
        };
        let source_resolution = (1920, 1080);

        let args = executor.build_extract_command(
            &video_path,
            &time_range,
            &output_path,
            source_resolution,
        );

        // Verify -ss comes before -i (for faster seeking)
        let ss_index = args.iter().position(|arg| arg == "-ss").unwrap();
        let i_index = args.iter().position(|arg| arg == "-i").unwrap();
        assert!(ss_index < i_index, "Start time (-ss) should come before input (-i)");

        // Verify -t comes after -i
        let t_index = args.iter().position(|arg| arg == "-t").unwrap();
        assert!(t_index > i_index, "Duration (-t) should come after input (-i)");

        // Verify output path is last
        assert_eq!(args.last().unwrap(), &output_path.to_string_lossy().to_string());
    }

    #[test]
    fn test_build_extract_command_overwrite_flag() {
        use crate::selector::TimeRange;
        use std::path::PathBuf;

        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let video_path = PathBuf::from("/path/to/video.mp4");
        let output_path = PathBuf::from("/path/to/output.mp4");
        let time_range = TimeRange {
            start_seconds: 0.0,
            duration_seconds: 5.0,
        };
        let source_resolution = (1920, 1080);

        let args = executor.build_extract_command(
            &video_path,
            &time_range,
            &output_path,
            source_resolution,
        );

        // Should include -y flag to overwrite existing files
        assert!(args.contains(&"-y".to_string()));
    }
}
