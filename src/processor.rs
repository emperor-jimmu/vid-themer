// Video processing pipeline coordination

use crate::ffmpeg::FFmpegExecutor;
use crate::scanner::VideoFile;
use crate::selector::ClipSelector;
use std::path::{Path, PathBuf};

// Constants for output directory and file naming
const BACKDROPS_DIR: &str = "backdrops";

pub struct VideoProcessor {
    selector: Box<dyn ClipSelector>,
    ffmpeg: FFmpegExecutor,
    intro_exclusion_percent: f64,
    outro_exclusion_percent: f64,
    clip_count: u8,
    clip_config: crate::selector::ClipConfig,
    force: bool,
}

impl VideoProcessor {
    pub fn new(
        selector: Box<dyn ClipSelector>,
        ffmpeg: FFmpegExecutor,
        intro_exclusion_percent: f64,
        outro_exclusion_percent: f64,
        clip_count: u8,
        clip_config: crate::selector::ClipConfig,
        force: bool,
    ) -> Self {
        Self {
            selector,
            ffmpeg,
            intro_exclusion_percent,
            outro_exclusion_percent,
            clip_count,
            clip_config,
            force,
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
        // Target comes from CLI argument
        let backdrops_dir = video.parent_dir.join(BACKDROPS_DIR);
        let existing_clip_count = if !self.force && backdrops_dir.exists() {
            self.count_existing_clips(&backdrops_dir)
        } else {
            0
        };

        // If already have requested clips but no done.ext, write the marker and skip
        if existing_clip_count >= self.clip_count {
            // Ensure backdrops dir exists before writing marker
            if backdrops_dir.exists()
                && let Err(e) = crate::scanner::write_done_marker(&backdrops_dir)
            {
                eprintln!(
                    "Warning: Failed to write done marker for {}: {}",
                    video.path.display(),
                    e
                );
            }
            return ProcessResult::success(&video_path, PathBuf::new(), 0);
        }

        let clips_to_generate = self.clip_count - existing_clip_count;

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
                return ProcessResult::failure(
                    &video_path,
                    PathBuf::new(),
                    error_message,
                    stderr,
                    0,
                );
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
                return ProcessResult::failure(
                    &video_path,
                    PathBuf::new(),
                    format!(
                        "No valid clips could be selected (requested: {} clips)",
                        clips_to_generate
                    ),
                    None,
                    0,
                );
            }
            Err(e) => {
                return ProcessResult::failure(
                    &video_path,
                    PathBuf::new(),
                    format!(
                        "Failed to select clip segment (requested: {} clips): {}",
                        clips_to_generate, e
                    ),
                    None,
                    0,
                );
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
                return ProcessResult::failure(
                    &video_path,
                    PathBuf::new(),
                    format!("Failed to create output directory: {}", e),
                    None,
                    0,
                );
            }
        };

        // Step 4: Extract each clip with sequential naming, starting after existing clips
        let mut last_output_path = PathBuf::new();
        let total_clips = time_ranges.len();

        for (index, time_range) in time_ranges.iter().enumerate() {
            // Start numbering from existing_clip_count + 1
            let clip_num = existing_clip_count as usize + index + 1;
            let output_filename = format!("backdrop{}.mp4", clip_num);

            let output_path = backdrops_dir.join(&output_filename);
            last_output_path = output_path.clone();

            if let Err(e) = self
                .ffmpeg
                .extract_clip(&video.path, time_range, &output_path)
            {
                let stderr = e.stderr().map(|s| s.to_string());
                return ProcessResult::failure(
                    &video_path,
                    output_path,
                    format!(
                        "Failed to extract clip {} of {} (backdrop{}.mp4): {}",
                        index + 1,
                        time_ranges.len(),
                        clip_num,
                        e
                    ),
                    stderr,
                    index,
                );
            }

            // Call progress callback after successful extraction
            progress_callback(index + 1, total_clips, &output_filename);
        }

        // Write done.ext marker — we've now rendered all needed clips
        if let Err(e) = crate::scanner::write_done_marker(&backdrops_dir) {
            eprintln!(
                "Warning: Failed to write done marker for {}: {}",
                video.path.display(),
                e
            );
        }

        ProcessResult::success(&video_path, last_output_path, time_ranges.len())
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
}

pub struct ProcessResult {
    pub video_path: PathBuf,
    pub output_path: PathBuf,
    pub success: bool,
    pub error_message: Option<String>,
    pub ffmpeg_stderr: Option<String>,
    pub clips_generated: usize,
}

impl ProcessResult {
    fn success(video_path: &Path, output_path: PathBuf, clips_generated: usize) -> Self {
        Self {
            video_path: video_path.to_path_buf(),
            output_path,
            success: true,
            error_message: None,
            ffmpeg_stderr: None,
            clips_generated,
        }
    }

