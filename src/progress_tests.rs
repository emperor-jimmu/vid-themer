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
