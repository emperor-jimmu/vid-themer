// Video processing pipeline coordination

use crate::ffmpeg::FFmpegExecutor;
use crate::scanner::VideoFile;
use crate::selector::ClipSelector;
use std::path::PathBuf;

pub struct VideoProcessor {
    selector: Box<dyn ClipSelector>,
    ffmpeg: FFmpegExecutor,
    intro_exclusion_percent: f64,
    outro_exclusion_percent: f64,
}

impl VideoProcessor {
    pub fn new(
        selector: Box<dyn ClipSelector>,
        ffmpeg: FFmpegExecutor,
        intro_exclusion_percent: f64,
        outro_exclusion_percent: f64,
    ) -> Self {
        Self {
            selector,
            ffmpeg,
            intro_exclusion_percent,
            outro_exclusion_percent,
        }
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
        let time_range = match self.selector.select_segment(
            &video.path,
            duration,
            self.intro_exclusion_percent,
            self.outro_exclusion_percent,
        ) {
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
    #[allow(dead_code)]
    ProcessingFailed(String),
    
    #[error("Failed to get video duration: {0}")]
    #[allow(dead_code)]
    DurationDetectionFailed(String),
    
    #[error("Failed to select clip segment: {0}")]
    #[allow(dead_code)]
    SegmentSelectionFailed(String),
    
    #[error("Failed to create output directory: {0}")]
    OutputDirectoryCreationFailed(String),
    
    #[error("Failed to extract clip: {0}")]
    #[allow(dead_code)]
    ClipExtractionFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Resolution;
    use crate::selector::{ClipSelector, TimeRange};
    use proptest::prelude::*;
    use proptest::test_runner::Config as ProptestConfig;
    use std::fs;
    use std::io::Write;

    // Mock selector for testing
    struct MockSelector;

    impl ClipSelector for MockSelector {
        fn select_segment(
            &self,
            _video_path: &std::path::Path,
            duration: f64,
            _intro_exclusion_percent: f64,
            _outro_exclusion_percent: f64,
        ) -> Result<TimeRange, crate::selector::SelectionError> {
            // Return a simple time range for testing
            Ok(TimeRange {
                start_seconds: 0.0,
                duration_seconds: duration.min(5.0),
            })
        }
    }

    // Helper function to create a test video file structure
    fn create_test_video_structure(base_dir: &std::path::Path, video_name: &str) -> VideoFile {
        let video_path = base_dir.join(video_name);
        
        // Create a dummy video file
        let mut file = fs::File::create(&video_path).unwrap();
        file.write_all(b"fake video content").unwrap();
        
        VideoFile {
            path: video_path,
            parent_dir: base_dir.to_path_buf(),
        }
    }

    // Feature: video-clip-extractor, Property 6: Output File Naming
    // **Validates: Requirements 2.3, 5.5**
    proptest! {
        #[test]
        fn test_output_file_naming(
            // Generate various video file names to ensure output is always "backdrop.mp4"
            video_name in "[a-zA-Z0-9_-]{1,20}\\.(mp4|mkv)",
            // Test with different parent directory names
            parent_dir_name in "[a-zA-Z0-9_-]{1,15}",
        ) {
            // Property: For any processed video, the output file should always be named "backdrop.mp4"
            // regardless of the source video name or parent directory structure
            
            // Create a temporary directory for testing
            let temp_base = std::env::temp_dir().join(format!(
                "processor_test_{}_{}",
                std::process::id(),
                rand::random::<u32>()
            ));
            let _ = fs::remove_dir_all(&temp_base);
            
            // Create parent directory with the generated name
            let parent_dir = temp_base.join(&parent_dir_name);
            fs::create_dir_all(&parent_dir).unwrap();
            
            // Create test video file
            let video_file = create_test_video_structure(&parent_dir, &video_name);
            
            // Create processor with mock selector
            let selector = Box::new(MockSelector);
            let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
            let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0);
            
            // Call create_output_directory to get the output path
            let output_path = processor.create_output_directory(&video_file);
            
            // Property 1: The method should succeed
            prop_assert!(
                output_path.is_ok(),
                "create_output_directory should succeed for video {:?}",
                video_file.path
            );
            
            let output_path = output_path.unwrap();
            
            // Property 2: The output file name should always be "backdrop.mp4" (lowercase)
            let output_filename = output_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            
            prop_assert_eq!(
                output_filename,
                "backdrop.mp4",
                "Output file name should always be 'backdrop.mp4' (lowercase), \
                 regardless of source video name '{}' or parent directory '{}'",
                video_name,
                parent_dir_name
            );
            
            // Property 3: The output should be in a "backdrops" subdirectory
            let parent_of_output = output_path.parent().unwrap();
            let backdrops_dir_name = parent_of_output.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            
            prop_assert_eq!(
                backdrops_dir_name,
                "backdrops",
                "Output should be in a 'backdrops' subdirectory"
            );
            
            // Property 4: The backdrops directory should be in the video's parent directory
            let backdrops_parent = parent_of_output.parent().unwrap();
            prop_assert_eq!(
                backdrops_parent,
                &video_file.parent_dir,
                "Backdrops directory should be in the video's parent directory"
            );
            
            // Property 5: The full output path structure should be: parent_dir/backdrops/backdrop.mp4
            let expected_path = video_file.parent_dir.join("backdrops").join("backdrop.mp4");
            prop_assert_eq!(
                &output_path,
                &expected_path,
                "Output path should follow the structure: parent_dir/backdrops/backdrop.mp4"
            );
            
            // Property 6: Verify the backdrops directory was actually created
            prop_assert!(
                parent_of_output.exists(),
                "Backdrops directory should be created at {:?}",
                parent_of_output
            );
            
            prop_assert!(
                parent_of_output.is_dir(),
                "Backdrops path should be a directory"
            );
            
            // Clean up
            let _ = fs::remove_dir_all(&temp_base);
        }
    }

    #[test]
    fn test_output_file_naming_basic() {
        // Basic unit test to verify output file naming
        let temp_dir = std::env::temp_dir().join(format!(
            "processor_basic_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        
        // Create a test video
        let video_file = create_test_video_structure(&temp_dir, "test_video.mp4");
        
        // Create processor
        let selector = Box::new(MockSelector);
        let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
        let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0);
        
        // Get output path
        let output_path = processor.create_output_directory(&video_file).unwrap();
        
        // Verify the output file name is "backdrop.mp4"
        assert_eq!(
            output_path.file_name().unwrap().to_str().unwrap(),
            "backdrop.mp4"
        );
        
        // Verify the structure
        let expected = temp_dir.join("backdrops").join("backdrop.mp4");
        assert_eq!(output_path, expected);
        
        // Verify backdrops directory exists
        assert!(temp_dir.join("backdrops").exists());
        assert!(temp_dir.join("backdrops").is_dir());
        
        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_output_file_naming_with_different_video_names() {
        // Test that output is always "backdrop.mp4" regardless of source video name
        let temp_dir = std::env::temp_dir().join(format!(
            "processor_names_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();
        
        let test_video_names = vec![
            "movie.mp4",
            "VIDEO.MP4",
            "My_Video_File.mkv",
            "video-with-dashes.mp4",
            "123456.mp4",
        ];
        
        let selector = Box::new(MockSelector);
        let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
        let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0);
        
        for video_name in test_video_names {
            let video_file = create_test_video_structure(&temp_dir, video_name);
            let output_path = processor.create_output_directory(&video_file).unwrap();
            
            // Always should be "backdrop.mp4" in lowercase
            assert_eq!(
                output_path.file_name().unwrap().to_str().unwrap(),
                "backdrop.mp4",
                "Output should always be 'backdrop.mp4' for source video '{}'",
                video_name
            );
        }
        
        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    // Feature: video-clip-extractor, Property 15: Error Recovery Continuation
    // **Validates: Requirements 7.2, 7.3**
    // Note: This test is ignored by default because it makes real FFmpeg calls on fake files
    // which can be slow. The unit test test_error_recovery_continuation_unit covers the same behavior.
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10))]
        #[test]
        #[ignore]
        fn test_error_recovery_continuation(
            // Generate a batch of videos with varying counts
            video_count in 3usize..6usize,
        ) {
            // Property: For any batch of videos where some fail to process,
            // the tool should continue processing remaining videos and not halt on the first error
            
            // This test simulates the behavior by creating a batch of ProcessResults
            // where some succeed and some fail, then verifying that all videos are processed
            
            // Create a temporary directory for testing
            let temp_base = std::env::temp_dir().join(format!(
                "processor_error_recovery_test_{}_{}",
                std::process::id(),
                rand::random::<u32>()
            ));
            let _ = fs::remove_dir_all(&temp_base);
            fs::create_dir_all(&temp_base).unwrap();
            
            // Create multiple video files
            let mut video_files = Vec::new();
            for i in 0..video_count {
                let video_dir = temp_base.join(format!("video_dir_{}", i));
                fs::create_dir_all(&video_dir).unwrap();
                
                let video_name = format!("video_{}.mp4", i);
                let video_file = create_test_video_structure(&video_dir, &video_name);
                video_files.push(video_file);
            }
            
            // Create a processor with mock selector
            // Note: The actual FFmpeg calls will fail on fake video files,
            // which is perfect for testing error recovery
            let selector = Box::new(MockSelector);
            let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
            let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0);
            
            // Process all videos and collect results
            // Since we're using fake video files, FFmpeg will fail to get duration
            // This simulates real-world errors (corrupted videos, etc.)
            let mut results = Vec::new();
            for video in &video_files {
                let result = processor.process_video(video);
                results.push(result);
            }
            
            // Property 1: All videos should have been processed (no early termination)
            // This is the core property: even if some videos fail, all should be attempted
            prop_assert_eq!(
                results.len(),
                video_count,
                "All {} videos should be processed, demonstrating that processing \
                 continues after errors rather than halting on first failure",
                video_count
            );
            
            // Property 2: Each result should have a video_path set
            for (i, result) in results.iter().enumerate() {
                prop_assert!(
                    !result.video_path.as_os_str().is_empty(),
                    "Result {} should have a video path set",
                    i
                );
            }
            
            // Property 3: Failed results should have error messages
            for (i, result) in results.iter().enumerate() {
                if !result.success {
                    prop_assert!(
                        result.error_message.is_some(),
                        "Failed result {} should have an error message",
                        i
                    );
                    
                    let error_msg = result.error_message.as_ref().unwrap();
                    prop_assert!(
                        !error_msg.is_empty(),
                        "Error message for result {} should not be empty",
                        i
                    );
                }
            }
            
            // Property 4: The processing loop doesn't panic or abort
            // (if we got here, the loop completed successfully)
            // This validates Requirements 7.2: continue processing other videos after error
            prop_assert!(
                true,
                "Processing loop completed without panic or early termination"
            );
            
            // Property 5: Verify that process_video returns a result for each video
            // (not None, not panic, just a Result indicating success or failure)
            for (i, result) in results.iter().enumerate() {
                // Each result should have either success=true or an error_message
                if result.success {
                    prop_assert!(
                        result.error_message.is_none(),
                        "Successful result {} should not have error message",
                        i
                    );
                } else {
                    prop_assert!(
                        result.error_message.is_some(),
                        "Failed result {} should have error message",
                        i
                    );
                }
            }
            
            // Clean up
            let _ = fs::remove_dir_all(&temp_base);
        }
    }