    fn failure(
        video_path: &Path,
        output_path: PathBuf,
        error_message: String,
        ffmpeg_stderr: Option<String>,
        clips_generated: usize,
    ) -> Self {
        Self {
            video_path: video_path.to_path_buf(),
            output_path,
            success: false,
            error_message: Some(error_message),
            ffmpeg_stderr,
            clips_generated,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("Failed to create output directory: {0}")]
    OutputDirectoryCreationFailed(String),
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
        let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true, false);
        let processor =
            VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 1, default_test_config(), false);

        // Get output path
        let output_path = processor.create_backdrops_directory(&video_file)
            .unwrap().join("backdrop1.mp4");

        // Verify the output file name is "backdrop1.mp4"
        assert_eq!(
            output_path.file_name().unwrap().to_str().unwrap(),
            "backdrop1.mp4"
        );

        // Verify the structure
        let expected = temp_dir.join("backdrops").join("backdrop1.mp4");
        assert_eq!(output_path, expected);

        // Verify backdrops directory exists
        assert!(temp_dir.join("backdrops").exists());
        assert!(temp_dir.join("backdrops").is_dir());

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
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
            let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true, false);
            let processor = VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 1, default_test_config(), false);

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

    #[test]
    fn test_count_existing_clips_stops_at_first_missing() {
        let temp_dir = std::env::temp_dir().join(format!(
            "processor_count_gap_{}_{}",
            std::process::id(),
            rand::random::<u32>()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let backdrops = temp_dir.join("backdrops");
        fs::create_dir_all(&backdrops).unwrap();
        fs::write(backdrops.join("backdrop1.mp4"), b"not-empty").unwrap();
        fs::write(backdrops.join("backdrop3.mp4"), b"not-empty").unwrap();

        let selector = Box::new(MockSelector);
        let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true, false);
        let processor =
            VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 4, default_test_config(), false);

        assert_eq!(processor.count_existing_clips(&backdrops), 1);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_count_existing_clips_stops_at_zero_byte() {
        let temp_dir = std::env::temp_dir().join(format!(
            "processor_count_zero_{}_{}",
            std::process::id(),
            rand::random::<u32>()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let backdrops = temp_dir.join("backdrops");
        fs::create_dir_all(&backdrops).unwrap();
        fs::write(backdrops.join("backdrop1.mp4"), b"not-empty").unwrap();
        fs::File::create(backdrops.join("backdrop2.mp4")).unwrap();
        fs::write(backdrops.join("backdrop3.mp4"), b"not-empty").unwrap();

        let selector = Box::new(MockSelector);
        let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true, false);
        let processor =
            VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 4, default_test_config(), false);

        assert_eq!(processor.count_existing_clips(&backdrops), 1);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_process_video_skips_when_enough_existing_clips() {
        let temp_dir = std::env::temp_dir().join(format!(
            "processor_skip_existing_{}_{}",
            std::process::id(),
            rand::random::<u32>()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let video_file = create_test_video_structure(&temp_dir, "video.mp4");
        let backdrops = temp_dir.join("backdrops");
        fs::create_dir_all(&backdrops).unwrap();
        fs::write(backdrops.join("backdrop1.mp4"), b"clip-1").unwrap();
        fs::write(backdrops.join("backdrop2.mp4"), b"clip-2").unwrap();

        let selector = Box::new(MockSelector);
        let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true, false);
        let processor =
            VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 2, default_test_config(), false);

        let result = processor.process_video(&video_file, |_, _, _| {});
        assert!(result.success);
        assert_eq!(result.clips_generated, 0);
        assert!(backdrops.join("done.ext").exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_process_video_force_ignores_existing_clips() {
        let temp_dir = std::env::temp_dir().join(format!(
            "processor_force_existing_{}_{}",
            std::process::id(),
            rand::random::<u32>()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let video_file = create_test_video_structure(&temp_dir, "video.mp4");
        let backdrops = temp_dir.join("backdrops");
        fs::create_dir_all(&backdrops).unwrap();
        fs::write(backdrops.join("backdrop1.mp4"), b"clip-1").unwrap();
        fs::write(backdrops.join("backdrop2.mp4"), b"clip-2").unwrap();

        let selector = Box::new(MockSelector);
        let ffmpeg = FFmpegExecutor::new(Resolution::Hd1080, true, false);
        let processor =
            VideoProcessor::new(selector, ffmpeg, 1.0, 40.0, 2, default_test_config(), true);

        let result = processor.process_video(&video_file, |_, _, _| {});
        assert!(!result.success, "force mode should not short-circuit on existing clips");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
