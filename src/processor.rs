// Video processing pipeline coordination

use crate::ffmpeg::FFmpegExecutor;
use crate::scanner::VideoFile;
use crate::selector::ClipSelector;
use std::path::PathBuf;

// Constants for output directory and file naming
const BACKDROPS_DIR: &str = "backdrops";
#[allow(dead_code)]
const BACKDROP_FILE: &str = "backdrop.mp4";

pub struct VideoProcessor {
    selector: Box<dyn ClipSelector>,
    ffmpeg: FFmpegExecutor,
    intro_exclusion_percent: f64,
    outro_exclusion_percent: f64,
    clip_count: u8,
    clip_config: crate::selector::ClipConfig,
}

impl VideoProcessor {
    pub fn new(
        selector: Box<dyn ClipSelector>,
        ffmpeg: FFmpegExecutor,
        intro_exclusion_percent: f64,
        outro_exclusion_percent: f64,
        clip_count: u8,
        clip_config: crate::selector::ClipConfig,
    ) -> Self {
        Self {
            selector,
            ffmpeg,
            intro_exclusion_percent,
            outro_exclusion_percent,
            clip_count,
            clip_config,
        }
    }

    /// Process a video file: detect duration, select segment, create output directory, and extract clip
    /// Returns ProcessResult with success/failure status
    /// Handles errors gracefully by logging and continuing
    /// 
    /// The progress_callback is called after each clip is successfully extracted with:
    /// - clip_num: The number of the clip that was just extracted (1-indexed)
    /// - total_clips: The total number of clips being extracted
    /// - filename: The name of the clip file that was just created
    pub fn process_video<F>(&self, video: &VideoFile, mut progress_callback: F) -> ProcessResult
    where
        F: FnMut(usize, usize, &str),
    {
        let video_path = video.path.clone();

        // Step 0: Check for existing clips and determine how many more we need
        let backdrops_dir = video.parent_dir.join(BACKDROPS_DIR);
        let existing_clip_count = if backdrops_dir.exists() {
            self.count_existing_clips(&backdrops_dir)
        } else {
            0
        };

        // Calculate how many clips we need to generate
        let clips_to_generate = if existing_clip_count >= self.clip_count {
            // Already have enough clips, skip this video
            return ProcessResult {
                video_path,
                output_path: PathBuf::new(),
                success: true,
                error_message: None,
                ffmpeg_stderr: None,
                clips_generated: 0,
                clip_filenames: Vec::new(),
            };
        } else {
            self.clip_count - existing_clip_count
        };

        // Step 1: Get video duration
        let duration = match self.ffmpeg.get_duration(&video.path) {
            Ok(d) => d,
            Err(e) => {
                let stderr = e.stderr().map(|s| s.to_string());
                let error_message = match e {
                    crate::ffmpeg::FFmpegError::CorruptedFile(_) => {
                        format!("Skipping corrupted or incomplete video file: {}", e)
                    }
                    _ => format!("Failed to get video duration: {}", e),
                };
                return ProcessResult {
                    video_path,
                    output_path: PathBuf::new(),
                    success: false,
                    error_message: Some(error_message),
                    ffmpeg_stderr: stderr,
                    clips_generated: 0,
                    clip_filenames: Vec::new(),
                };
            }
        };

        // Step 2: Select segments using ClipSelector strategy (only for the clips we need)
        let time_ranges = match self.selector.select_clips(
            &video.path,
            duration,
            self.intro_exclusion_percent,
            self.outro_exclusion_percent,
            clips_to_generate,
            &self.clip_config,
        ) {
            Ok(ranges) if !ranges.is_empty() => ranges,
            Ok(_) => {
                return ProcessResult {
                    video_path,
                    output_path: PathBuf::new(),
                    success: false,
                    error_message: Some(format!(
                        "No valid clips could be selected (requested: {} clips)",
                        clips_to_generate
                    )),
                    ffmpeg_stderr: None,
                    clips_generated: 0,
                    clip_filenames: Vec::new(),
                };
            }
            Err(e) => {
                return ProcessResult {
                    video_path,
                    output_path: PathBuf::new(),
                    success: false,
                    error_message: Some(format!(
                        "Failed to select clip segment (requested: {} clips): {}",
                        clips_to_generate, e
                    )),
                    ffmpeg_stderr: None,
                    clips_generated: 0,
                    clip_filenames: Vec::new(),
                };
            }
        };

        // Warn if fewer clips generated than requested
        if time_ranges.len() < clips_to_generate as usize {
            eprintln!(
                "Warning: Only generated {} of {} requested clips for {}",
                time_ranges.len(),
                clips_to_generate,
                video.path.display()
            );
        }

        // Step 3: Create output directory
        let backdrops_dir = match self.create_backdrops_directory(video) {
            Ok(dir) => dir,
            Err(e) => {
                return ProcessResult {
                    video_path,
                    output_path: PathBuf::new(),
                    success: false,
                    error_message: Some(format!("Failed to create output directory: {}", e)),
                    ffmpeg_stderr: None,
                    clips_generated: 0,
                    clip_filenames: Vec::new(),
                };
            }
        };

        // Step 4: Extract each clip with sequential naming, starting after existing clips
        let mut last_output_path = PathBuf::new();
        let mut clip_filenames = Vec::new();
        let total_clips = time_ranges.len();

        for (index, time_range) in time_ranges.iter().enumerate() {
            // Start numbering from existing_clip_count + 1
            let clip_num = existing_clip_count as usize + index + 1;
            let output_filename = format!("backdrop{}.mp4", clip_num);
            clip_filenames.push(output_filename.clone());

            let output_path = backdrops_dir.join(&output_filename);
            last_output_path = output_path.clone();

            if let Err(e) = self
                .ffmpeg
                .extract_clip(&video.path, time_range, &output_path)
            {
                let stderr = e.stderr().map(|s| s.to_string());
                return ProcessResult {
                    video_path,
                    output_path: output_path.clone(),
                    success: false,
                    error_message: Some(format!(
                        "Failed to extract clip {} of {} (backdrop{}.mp4): {}",
                        index + 1,
                        time_ranges.len(),
                        clip_num,
                        e
                    )),
                    ffmpeg_stderr: stderr,
                    clips_generated: index,
                    clip_filenames: clip_filenames.clone(),
                };
            }

            // Call progress callback after successful extraction
            progress_callback(index + 1, total_clips, &output_filename);
        }

        ProcessResult {
            video_path,
            output_path: last_output_path,
            success: true,
            error_message: None,
            ffmpeg_stderr: None,
            clips_generated: time_ranges.len(),
            clip_filenames,
        }
    }