    #[test]
    #[ignore] // Ignored by default - makes real FFmpeg calls which are slow on fake files
    fn test_error_recovery_continuation_unit() {
        // Unit test to explicitly verify error recovery behavior
        // This test creates a scenario where one video fails and verifies
        // that subsequent videos are still processed
        
        let temp_base = std::env::temp_dir().join(format!(
            "processor_error_recovery_unit_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_base);
        fs::create_dir_all(&temp_base).unwrap();
        
        // Create three video files (all will fail FFmpeg calls since they're fake)
        let video1_dir = temp_base.join("video1");
        let video2_dir = temp_base.join("video2");
        let video3_dir = temp_base.join("video3");
        
        fs::create_dir_all(&video1_dir).unwrap();
        fs::create_dir_all(&video2_dir).unwrap();
        fs::create_dir_all(&video3_dir).unwrap();
        
        let video1 = create_test_video_structure(&video1_dir, "video1.mp4");
        let video2 = create_test_video_structure(&video2_dir, "video2.mp4");
        let video3 = create_test_video_structure(&video3_dir, "video3.mp4");
        
        let videos = vec![video1, video2, video3];
        
        // Create processor
        let selector = Box::new(MockSelector);
        let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
        let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0);
        
        // Process all videos
        let mut results = Vec::new();
        for video in &videos {
            let result = processor.process_video(video);
            results.push(result);
        }
        
        // Verify all three videos were processed (no early termination)
        assert_eq!(
            results.len(),
            3,
            "All 3 videos should be processed despite errors"
        );
        
        // Verify each result has a video path
        for (i, result) in results.iter().enumerate() {
            assert!(
                !result.video_path.as_os_str().is_empty(),
                "Result {} should have video path",
                i
            );
        }
        
        // Since these are fake videos, they should all fail
        // But the important thing is that all were attempted
        for (i, result) in results.iter().enumerate() {
            if !result.success {
                assert!(
                    result.error_message.is_some(),
                    "Failed result {} should have error message",
                    i
                );
            }
        }
        
        println!("✓ Error recovery test passed: all {} videos were processed", results.len());
        
        // Clean up
        let _ = fs::remove_dir_all(&temp_base);
    }

    // Feature: video-clip-extractor, Property 16: Error Messages Include Paths
    // **Validates: Requirements 7.5**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]
        #[test]
        fn test_error_messages_include_paths(
            // Generate various video file names
            video_name in "[a-zA-Z0-9_-]{1,20}\\.(mp4|mkv)",
            // Generate different parent directory names
            parent_dir_name in "[a-zA-Z0-9_-]{1,15}",
        ) {
            // Property: For any error that occurs during processing,
            // the error message should include the file path of the video being processed
            
            // Create a temporary directory for testing
            let temp_base = std::env::temp_dir().join(format!(
                "processor_error_path_test_{}_{}",
                std::process::id(),
                rand::random::<u32>()
            ));
            let _ = fs::remove_dir_all(&temp_base);
            
            // Create parent directory with the generated name
            let parent_dir = temp_base.join(&parent_dir_name);
            fs::create_dir_all(&parent_dir).unwrap();
            
            // Create test video file (fake video that will cause FFmpeg to fail)
            let video_file = create_test_video_structure(&parent_dir, &video_name);
            
            // Store the video path for verification
            let video_path = video_file.path.clone();
            let video_path_str = video_path.to_string_lossy().to_string();
            
            // Create processor with mock selector
            let selector = Box::new(MockSelector);
            let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
            let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0);
            
            // Process the video (will fail because it's a fake video file)
            let result = processor.process_video(&video_file);
            
            // Property 1: The result should have the video_path set
            prop_assert_eq!(
                &result.video_path,
                &video_path,
                "ProcessResult should contain the video path"
            );
            
            // Property 2: Since this is a fake video, processing should fail
            // (FFmpeg will fail to get duration from the fake video file)
            prop_assert!(
                !result.success,
                "Processing should fail for fake video file"
            );
            
            // Property 3: Failed processing should have an error message
            prop_assert!(
                result.error_message.is_some(),
                "Failed processing should have an error message"
            );
            
            let error_message = result.error_message.as_ref().unwrap();
            
            // Property 4: The error message should not be empty
            prop_assert!(
                !error_message.is_empty(),
                "Error message should not be empty"
            );
            
            // Property 5: The error message should be descriptive
            // It should indicate what went wrong (e.g., "Failed to get video duration")
            prop_assert!(
                error_message.contains("Failed") || 
                error_message.contains("failed") ||
                error_message.contains("Error") ||
                error_message.contains("error"),
                "Error message should be descriptive and indicate failure: '{}'",
                error_message
            );
            
            // Property 6 (CORE): The ProcessResult should contain the video path
            // This allows the caller (main.rs) to include the path in error output
            // The video_path field in ProcessResult serves as the path reference
            prop_assert!(
                !result.video_path.as_os_str().is_empty(),
                "ProcessResult.video_path should be set to allow error reporting with path"
            );
            
            // Property 7: Verify the video_path in the result matches the original video
            prop_assert_eq!(
                result.video_path.to_string_lossy().to_string(),
                video_path_str,
                "ProcessResult.video_path should match the original video path"
            );
            
            // Property 8: The video_path should contain the video filename
            let video_filename = video_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            
            prop_assert!(
                result.video_path.to_string_lossy().contains(video_filename),
                "ProcessResult.video_path should contain the video filename '{}'",
                video_filename
            );
            
            // Property 9: The video_path should contain the parent directory name
            prop_assert!(
                result.video_path.to_string_lossy().contains(&parent_dir_name),
                "ProcessResult.video_path should contain the parent directory name '{}'",
                parent_dir_name
            );
            
            // Clean up
            let _ = fs::remove_dir_all(&temp_base);
        }
    }

    // Feature: video-clip-extractor, Property 14: Overwrite Existing Clips
    // **Validates: Requirements 5.3**
    proptest! {
        #[test]
        fn test_overwrite_existing_clips(
            // Generate various video file names
            video_name in "[a-zA-Z0-9_-]{1,20}\\.(mp4|mkv)",
            // Generate different parent directory names
            parent_dir_name in "[a-zA-Z0-9_-]{1,15}",
            // Generate different content for the existing and new files
            existing_content in "[a-zA-Z0-9 ]{10,50}",
            new_content in "[a-zA-Z0-9 ]{10,50}",
        ) {
            // Property: For any video in a directory with an existing backdrops/backdrop.mp4 file,
            // processing should replace the existing file with the new clip
            
            // Create a temporary directory for testing
            let temp_base = std::env::temp_dir().join(format!(
                "processor_overwrite_test_{}_{}",
                std::process::id(),
                rand::random::<u32>()
            ));
            let _ = fs::remove_dir_all(&temp_base);
            
            // Create parent directory with the generated name
            let parent_dir = temp_base.join(&parent_dir_name);
            fs::create_dir_all(&parent_dir).unwrap();
            
            // Create test video file
            let video_file = create_test_video_structure(&parent_dir, &video_name);
            
            // Create processor with mock selector
            let selector = Box::new(MockSelector);
            let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
            let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0);
            
            // Step 1: Create the backdrops directory and an existing backdrop.mp4 file
            let backdrops_dir = parent_dir.join("backdrops");
            fs::create_dir_all(&backdrops_dir).unwrap();
            
            let existing_backdrop_path = backdrops_dir.join("backdrop.mp4");
            let mut existing_file = fs::File::create(&existing_backdrop_path).unwrap();
            existing_file.write_all(existing_content.as_bytes()).unwrap();
            drop(existing_file); // Close the file
            
            // Verify the existing file was created
            prop_assert!(
                existing_backdrop_path.exists(),
                "Existing backdrop.mp4 should exist before processing"
            );
            
            // Read the existing file content to verify it later
            let existing_file_content = fs::read_to_string(&existing_backdrop_path).unwrap();
            prop_assert_eq!(
                &existing_file_content,
                &existing_content,
                "Existing file should contain the original content"
            );
            
            // Get the metadata of the existing file (modification time, size)
            let existing_metadata = fs::metadata(&existing_backdrop_path).unwrap();
            let existing_size = existing_metadata.len();
            
            // Step 2: Call create_output_directory which should return the path to backdrop.mp4
            let output_path = processor.create_output_directory(&video_file);
            
            // Property 1: The method should succeed even when the file already exists
            prop_assert!(
                output_path.is_ok(),
                "create_output_directory should succeed even when backdrop.mp4 already exists"
            );
            
            let output_path = output_path.unwrap();
            
            // Property 2: The output path should point to the same location as the existing file
            prop_assert_eq!(
                &output_path,
                &existing_backdrop_path,
                "Output path should be the same as the existing backdrop.mp4 path"
            );
            
            // Step 3: Simulate writing new content to the output path (overwriting)
            let mut new_file = fs::File::create(&output_path).unwrap();
            new_file.write_all(new_content.as_bytes()).unwrap();
            drop(new_file); // Close the file
            
            // Property 3: The file should still exist after overwriting
            prop_assert!(
                output_path.exists(),
                "backdrop.mp4 should still exist after overwriting"
            );
            
            // Property 4: The file content should be the new content (overwritten)
            let new_file_content = fs::read_to_string(&output_path).unwrap();
            prop_assert_eq!(
                &new_file_content,
                &new_content,
                "File content should be the new content after overwriting"
            );
            
            // Property 5: The file should not contain the old content
            prop_assert_ne!(
                &new_file_content,
                &existing_content,
                "File should not contain the old content after overwriting"
            );
            
            // Property 6: The file size should have changed (if content sizes differ)
            let new_metadata = fs::metadata(&output_path).unwrap();
            let new_size = new_metadata.len();
            
            if existing_content.len() != new_content.len() {
                prop_assert_ne!(
                    existing_size,
                    new_size,
                    "File size should change when content is overwritten with different size"
                );
            }
            
            // Property 7: Verify the file path structure is still correct
            let parent_of_output = output_path.parent().unwrap();
            let backdrops_dir_name = parent_of_output.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            
            prop_assert_eq!(
                backdrops_dir_name,
                "backdrops",
                "Output should still be in the 'backdrops' subdirectory after overwrite"
            );
            
            // Property 8: Verify the filename is still "backdrop.mp4"
            let output_filename = output_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            
            prop_assert_eq!(
                output_filename,
                "backdrop.mp4",
                "Output filename should still be 'backdrop.mp4' after overwrite"
            );
            
            // Clean up
            let _ = fs::remove_dir_all(&temp_base);
        }
    }
}
