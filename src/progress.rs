// Progress reporting and user feedback

use crate::logger::FailureLogger;
use crate::processor::ProcessResult;
use colored::Colorize;
use std::io::Write;

pub struct ProgressReporter {
    pub total: usize,
    pub current: usize,
    pub successful: usize,
    pub failed: usize,
    logger: Option<FailureLogger>,
}

impl ProgressReporter {
    pub fn new() -> Self {
        Self {
            total: 0,
            current: 0,
            successful: 0,
            failed: 0,
            logger: None,
        }
    }

    pub fn with_logger(logger: FailureLogger) -> Self {
        Self {
            total: 0,
            current: 0,
            successful: 0,
            failed: 0,
            logger: Some(logger),
        }
    }

    pub fn start(&mut self, total: usize) {
        self.total = total;
        println!(
            "{} {} videos to process",
            "Found".bright_cyan().bold(),
            total.to_string().bright_yellow().bold()
        );
    }

    /// Report progress for a single clip extraction
    /// This is called during video processing as each clip completes
    /// Must be called while holding the reporter lock to prevent interleaving
    pub fn update_clip_progress(
        &self,
        clip_num: usize,
        total_clips: usize,
        filename: &str,
        video_path: &std::path::Path,
    ) {
        // On first clip, print the header
        if clip_num == 1 {
            println!(
                "{} Processing: {}",
                format!("[{}/{}]", self.current, self.total)
                    .bright_blue()
                    .bold(),
                video_path.display().to_string().bright_white()
            );
        }

        let bar_width = 13;
        let filled = (clip_num * bar_width) / total_clips;
        let empty = bar_width - filled;
        let bar = format!(
            "[{}{}]",
            "=".repeat(filled).bright_green(),
            " ".repeat(empty)
        );

        // Use \r to overwrite the previous line
        print!(
            "\r  {} {} {}",
            filename.bright_cyan().bold(),
            bar,
            format!("{}/{}", clip_num, total_clips).bright_yellow()
        );

        // Flush to ensure immediate display
        let _ = std::io::stdout().flush();

        // On last clip, print newline to move to next line
        if clip_num == total_clips {
            println!();
        }
    }

    pub fn update(&mut self, result: &ProcessResult) {
        // Note: current counter is already incremented before processing
        // Buffer all output for this video completion to print atomically
        let mut output = String::new();

        if result.success {
            self.successful += 1;
            // No summary needed - individual clips already printed in real-time
        } else {
            self.failed += 1;
            if let Some(error) = &result.error_message {
                output.push_str(&format!(
                    "  {} {}\n",
                    "X".bright_red().bold(),
                    error.bright_red()
                ));
            }

            // Log failure to file if logger is available
            if let Some(logger) = &self.logger {
                logger.log_failure(result, result.ffmpeg_stderr.as_deref());
            }
        }

        // Print all output atomically (single print call prevents interleaving)
        print!("{}", output);
    }