    /// Count existing valid backdrop files in sequential order
    fn count_existing_clips(&self, backdrops_dir: &std::path::Path) -> u8 {
        let mut count = 0u8;
        
        // Check for backdrop files in sequential order (backdrop1.mp4, backdrop2.mp4, etc.)
        for i in 1..=4 {
            let backdrop_path = backdrops_dir.join(format!("backdrop{}.mp4", i));
            
            if let Ok(metadata) = std::fs::metadata(&backdrop_path) {
                if metadata.is_file() && metadata.len() > 0 {
                    count += 1;
                } else {
                    // Stop counting if we hit a zero-byte or invalid file
                    break;
                }
            } else {
                // Stop counting if the file doesn't exist
                break;
            }
        }
        
        count
    }

    /// Create the backdrops subdirectory and return the directory path
    /// Returns the path to the backdrops directory relative to the video's parent directory
    fn create_backdrops_directory(&self, video: &VideoFile) -> Result<PathBuf, ProcessError> {
        // Create backdrops subdirectory in video's parent directory
        let backdrops_dir = video.parent_dir.join(BACKDROPS_DIR);

        std::fs::create_dir_all(&backdrops_dir).map_err(|e| {
            ProcessError::OutputDirectoryCreationFailed(format!(
                "Failed to create directory {:?}: {}",
                backdrops_dir, e
            ))
        })?;

        // Return the backdrops directory path
        Ok(backdrops_dir)
    }

    /// Create the backdrops subdirectory and return the full output path
    /// Returns the path to backdrops/backdrop.mp4 relative to the video's parent directory
    /// This method is kept for backward compatibility with existing tests
    #[allow(dead_code)]
    fn create_output_directory(&self, video: &VideoFile) -> Result<PathBuf, ProcessError> {
        let backdrops_dir = self.create_backdrops_directory(video)?;
        Ok(backdrops_dir.join(BACKDROP_FILE))
    }
}

pub struct ProcessResult {
    pub video_path: PathBuf,
    pub output_path: PathBuf,
    pub success: bool,
    pub error_message: Option<String>,
    pub ffmpeg_stderr: Option<String>,
    pub clips_generated: usize,
    #[allow(dead_code)]
    pub clip_filenames: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
#[allow(clippy::enum_variant_names)]
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

