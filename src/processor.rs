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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Resolution;
    use crate::selector::{ClipSelector, TimeRange};
    use proptest::prelude::*;
    use std::fs;
    use std::io::Write;

    // Mock selector for testing
    struct MockSelector;

    impl ClipSelector for MockSelector {
        fn select_segment(
            &self,
            _video_path: &std::path::Path,
            duration: f64,
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
            let processor = VideoProcessor::new(selector, ffmpeg);
            
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
        let processor = VideoProcessor::new(selector, ffmpeg);
        
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
        let processor = VideoProcessor::new(selector, ffmpeg);
        
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
            let processor = VideoProcessor::new(selector, ffmpeg);
            
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