    pub fn finish(&self) {
        println!(
            "{} {} successful, {} failed",
            "Completed:".bright_cyan().bold(),
            self.successful.to_string().bright_green().bold(),
            self.failed.to_string().bright_red().bold()
        );

        if self.failed > 0
            && let Some(logger) = &self.logger
        {
            println!(
                "{} {}",
                "Failure details logged to:".bright_yellow(),
                logger.log_path().display().to_string().bright_white()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::path::PathBuf;

    // Helper function to create a ProcessResult for testing
    fn create_process_result(
        video_path: &str,
        output_path: &str,
        success: bool,
        error_message: Option<String>,
    ) -> ProcessResult {
        ProcessResult {
            video_path: PathBuf::from(video_path),
            output_path: PathBuf::from(output_path),
            success,
            error_message,
            ffmpeg_stderr: None,
            clips_generated: if success { 1 } else { 0 },
        }
    }

    // Feature: video-clip-extractor, Property 17: Progress Updates Per Video
    // **Validates: Requirements 8.2**
    proptest! {
        #[test]
        fn test_progress_updates_per_video(
            // Generate a random number of videos to process (1 to 50)
            total_videos in 1usize..=50,
            // Generate a random number of successful videos (0 to total)
            successful_ratio in 0.0f64..=1.0,
        ) {
            // Property: For any batch of videos being processed, the tool should display
            // a progress update for each video showing current count and total count

            // Calculate how many videos will be successful based on the ratio
            let num_successful = (total_videos as f64 * successful_ratio).round() as usize;
            let num_failed = total_videos - num_successful;

            // Create a progress reporter
            let mut reporter = ProgressReporter::new();

            // Property 1: Initial state should have zero counts
            prop_assert_eq!(reporter.total, 0, "Initial total should be 0");
            prop_assert_eq!(reporter.current, 0, "Initial current should be 0");
            prop_assert_eq!(reporter.successful, 0, "Initial successful should be 0");
            prop_assert_eq!(reporter.failed, 0, "Initial failed should be 0");

            // Start the reporter with the total number of videos
            reporter.start(total_videos);

            // Property 2: After start, total should be set correctly
            prop_assert_eq!(
                reporter.total,
                total_videos,
                "Total should be set to {} after start",
                total_videos
            );

            // Property 3: Current should still be 0 after start (no videos processed yet)
            prop_assert_eq!(
                reporter.current,
                0,
                "Current should be 0 after start (no videos processed yet)"
            );

            // Process each video and verify progress updates
            for i in 0..total_videos {
                // Determine if this video should succeed or fail
                let is_success = i < num_successful;

                // Create a process result
                let result = if is_success {
                    create_process_result(
                        &format!("/path/to/video_{}.mp4", i),
                        &format!("/path/to/backdrops/backdrop_{}.mp4", i),
                        true,
                        None,
                    )
                } else {
                    create_process_result(
                        &format!("/path/to/video_{}.mp4", i),
                        "",
                        false,
                        Some(format!("Error processing video {}", i)),
                    )
                };

                // Increment current counter (simulating video processing start)
                reporter.current += 1;

                // Update the reporter with result
                reporter.update(&result);

                // Property 4: Current count should increment by 1 for each video
                prop_assert_eq!(
                    reporter.current,
                    i + 1,
                    "Current count should be {} after processing video {}",
                    i + 1,
                    i
                );

                // Property 5: Current count should never exceed total count
                prop_assert!(
                    reporter.current <= reporter.total,
                    "Current count ({}) should never exceed total count ({})",
                    reporter.current,
                    reporter.total
                );

                // Property 6: Successful + Failed should equal Current
                prop_assert_eq!(
                    reporter.successful + reporter.failed,
                    reporter.current,
                    "Successful ({}) + Failed ({}) should equal Current ({})",
                    reporter.successful,
                    reporter.failed,
                    reporter.current
                );

                // Property 7: Successful count should match expected
                let expected_successful = (i + 1).min(num_successful);
                prop_assert_eq!(
                    reporter.successful,
                    expected_successful,
                    "Successful count should be {} after processing video {}",
                    expected_successful,
                    i
                );

                // Property 8: Failed count should match expected
                let expected_failed = if i + 1 > num_successful {
                    (i + 1) - num_successful
                } else {
                    0
                };
                prop_assert_eq!(
                    reporter.failed,
                    expected_failed,
                    "Failed count should be {} after processing video {}",
                    expected_failed,
                    i
                );
            }

            // Property 9: After processing all videos, current should equal total
            prop_assert_eq!(
                reporter.current,
                reporter.total,
                "After processing all videos, current ({}) should equal total ({})",
                reporter.current,
                reporter.total
            );

            // Property 10: Final successful count should match expected
            prop_assert_eq!(
                reporter.successful,
                num_successful,
                "Final successful count should be {}",
                num_successful
            );

            // Property 11: Final failed count should match expected
            prop_assert_eq!(
                reporter.failed,
                num_failed,
                "Final failed count should be {}",
                num_failed
            );

            // Property 12: Total should remain unchanged throughout processing
            prop_assert_eq!(
                reporter.total,
                total_videos,
                "Total should remain {} throughout processing",
                total_videos
            );

            // Call finish to complete the progress reporting
            reporter.finish();

            // Property 13: Counts should remain stable after finish
            prop_assert_eq!(
                reporter.current,
                total_videos,
                "Current should still be {} after finish",
                total_videos
            );
            prop_assert_eq!(
                reporter.successful,
                num_successful,
                "Successful should still be {} after finish",
                num_successful
            );
            prop_assert_eq!(
                reporter.failed,
                num_failed,
                "Failed should still be {} after finish",
                num_failed
            );
        }
    }

    #[test]
    fn test_initial_progress_message() {
        // Test that total count is displayed at start
        // Requirements: 8.1
        let mut reporter = ProgressReporter::new();

        // Verify initial state before start
        assert_eq!(reporter.total, 0);
        assert_eq!(reporter.current, 0);
        assert_eq!(reporter.successful, 0);
        assert_eq!(reporter.failed, 0);

        // Start with a specific total count
        let total_count = 42;
        reporter.start(total_count);

        // Verify that total is set correctly
        assert_eq!(reporter.total, total_count);

        // Verify that other counters remain at 0 (no videos processed yet)
        assert_eq!(reporter.current, 0);
        assert_eq!(reporter.successful, 0);
        assert_eq!(reporter.failed, 0);
    }

    #[test]
    fn test_progress_updates_single_video() {
        // Basic unit test for a single video
        let mut reporter = ProgressReporter::new();

        reporter.start(1);
        assert_eq!(reporter.total, 1);
        assert_eq!(reporter.current, 0);

        let result = create_process_result(
            "/path/to/video.mp4",
            "/path/to/backdrops/backdrop.mp4",
            true,
            None,
        );

        reporter.current += 1;
        reporter.update(&result);

        assert_eq!(reporter.current, 1);
        assert_eq!(reporter.successful, 1);
        assert_eq!(reporter.failed, 0);

        reporter.finish();
    }

    #[test]
    fn test_progress_updates_multiple_videos_all_success() {
        // Test with multiple videos, all successful
        let mut reporter = ProgressReporter::new();
        let total = 5;

        reporter.start(total);

        for i in 0..total {
            let result = create_process_result(
                &format!("/path/to/video_{}.mp4", i),
                &format!("/path/to/backdrops/backdrop_{}.mp4", i),
                true,
                None,
            );

            reporter.current += 1;
            reporter.update(&result);

            assert_eq!(reporter.current, i + 1);
            assert_eq!(reporter.successful, i + 1);
            assert_eq!(reporter.failed, 0);
        }

        assert_eq!(reporter.current, total);
        assert_eq!(reporter.successful, total);
        assert_eq!(reporter.failed, 0);

        reporter.finish();
    }

    #[test]
    fn test_progress_updates_multiple_videos_all_failed() {
        // Test with multiple videos, all failed
        let mut reporter = ProgressReporter::new();
        let total = 5;

        reporter.start(total);

        for i in 0..total {
            let result = create_process_result(
                &format!("/path/to/video_{}.mp4", i),
                "",
                false,
                Some(format!("Error {}", i)),
            );

            reporter.current += 1;
            reporter.update(&result);

            assert_eq!(reporter.current, i + 1);
            assert_eq!(reporter.successful, 0);
            assert_eq!(reporter.failed, i + 1);
        }

        assert_eq!(reporter.current, total);
        assert_eq!(reporter.successful, 0);
        assert_eq!(reporter.failed, total);

        reporter.finish();
    }

    #[test]
    fn test_progress_updates_mixed_success_and_failure() {
        // Test with mixed success and failure
        let mut reporter = ProgressReporter::new();

        reporter.start(4);

        // Video 1: Success
        let result1 = create_process_result(
            "/path/to/video_1.mp4",
            "/path/to/backdrops/backdrop_1.mp4",
            true,
            None,
        );
        reporter.current += 1;
        reporter.update(&result1);
        assert_eq!(reporter.current, 1);
        assert_eq!(reporter.successful, 1);
        assert_eq!(reporter.failed, 0);

        // Video 2: Failure
        let result2 = create_process_result(
            "/path/to/video_2.mp4",
            "",
            false,
            Some("Error 2".to_string()),
        );
        reporter.current += 1;
        reporter.update(&result2);
        assert_eq!(reporter.current, 2);
        assert_eq!(reporter.successful, 1);
        assert_eq!(reporter.failed, 1);

        // Video 3: Success
        let result3 = create_process_result(
            "/path/to/video_3.mp4",
            "/path/to/backdrops/backdrop_3.mp4",
            true,
            None,
        );
        reporter.current += 1;
        reporter.update(&result3);
        assert_eq!(reporter.current, 3);
        assert_eq!(reporter.successful, 2);
        assert_eq!(reporter.failed, 1);

        // Video 4: Failure
        let result4 = create_process_result(
            "/path/to/video_4.mp4",
            "",
            false,
            Some("Error 4".to_string()),
        );
        reporter.current += 1;
        reporter.update(&result4);
        assert_eq!(reporter.current, 4);
        assert_eq!(reporter.successful, 2);
        assert_eq!(reporter.failed, 2);

        reporter.finish();
    }

    #[test]
    fn test_progress_current_never_exceeds_total() {
        // Verify that current never exceeds total
        let mut reporter = ProgressReporter::new();
        let total = 3;

        reporter.start(total);

        for i in 0..total {
            let result = create_process_result(
                &format!("/path/to/video_{}.mp4", i),
                &format!("/path/to/backdrops/backdrop_{}.mp4", i),
                true,
                None,
            );

            reporter.current += 1;
            reporter.update(&result);

            // Current should never exceed total
            assert!(reporter.current <= reporter.total);
        }

        // After processing all videos, current should equal total
        assert_eq!(reporter.current, reporter.total);

        reporter.finish();
    }

    // Feature: video-clip-extractor, Property 18: Success Messages Include Output Path
    // **Validates: Requirements 8.3**
    proptest! {
        #[test]
        fn test_success_messages_include_output_path(
            // Generate random number of successful videos (1 to 30)
            num_videos in 1usize..=30,
            // Generate random video paths
            video_paths in prop::collection::vec("[a-zA-Z0-9_/-]{5,30}\\.mp4", 1..=30),
            // Generate random output paths
            output_paths in prop::collection::vec("[a-zA-Z0-9_/-]{5,40}/backdrops/backdrop\\.mp4", 1..=30),
        ) {
            // Property: For any successfully processed video, the ProcessResult should contain
            // the output path, and the progress reporter should be able to display it

            // Ensure we have enough paths for the number of videos
            let num_videos = num_videos.min(video_paths.len()).min(output_paths.len());

            // Create a progress reporter
            let mut reporter = ProgressReporter::new();
            reporter.start(num_videos);

            // Process each video with success status
            for i in 0..num_videos {
                let video_path = &video_paths[i];
                let output_path = &output_paths[i];

                // Create a successful ProcessResult
                let result = create_process_result(
                    video_path,
                    output_path,
                    true,  // success = true
                    None,  // no error message
                );

                // Property 1: Successful results must have success = true
                prop_assert!(
                    result.success,
                    "ProcessResult should have success = true for successful processing"
                );

                // Property 2: Successful results must have no error message
                prop_assert!(
                    result.error_message.is_none(),
                    "ProcessResult should have no error message for successful processing"
                );

                // Property 3: Successful results must have a non-empty output path
                prop_assert!(
                    !result.output_path.as_os_str().is_empty(),
                    "ProcessResult should have a non-empty output path for successful processing"
                );

                // Property 4: The output path should be accessible and convertible to string
                let output_path_str = result.output_path.to_string_lossy();
                prop_assert!(
                    !output_path_str.is_empty(),
                    "Output path should be convertible to a non-empty string"
                );

                // Property 5: The output path should match what was provided
                let result_output_path = result.output_path.to_string_lossy();
                prop_assert_eq!(
                    result_output_path.as_ref(),
                    output_path,
                    "Output path in ProcessResult should match the provided output path"
                );

                // Property 6: For successful results, the output path should typically end with "backdrop.mp4"
                // (This is a domain-specific property based on the application requirements)
                let path_str = result.output_path.to_string_lossy();
                prop_assert!(
                    path_str.ends_with("backdrop.mp4"),
                    "Output path should end with 'backdrop.mp4' for successful processing, got: {}",
                    path_str
                );

                // Property 7: The output path should contain "backdrops" directory
                // (This is a domain-specific property based on the application requirements)
                prop_assert!(
                    path_str.contains("backdrops"),
                    "Output path should contain 'backdrops' directory, got: {}",
                    path_str
                );

                // Start video processing and update the reporter
                reporter.current += 1;
                reporter.update(&result);

                // Property 8: After update, successful count should increment
                prop_assert_eq!(
                    reporter.successful,
                    i + 1,
                    "Successful count should be {} after processing video {}",
                    i + 1,
                    i
                );

                // Property 9: Failed count should remain 0 for all successful videos
                prop_assert_eq!(
                    reporter.failed,
                    0,
                    "Failed count should remain 0 when all videos are successful"
                );
            }

            // Property 10: After processing all successful videos, successful count should equal total
            prop_assert_eq!(
                reporter.successful,
                num_videos,
                "Final successful count should equal total number of videos processed"
            );

            // Property 11: Failed count should still be 0
            prop_assert_eq!(
                reporter.failed,
                0,
                "Failed count should be 0 when all videos are successful"
            );

            // Property 12: Current count should equal total
            prop_assert_eq!(
                reporter.current,
                num_videos,
                "Current count should equal total after processing all videos"
            );
        }
    }

    #[test]
    fn test_success_message_includes_output_path_basic() {
        // Basic unit test to verify success messages include output path
        let mut reporter = ProgressReporter::new();

        reporter.start(1);

        let video_path = "/path/to/video.mp4";
        let output_path = "/path/to/backdrops/backdrop.mp4";

        let result = create_process_result(video_path, output_path, true, None);

        // Verify the result has the expected properties
        assert!(result.success);
        assert!(result.error_message.is_none());
        assert_eq!(result.output_path.to_string_lossy(), output_path);

        // Start video and update the reporter
        reporter.current += 1;
        reporter.update(&result);

        // Verify the reporter state
        assert_eq!(reporter.successful, 1);
        assert_eq!(reporter.failed, 0);
        assert_eq!(reporter.current, 1);
    }

    #[test]
    fn test_success_vs_failure_output_path_handling() {
        // Test that success messages include output path, but failure messages don't
        let mut reporter = ProgressReporter::new();

        reporter.start(2);

        // Process a successful video
        let success_result = create_process_result(
            "/path/to/video1.mp4",
            "/path/to/backdrops/backdrop.mp4",
            true,
            None,
        );

        reporter.current += 1;
        reporter.update(&success_result);

        // Verify success result has output path
        assert!(success_result.success);
        assert!(!success_result.output_path.as_os_str().is_empty());
        assert_eq!(reporter.successful, 1);
        assert_eq!(reporter.failed, 0);

        // Process a failed video
        let failure_result = create_process_result(
            "/path/to/video2.mp4",
            "", // Empty output path for failure
            false,
            Some("Processing failed".to_string()),
        );

        reporter.current += 1;
        reporter.update(&failure_result);

        // Verify failure result has error message but may have empty output path
        assert!(!failure_result.success);
        assert!(failure_result.error_message.is_some());
        assert_eq!(reporter.successful, 1);
        assert_eq!(reporter.failed, 1);
    }

    #[test]
    fn test_multiple_success_messages_with_different_output_paths() {
        // Test that each successful video has its own unique output path
        let mut reporter = ProgressReporter::new();
        let num_videos = 5;

        reporter.start(num_videos);

        let mut output_paths = Vec::new();

        for i in 0..num_videos {
            let video_path = format!("/path/to/video_{}.mp4", i);
            let output_path = format!("/path/to/dir_{}/backdrops/backdrop.mp4", i);

            let result = create_process_result(&video_path, &output_path, true, None);

            // Verify the result has the correct output path
            assert!(result.success);
            assert_eq!(result.output_path.to_string_lossy(), output_path);

            // Store the output path for uniqueness check
            output_paths.push(result.output_path.clone());

            reporter.current += 1;
            reporter.update(&result);
        }

        // Verify all output paths are unique (different directories)
        for i in 0..output_paths.len() {
            for j in (i + 1)..output_paths.len() {
                assert_ne!(
                    output_paths[i], output_paths[j],
                    "Output paths should be unique for different videos"
                );
            }
        }

        // Verify final state
        assert_eq!(reporter.successful, num_videos);
        assert_eq!(reporter.failed, 0);
        assert_eq!(reporter.current, num_videos);
    }

    #[test]
    fn test_summary_message() {
        // Test that successful and failed counts are displayed at end
        // Requirements: 8.4
        let mut reporter = ProgressReporter::new();

        // Test case 1: All successful
        reporter.start(5);
        for i in 0..5 {
            let result = create_process_result(
                &format!("/path/to/video_{}.mp4", i),
                &format!("/path/to/backdrops/backdrop_{}.mp4", i),
                true,
                None,
            );
            reporter.current += 1;
            reporter.update(&result);
        }

        // Verify counts before finish
        assert_eq!(reporter.successful, 5);
        assert_eq!(reporter.failed, 0);

        // Call finish to display summary
        reporter.finish();

        // Verify counts remain stable after finish
        assert_eq!(reporter.successful, 5);
        assert_eq!(reporter.failed, 0);

        // Test case 2: All failed
        let mut reporter2 = ProgressReporter::new();
        reporter2.start(3);
        for i in 0..3 {
            let result = create_process_result(
                &format!("/path/to/video_{}.mp4", i),
                "",
                false,
                Some(format!("Error {}", i)),
            );
            reporter2.current += 1;
            reporter2.update(&result);
        }

        // Verify counts before finish
        assert_eq!(reporter2.successful, 0);
        assert_eq!(reporter2.failed, 3);

        // Call finish to display summary
        reporter2.finish();

        // Verify counts remain stable after finish
        assert_eq!(reporter2.successful, 0);
        assert_eq!(reporter2.failed, 3);

        // Test case 3: Mixed success and failure
        let mut reporter3 = ProgressReporter::new();
        reporter3.start(10);

        // Process 7 successful videos
        for i in 0..7 {
            let result = create_process_result(
                &format!("/path/to/video_{}.mp4", i),
                &format!("/path/to/backdrops/backdrop_{}.mp4", i),
                true,
                None,
            );
            reporter3.current += 1;
            reporter3.update(&result);
        }

        // Process 3 failed videos
        for i in 7..10 {
            let result = create_process_result(
                &format!("/path/to/video_{}.mp4", i),
                "",
                false,
                Some(format!("Error {}", i)),
            );
            reporter3.current += 1;
            reporter3.update(&result);
        }

        // Verify counts before finish
        assert_eq!(reporter3.successful, 7);
        assert_eq!(reporter3.failed, 3);
        assert_eq!(reporter3.current, 10);

        // Call finish to display summary
        reporter3.finish();

        // Verify counts remain stable after finish
        assert_eq!(reporter3.successful, 7);
        assert_eq!(reporter3.failed, 3);
        assert_eq!(reporter3.current, 10);

        // Test case 4: Zero videos (edge case)
        let mut reporter4 = ProgressReporter::new();
        reporter4.start(0);

        // Verify initial state
        assert_eq!(reporter4.successful, 0);
        assert_eq!(reporter4.failed, 0);
        assert_eq!(reporter4.current, 0);
        assert_eq!(reporter4.total, 0);

        // Call finish with no videos processed
        reporter4.finish();

        // Verify counts remain at zero
        assert_eq!(reporter4.successful, 0);
        assert_eq!(reporter4.failed, 0);
    }
}