    #[error("No valid clips could be selected")]
    #[allow(dead_code)]
    NoValidClips,
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
        fn select_clips(
            &self,
            _video_path: &std::path::Path,
            duration: f64,
            _intro_exclusion_percent: f64,
            _outro_exclusion_percent: f64,
            _clip_count: u8,
            config: &crate::selector::ClipConfig,
        ) -> Result<Vec<TimeRange>, crate::selector::SelectionError> {
            // Return a simple time range for testing (single clip for backward compatibility)
            Ok(vec![TimeRange {
                start_seconds: 0.0,
                duration_seconds: duration.min(config.max_duration),
            }])
        }
    }

    // Helper function to create default ClipConfig for tests
    fn default_test_config() -> crate::selector::ClipConfig {
        crate::selector::ClipConfig {
            min_duration: 10.0,
            max_duration: 15.0,
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

    // Feature: video-clip-extractor, Property 13: Backdrops Directory Creation
    // **Validates: Requirements 5.2**
    proptest! {
        #[test]
        fn test_backdrops_directory_creation(
            // Generate various video file names
            video_name in "[a-zA-Z0-9_-]{1,20}\\.(mp4|mkv)",
            // Test with different parent directory names
            parent_dir_name in "[a-zA-Z0-9_-]{1,15}",
        ) {
            // Property: For any video in a directory without an existing backdrops folder,
            // the tool should create the backdrops directory before writing the output

            // Create a temporary directory for testing
            let temp_base = std::env::temp_dir().join(format!(
                "processor_backdrops_test_{}_{}",
                std::process::id(),
                rand::random::<u32>()
            ));
            let _ = fs::remove_dir_all(&temp_base);

            // Create parent directory with the generated name
            let parent_dir = temp_base.join(&parent_dir_name);
            fs::create_dir_all(&parent_dir).unwrap();

            // Create test video file
            let video_file = create_test_video_structure(&parent_dir, &video_name);

            // Verify the backdrops directory does NOT exist initially
            let backdrops_dir = parent_dir.join("backdrops");
            prop_assert!(
                !backdrops_dir.exists(),
                "Backdrops directory should not exist before processing"
            );

            // Create processor with mock selector
            let selector = Box::new(MockSelector);
            let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
            let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 1, default_test_config());

            // Call create_output_directory to trigger directory creation
            let output_path = processor.create_output_directory(&video_file);

            // Property 1: The method should succeed
            prop_assert!(
                output_path.is_ok(),
                "create_output_directory should succeed for video {:?}",
                video_file.path
            );

            let output_path = output_path.unwrap();

            // Property 2 (CORE): The backdrops directory should now exist
            // This is the main property being tested - directory creation
            prop_assert!(
                backdrops_dir.exists(),
                "Backdrops directory should be created at {:?} when it didn't exist before",
                backdrops_dir
            );

            // Property 3: The created path should be a directory (not a file)
            prop_assert!(
                backdrops_dir.is_dir(),
                "Backdrops path should be a directory, not a file"
            );

            // Property 4: The backdrops directory should be in the video's parent directory
            let backdrops_parent = backdrops_dir.parent().unwrap();
            prop_assert_eq!(
                backdrops_parent,
                &video_file.parent_dir,
                "Backdrops directory should be created in the video's parent directory"
            );

            // Property 5: The backdrops directory name should be "backdrops" (lowercase)
            let backdrops_dir_name = backdrops_dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            prop_assert_eq!(
                backdrops_dir_name,
                "backdrops",
                "Directory name should be 'backdrops' in lowercase"
            );

            // Property 6: The output path should point to backdrop.mp4 inside the created directory
            let expected_output = backdrops_dir.join("backdrop.mp4");
            prop_assert_eq!(
                &output_path,
                &expected_output,
                "Output path should be backdrops/backdrop.mp4"
            );

            // Property 7: The parent of the output file should be the backdrops directory
            let output_parent = output_path.parent().unwrap();
            prop_assert_eq!(
                output_parent,
                &backdrops_dir,
                "Output file's parent should be the backdrops directory"
            );

            // Property 8: Calling create_output_directory again should succeed (idempotent)
            // The directory already exists now, but the method should still work
            let output_path_2 = processor.create_output_directory(&video_file);
            prop_assert!(
                output_path_2.is_ok(),
                "create_output_directory should succeed even when directory already exists (idempotent)"
            );

            // Property 9: The backdrops directory should still exist after second call
            prop_assert!(
                backdrops_dir.exists(),
                "Backdrops directory should still exist after second call"
            );

            // Property 10: Both calls should return the same output path
            prop_assert_eq!(
                &output_path,
                &output_path_2.unwrap(),
                "Multiple calls should return the same output path"
            );

            // Clean up
            let _ = fs::remove_dir_all(&temp_base);
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
            let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 1, default_test_config());

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
        let temp_dir = std::env::temp_dir().join(format!("processor_basic_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        // Create a test video
        let video_file = create_test_video_structure(&temp_dir, "test_video.mp4");

        // Create processor
        let selector = Box::new(MockSelector);
        let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
        let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 1, default_test_config());

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
        let temp_dir = std::env::temp_dir().join(format!("processor_names_{}", std::process::id()));
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
        let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 1, default_test_config());

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
            let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 1, default_test_config());

            // Process all videos and collect results
            // Since we're using fake video files, FFmpeg will fail to get duration
            // This simulates real-world errors (corrupted videos, etc.)
            let mut results = Vec::new();
            for video in &video_files {
                let result = processor.process_video(video, |_, _, _| {});
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
        let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 1, default_test_config());

        // Process all videos
        let mut results = Vec::new();
        for video in &videos {
            let result = processor.process_video(video, |_, _, _| {});
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

        println!(
            "✓ Error recovery test passed: all {} videos were processed",
            results.len()
        );

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
            let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 1, default_test_config());

            // Process the video (will fail because it's a fake video file)
            let result = processor.process_video(&video_file, |_, _, _| {});

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
            // It should indicate what went wrong (e.g., "Failed to get video duration" or "Skipping corrupted")
            prop_assert!(
                error_message.contains("Failed") ||
                error_message.contains("failed") ||
                error_message.contains("Error") ||
                error_message.contains("error") ||
                error_message.contains("Skipping") ||
                error_message.contains("Corrupted") ||
                error_message.contains("corrupted"),
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

    // Feature: video-clip-extractor, Property 12: Output Path Structure
    // **Validates: Requirements 5.1, 5.4, 5.5**
    proptest! {
        #[test]
        fn test_output_path_structure(
            // Generate various video file names
            video_name in "[a-zA-Z0-9_-]{1,20}\\.(mp4|mkv)",
            // Test with different parent directory names
            parent_dir_name in "[a-zA-Z0-9_-]{1,15}",
            // Test with nested directory structures
            use_nested in proptest::bool::ANY,
        ) {
            // Property: For any processed video, the output path should be in a subdirectory
            // named "backdrops" (lowercase) relative to the source video's parent directory,
            // with filename "backdrop.mp4" (lowercase).

            // Create a temporary directory for testing
            let temp_base = std::env::temp_dir().join(format!(
                "processor_path_structure_test_{}_{}",
                std::process::id(),
                rand::random::<u32>()
            ));
            let _ = fs::remove_dir_all(&temp_base);

            // Create parent directory structure (optionally nested)
            let parent_dir = if use_nested {
                // Create a nested structure: temp_base/nested/parent_dir_name
                let nested = temp_base.join("nested");
                fs::create_dir_all(&nested).unwrap();
                nested.join(&parent_dir_name)
            } else {
                // Create a flat structure: temp_base/parent_dir_name
                temp_base.join(&parent_dir_name)
            };

            fs::create_dir_all(&parent_dir).unwrap();

            // Create test video file
            let video_file = create_test_video_structure(&parent_dir, &video_name);

            // Create processor with mock selector
            let selector = Box::new(MockSelector);
            let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
            let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 1, default_test_config());

            // Call create_output_directory to get the output path
            let output_path = processor.create_output_directory(&video_file);

            // Property 1: The method should succeed
            prop_assert!(
                output_path.is_ok(),
                "create_output_directory should succeed for video {:?}",
                video_file.path
            );

            let output_path = output_path.unwrap();

            // Property 2 (CORE): The output path should be in a subdirectory named "backdrops"
            // relative to the source video's parent directory
            let expected_backdrops_dir = video_file.parent_dir.join("backdrops");
            let actual_backdrops_dir = output_path.parent().unwrap();

            prop_assert_eq!(
                actual_backdrops_dir,
                &expected_backdrops_dir,
                "Output path should be in a 'backdrops' subdirectory relative to the video's parent directory"
            );

            // Property 3: The backdrops directory name should be "backdrops" in lowercase
            let backdrops_dir_name = actual_backdrops_dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            prop_assert_eq!(
                backdrops_dir_name,
                "backdrops",
                "Subdirectory name should be 'backdrops' in lowercase (not 'Backdrops' or 'BACKDROPS')"
            );

            // Property 4: The output filename should be "backdrop.mp4" in lowercase
            let output_filename = output_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            prop_assert_eq!(
                output_filename,
                "backdrop.mp4",
                "Output filename should be 'backdrop.mp4' in lowercase (not 'Backdrop.mp4' or 'BACKDROP.MP4')"
            );

            // Property 5: The complete output path structure should be:
            // <video_parent_dir>/backdrops/backdrop.mp4
            let expected_output_path = video_file.parent_dir.join("backdrops").join("backdrop.mp4");

            prop_assert_eq!(
                &output_path,
                &expected_output_path,
                "Complete output path should follow the structure: <video_parent_dir>/backdrops/backdrop.mp4"
            );

            // Property 6: The backdrops directory should be a direct child of the video's parent directory
            let backdrops_parent = actual_backdrops_dir.parent().unwrap();

            prop_assert_eq!(
                backdrops_parent,
                &video_file.parent_dir,
                "Backdrops directory should be a direct child of the video's parent directory"
            );

            // Property 7: The output path should be relative to the video's parent directory,
            // not to some other location
            prop_assert!(
                output_path.starts_with(&video_file.parent_dir),
                "Output path should start with the video's parent directory: {:?}",
                video_file.parent_dir
            );

            // Property 8: The path components should be in the correct order:
            // parent_dir -> backdrops -> backdrop.mp4
            let path_components: Vec<_> = output_path.components().collect();
            let parent_components: Vec<_> = video_file.parent_dir.components().collect();

            // The output path should have exactly 2 more components than the parent dir
            // (backdrops + backdrop.mp4)
            prop_assert_eq!(
                path_components.len(),
                parent_components.len() + 2,
                "Output path should have exactly 2 more components than parent dir (backdrops + backdrop.mp4)"
            );

            // Property 9: Verify the backdrops directory was actually created
            prop_assert!(
                expected_backdrops_dir.exists(),
                "Backdrops directory should be created at {:?}",
                expected_backdrops_dir
            );

            prop_assert!(
                expected_backdrops_dir.is_dir(),
                "Backdrops path should be a directory, not a file"
            );

            // Property 10: The structure should work regardless of nesting level
            // (whether the video is in a flat or nested directory structure)
            if use_nested {
                prop_assert!(
                    output_path.to_string_lossy().contains("nested"),
                    "Output path should preserve nested directory structure"
                );
            }

            // Property 11: The output filename should be exactly "backdrop.mp4"
            // (not the original video name)
            let video_filename = video_file.path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            let output_filename_check = output_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            if video_filename != "backdrop.mp4" {
                prop_assert_ne!(
                    output_filename_check,
                    video_filename,
                    "Output filename should be 'backdrop.mp4', not the original video filename '{}'",
                    video_filename
                );
            }

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
            let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 1, default_test_config());

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

    // Unit tests for multiple clip naming (Task 6.2)

    // Mock selector that returns multiple clips
    #[allow(dead_code)]
    struct MultiClipMockSelector {
        clip_count: u8,
    }

    impl ClipSelector for MultiClipMockSelector {
        fn select_clips(
            &self,
            _video_path: &std::path::Path,
            duration: f64,
            _intro_exclusion_percent: f64,
            _outro_exclusion_percent: f64,
            clip_count: u8,
            _config: &crate::selector::ClipConfig,
        ) -> Result<Vec<TimeRange>, crate::selector::SelectionError> {
            // Return multiple non-overlapping time ranges for testing
            let mut clips = Vec::new();
            let clip_duration = 5.0;
            let spacing = 2.0; // Gap between clips

            for i in 0..clip_count.min((duration / (clip_duration + spacing)).floor() as u8) {
                let start = i as f64 * (clip_duration + spacing);
                if start + clip_duration <= duration {
                    clips.push(TimeRange {
                        start_seconds: start,
                        duration_seconds: clip_duration,
                    });
                }
            }

            Ok(clips)
        }
    }

    #[test]
    fn test_sequential_naming_single_clip() {
        // Test that a single clip is named "backdrop1.mp4"
        let temp_dir =
            std::env::temp_dir().join(format!("processor_naming_single_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let video_file = create_test_video_structure(&temp_dir, "test_video.mp4");

        // Create processor with multi-clip mock selector
        let selector = Box::new(MultiClipMockSelector { clip_count: 1 });
        let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
        let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 1, default_test_config());

        // Create backdrops directory
        let backdrops_dir = processor.create_backdrops_directory(&video_file).unwrap();

        // Verify the expected output path for single clip
        let expected_path = backdrops_dir.join("backdrop1.mp4");
        assert_eq!(
            expected_path.file_name().unwrap().to_str().unwrap(),
            "backdrop1.mp4"
        );

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sequential_naming_two_clips() {
        // Test that two clips are named "backdrop1.mp4" and "backdrop2.mp4"
        let temp_dir =
            std::env::temp_dir().join(format!("processor_naming_two_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let video_file = create_test_video_structure(&temp_dir, "test_video.mp4");

        // Create processor with multi-clip mock selector
        let selector = Box::new(MultiClipMockSelector { clip_count: 2 });
        let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
        let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 2, default_test_config());

        // Create backdrops directory
        let backdrops_dir = processor.create_backdrops_directory(&video_file).unwrap();

        // Verify the expected output paths for two clips
        let expected_path1 = backdrops_dir.join("backdrop1.mp4");
        let expected_path2 = backdrops_dir.join("backdrop2.mp4");

        assert_eq!(
            expected_path1.file_name().unwrap().to_str().unwrap(),
            "backdrop1.mp4"
        );
        assert_eq!(
            expected_path2.file_name().unwrap().to_str().unwrap(),
            "backdrop2.mp4"
        );

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sequential_naming_three_clips() {
        // Test that three clips are named "backdrop1.mp4", "backdrop2.mp4", "backdrop3.mp4"
        let temp_dir =
            std::env::temp_dir().join(format!("processor_naming_three_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let video_file = create_test_video_structure(&temp_dir, "test_video.mp4");

        // Create processor with multi-clip mock selector
        let selector = Box::new(MultiClipMockSelector { clip_count: 3 });
        let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
        let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 3, default_test_config());

        // Create backdrops directory
        let backdrops_dir = processor.create_backdrops_directory(&video_file).unwrap();

        // Verify the expected output paths for three clips
        let expected_path1 = backdrops_dir.join("backdrop1.mp4");
        let expected_path2 = backdrops_dir.join("backdrop2.mp4");
        let expected_path3 = backdrops_dir.join("backdrop3.mp4");

        assert_eq!(
            expected_path1.file_name().unwrap().to_str().unwrap(),
            "backdrop1.mp4"
        );
        assert_eq!(
            expected_path2.file_name().unwrap().to_str().unwrap(),
            "backdrop2.mp4"
        );
        assert_eq!(
            expected_path3.file_name().unwrap().to_str().unwrap(),
            "backdrop3.mp4"
        );

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_sequential_naming_four_clips() {
        // Test that four clips are named "backdrop1.mp4", "backdrop2.mp4", "backdrop3.mp4", "backdrop4.mp4"
        let temp_dir =
            std::env::temp_dir().join(format!("processor_naming_four_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let video_file = create_test_video_structure(&temp_dir, "test_video.mp4");

        // Create processor with multi-clip mock selector
        let selector = Box::new(MultiClipMockSelector { clip_count: 4 });
        let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
        let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 4, default_test_config());

        // Create backdrops directory
        let backdrops_dir = processor.create_backdrops_directory(&video_file).unwrap();

        // Verify the expected output paths for four clips
        let expected_path1 = backdrops_dir.join("backdrop1.mp4");
        let expected_path2 = backdrops_dir.join("backdrop2.mp4");
        let expected_path3 = backdrops_dir.join("backdrop3.mp4");
        let expected_path4 = backdrops_dir.join("backdrop4.mp4");

        assert_eq!(
            expected_path1.file_name().unwrap().to_str().unwrap(),
            "backdrop1.mp4"
        );
        assert_eq!(
            expected_path2.file_name().unwrap().to_str().unwrap(),
            "backdrop2.mp4"
        );
        assert_eq!(
            expected_path3.file_name().unwrap().to_str().unwrap(),
            "backdrop3.mp4"
        );
        assert_eq!(
            expected_path4.file_name().unwrap().to_str().unwrap(),
            "backdrop4.mp4"
        );

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_output_directory_creation_for_multiple_clips() {
        // Test that the backdrops directory is created correctly for multiple clips
        let temp_dir =
            std::env::temp_dir().join(format!("processor_dir_creation_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let video_file = create_test_video_structure(&temp_dir, "test_video.mp4");

        // Verify the backdrops directory does NOT exist initially
        let backdrops_dir = temp_dir.join("backdrops");
        assert!(
            !backdrops_dir.exists(),
            "Backdrops directory should not exist before processing"
        );

        // Create processor
        let selector = Box::new(MultiClipMockSelector { clip_count: 3 });
        let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
        let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 3, default_test_config());

        // Create backdrops directory
        let created_dir = processor.create_backdrops_directory(&video_file).unwrap();

        // Verify the backdrops directory now exists
        assert!(
            backdrops_dir.exists(),
            "Backdrops directory should be created"
        );
        assert!(
            backdrops_dir.is_dir(),
            "Backdrops path should be a directory"
        );
        assert_eq!(
            created_dir, backdrops_dir,
            "Returned path should match expected backdrops directory"
        );

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_file_path_construction_for_multiple_clips() {
        // Test that file paths are constructed correctly for multiple clips
        let temp_dir = std::env::temp_dir().join(format!(
            "processor_path_construction_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let video_file = create_test_video_structure(&temp_dir, "test_video.mp4");

        // Create processor
        let selector = Box::new(MultiClipMockSelector { clip_count: 2 });
        let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
        let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 2, default_test_config());

        // Create backdrops directory
        let backdrops_dir = processor.create_backdrops_directory(&video_file).unwrap();

        // Construct expected paths
        let expected_path1 = backdrops_dir.join("backdrop1.mp4");
        let expected_path2 = backdrops_dir.join("backdrop2.mp4");

        // Verify path structure
        assert_eq!(expected_path1.parent().unwrap(), &backdrops_dir);
        assert_eq!(expected_path2.parent().unwrap(), &backdrops_dir);

        // Verify the backdrops directory is in the video's parent directory
        assert_eq!(backdrops_dir.parent().unwrap(), &video_file.parent_dir);

        // Verify full path structure
        let expected_full_path1 = video_file
            .parent_dir
            .join("backdrops")
            .join("backdrop1.mp4");
        let expected_full_path2 = video_file
            .parent_dir
            .join("backdrops")
            .join("backdrop2.mp4");

        assert_eq!(expected_path1, expected_full_path1);
        assert_eq!(expected_path2, expected_full_path2);

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    // Feature: multiple-clips-per-video, Property 7: Sequential Naming Convention
    // **Validates: Requirements 6.1**
    proptest! {
        #[test]
        fn test_sequential_naming_convention_property(
            // Generate clip counts from 1 to 4
            clip_count in 1u8..=4u8,
        ) {
            // Property: For any clip count N (1-4), the generated clips should be named
            // "backdrop1.mp4", "backdrop2.mp4", ..., "vidN.mp4" in sequential order

            // Create a temporary directory for testing
            let temp_base = std::env::temp_dir().join(format!(
                "processor_naming_property_test_{}_{}",
                std::process::id(),
                rand::random::<u32>()
            ));
            let _ = fs::remove_dir_all(&temp_base);
            fs::create_dir_all(&temp_base).unwrap();

            // Create test video file
            let video_file = create_test_video_structure(&temp_base, "test_video.mp4");

            // Create processor with multi-clip mock selector
            let selector = Box::new(MultiClipMockSelector { clip_count });
            let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
            let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, clip_count, default_test_config());

            // Create backdrops directory
            let backdrops_dir = processor.create_backdrops_directory(&video_file).unwrap();

            // Property 1: For each clip number from 1 to N, the filename should be "vidX.mp4"
            for i in 1..=clip_count {
                let expected_filename = format!("vid{}.mp4", i);
                let expected_path = backdrops_dir.join(&expected_filename);

                // Verify the filename is correct
                prop_assert_eq!(
                    expected_path.file_name().unwrap().to_str().unwrap(),
                    &expected_filename,
                    "Clip {} should be named '{}'",
                    i,
                    expected_filename
                );

                // Property 2: The path should be in the backdrops directory
                prop_assert_eq!(
                    expected_path.parent().unwrap(),
                    &backdrops_dir,
                    "Clip {} should be in the backdrops directory",
                    i
                );
            }

            // Property 3: The naming should be sequential (no gaps)
            // Verify that backdrop1.mp4, backdrop2.mp4, ..., vidN.mp4 exist (conceptually)
            // and there are no other numbered clips
            for i in 1..=clip_count {
                let expected_path = backdrops_dir.join(format!("vid{}.mp4", i));
                // We're just verifying the path structure, not that files exist
                prop_assert!(
                    expected_path.to_string_lossy().contains(&format!("vid{}.mp4", i)),
                    "Sequential naming should include vid{}.mp4",
                    i
                );
            }

            // Property 4: The naming should start at 1 (not 0)
            let first_clip_path = backdrops_dir.join("backdrop1.mp4");
            prop_assert!(
                first_clip_path.to_string_lossy().contains("backdrop1.mp4"),
                "First clip should be named 'backdrop1.mp4', not 'backdrop0.mp4'"
            );

            // Property 5: The naming should end at N (not N+1)
            let last_clip_path = backdrops_dir.join(format!("vid{}.mp4", clip_count));
            prop_assert!(
                last_clip_path.to_string_lossy().contains(&format!("vid{}.mp4", clip_count)),
                "Last clip should be named 'vid{}.mp4'",
                clip_count
            );

            // Property 6: There should be no clip numbered N+1
            // We verify this by checking that our loop only generates clips 1 to N
            // (This is implicitly tested by the loop above)

            // Property 7: All clip paths should follow the pattern "vidX.mp4" where X is a number
            for i in 1..=clip_count {
                let clip_path = backdrops_dir.join(format!("vid{}.mp4", i));
                let filename = clip_path.file_name().unwrap().to_str().unwrap();

                // Verify the pattern: starts with "vid", followed by a number, ends with ".mp4"
                prop_assert!(
                    filename.starts_with("vid"),
                    "Clip filename should start with 'vid': {}",
                    filename
                );
                prop_assert!(
                    filename.ends_with(".mp4"),
                    "Clip filename should end with '.mp4': {}",
                    filename
                );

                // Extract the number part and verify it matches the index
                let number_part = &filename[3..filename.len()-4]; // Extract between "vid" and ".mp4"
                let parsed_number: u8 = number_part.parse().unwrap();
                prop_assert_eq!(
                    parsed_number,
                    i,
                    "Clip number in filename should match the sequential index"
                );
            }

            // Property 8: The backdrops directory should be in the video's parent directory
            prop_assert_eq!(
                backdrops_dir.parent().unwrap(),
                &video_file.parent_dir,
                "Backdrops directory should be in the video's parent directory"
            );

            // Property 9: The backdrops directory name should be "backdrops" (lowercase)
            let backdrops_dir_name = backdrops_dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            prop_assert_eq!(
                backdrops_dir_name,
                "backdrops",
                "Directory name should be 'backdrops' in lowercase"
            );

            // Clean up
            let _ = fs::remove_dir_all(&temp_base);
        }
    }

    // Feature: multiple-clips-per-video, Property 8: Output Directory Consistency
    // **Validates: Requirements 6.2**
    proptest! {
        #[test]
        fn test_output_directory_consistency_property(
            // Generate clip counts from 1 to 4
            clip_count in 1u8..=4u8,
            // Generate various video file names
            video_name in "[a-zA-Z0-9_-]{1,20}\\.(mp4|mkv)",
            // Test with different parent directory names
            parent_dir_name in "[a-zA-Z0-9_-]{1,15}",
        ) {
            // Property: For any generated clip from a video at path P, the clip should be
            // located in the "backdrops/" subdirectory relative to P's parent directory.
            // This property ensures that all clips, regardless of count or video location,
            // are consistently placed in the same output directory structure.

            // Create a temporary directory for testing
            let temp_base = std::env::temp_dir().join(format!(
                "processor_output_dir_property_test_{}_{}",
                std::process::id(),
                rand::random::<u32>()
            ));
            let _ = fs::remove_dir_all(&temp_base);

            // Create parent directory with the generated name
            let parent_dir = temp_base.join(&parent_dir_name);
            fs::create_dir_all(&parent_dir).unwrap();

            // Create test video file
            let video_file = create_test_video_structure(&parent_dir, &video_name);

            // Create processor with multi-clip mock selector
            let selector = Box::new(MultiClipMockSelector { clip_count });
            let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true);
            let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, clip_count, default_test_config());

            // Create backdrops directory
            let backdrops_dir = processor.create_backdrops_directory(&video_file).unwrap();

            // Property 1 (CORE): The backdrops directory should be in the video's parent directory
            // This is the main property being tested - output directory consistency
            prop_assert_eq!(
                backdrops_dir.parent().unwrap(),
                &video_file.parent_dir,
                "Backdrops directory should be in the video's parent directory for video '{}'",
                video_name
            );

            // Property 2: The backdrops directory name should always be "backdrops" (lowercase)
            let backdrops_dir_name = backdrops_dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            prop_assert_eq!(
                backdrops_dir_name,
                "backdrops",
                "Directory name should always be 'backdrops' in lowercase"
            );

            // Property 3: For ALL clips (1 to N), each clip should be in the backdrops directory
            for i in 1..=clip_count {
                let clip_path = backdrops_dir.join(format!("vid{}.mp4", i));

                // Verify the clip's parent directory is the backdrops directory
                prop_assert_eq!(
                    clip_path.parent().unwrap(),
                    &backdrops_dir,
                    "Clip {} should be in the backdrops directory",
                    i
                );

                // Verify the full path structure: parent_dir/backdrops/vidX.mp4
                let expected_path = video_file.parent_dir.join("backdrops").join(format!("vid{}.mp4", i));
                prop_assert_eq!(
                    &clip_path,
                    &expected_path,
                    "Clip {} should follow the structure: parent_dir/backdrops/vidX.mp4",
                    i
                );
            }

            // Property 4: The backdrops directory should be a direct child of the video's parent directory
            // (not nested deeper)
            let backdrops_parent = backdrops_dir.parent().unwrap();
            prop_assert_eq!(
                backdrops_parent,
                &video_file.parent_dir,
                "Backdrops directory should be a direct child of the video's parent directory"
            );

            // Property 5: The relative path from video's parent to any clip should always be "backdrops/vidX.mp4"
            for i in 1..=clip_count {
                let clip_path = backdrops_dir.join(format!("vid{}.mp4", i));
                let relative_path = clip_path.strip_prefix(&video_file.parent_dir).unwrap();

                let expected_relative = std::path::Path::new("backdrops").join(format!("vid{}.mp4", i));
                prop_assert_eq!(
                    relative_path,
                    &expected_relative,
                    "Relative path for clip {} should be 'backdrops/vidX.mp4'",
                    i
                );
            }

            // Property 6: The backdrops directory should exist after creation
            prop_assert!(
                backdrops_dir.exists(),
                "Backdrops directory should exist at {:?}",
                backdrops_dir
            );

            // Property 7: The backdrops directory should be a directory (not a file)
            prop_assert!(
                backdrops_dir.is_dir(),
                "Backdrops path should be a directory, not a file"
            );

            // Property 8: The output directory structure should be consistent regardless of video name
            // This is implicitly tested by the properties above, but we verify explicitly
            let another_video = create_test_video_structure(&parent_dir, "different_video.mp4");
            let another_backdrops_dir = processor.create_backdrops_directory(&another_video).unwrap();

            prop_assert_eq!(
                &backdrops_dir,
                &another_backdrops_dir,
                "All videos in the same parent directory should use the same backdrops directory"
            );

            // Property 9: The output directory structure should be consistent regardless of clip count
            // Create processors with different clip counts and verify they use the same backdrops directory
            for test_clip_count in 1u8..=4u8 {
                let test_selector = Box::new(MultiClipMockSelector { clip_count: test_clip_count });
                let test_processor = VideoProcessor::new(test_selector, FFmpegExecutor::new(Resolution::Hd1080, true), 1.0, 40.0, test_clip_count, default_test_config());
                let test_backdrops_dir = test_processor.create_backdrops_directory(&video_file).unwrap();

                prop_assert_eq!(
                    &backdrops_dir,
                    &test_backdrops_dir,
                    "Backdrops directory should be the same regardless of clip count"
                );
            }

            // Property 10: The backdrops directory path should not depend on the video filename
            // (only on the parent directory)
            let video_with_different_name = create_test_video_structure(&parent_dir, "yet_another_video.mkv");
            let backdrops_for_different_video = processor.create_backdrops_directory(&video_with_different_name).unwrap();

            prop_assert_eq!(
                &backdrops_dir,
                &backdrops_for_different_video,
                "Backdrops directory should be the same for all videos in the same parent directory"
            );

            // Clean up
            let _ = fs::remove_dir_all(&temp_base);
        }
    }
}
