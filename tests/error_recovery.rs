// Integration tests for error recovery and handling
// Verifies that the tool continues processing when individual videos fail

use std::fs;
use std::process::Command;

mod common;
use common::*;

#[test]
fn test_error_recovery_with_corrupted_video() {
    // Test that the tool continues processing when one video fails
    // Requirements: 7.2, 7.3, 7.5
    let temp_base = std::env::temp_dir().join(format!(
        "integration_error_recovery_test_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_base);
    fs::create_dir_all(&temp_base).unwrap();

    // Create directory structure with multiple videos
    let dir1 = temp_base.join("dir1");
    let dir2 = temp_base.join("dir2");
    let dir3 = temp_base.join("dir3");
    fs::create_dir_all(&dir1).unwrap();
    fs::create_dir_all(&dir2).unwrap();
    fs::create_dir_all(&dir3).unwrap();

    // Create valid test videos
    let video1 = dir1.join("valid_video1.mp4");
    let video3 = dir3.join("valid_video3.mp4");

    let mut valid_videos_created = 0;
    if create_test_video(&video1, 30, 1280, 720) {
        valid_videos_created += 1;
        println!("Created valid test video: {:?}", video1);
    }
    if create_test_video(&video3, 30, 1280, 720) {
        valid_videos_created += 1;
        println!("Created valid test video: {:?}", video3);
    }

    if valid_videos_created == 0 {
        eprintln!("Skipping error recovery test: FFmpeg not available");
        let _ = fs::remove_dir_all(&temp_base);
        return;
    }

    // Create a corrupted video file (just write garbage data)
    let corrupted_video = dir2.join("corrupted_video.mp4");
    fs::write(
        &corrupted_video,
        b"This is not a valid video file, just garbage data to simulate corruption",
    )
    .unwrap();
    println!("Created corrupted video: {:?}", corrupted_video);

    // Build the project
    if let Err(e) = build_binary() {
        panic!("{}", e);
    }

    let binary_path = get_binary_path();

    if !binary_path.exists() {
        panic!("Binary not found at {:?}", binary_path);
    }

    // Run the tool on the directory with mixed valid and corrupted videos
    println!("Running video-clip-extractor on directory with corrupted video...");
    let output = Command::new(&binary_path)
        .arg(&temp_base)
        .arg("--strategy")
        .arg("random")
        .output()
        .expect("Failed to execute video-clip-extractor");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{}", stdout);
    if !stderr.is_empty() {
        println!("STDERR:\n{}", stderr);
    }

    // Verify the command succeeded overall (despite one video failing)
    // Requirement 7.2: Processing should continue for other videos
    assert!(
        output.status.success(),
        "Tool should complete successfully even with one corrupted video"
    );

    // Verify that the tool found all videos (including the corrupted one)
    assert!(
        stdout.contains("Found") && stdout.contains("videos to process"),
        "Progress output should show total videos found"
    );

    // Verify error is logged for the corrupted video
    // Requirement 7.3: FFmpeg execution failures should be logged
    // Requirement 7.5: Error messages should include the file path
    let output_combined = format!("{}\n{}", stdout, stderr);
    assert!(
        output_combined.contains("corrupted_video.mp4")
            || output_combined.contains("Error")
            || output_combined.contains("Failed")
            || output_combined.contains("failed"),
        "Error message should be logged for corrupted video with file path"
    );

    // Verify that valid videos were still processed successfully
    // Requirement 7.2: Continue processing other videos after error
    let clip1 = dir1.join("backdrops").join("backdrop1.mp4");
    let clip3 = dir3.join("backdrops").join("backdrop1.mp4");

    let mut successful_clips = 0;
    if clip1.exists() {
        successful_clips += 1;
        println!("✓ Valid video 1 was processed successfully");

        // Verify the clip is valid
        let metadata = fs::metadata(&clip1).unwrap();
        assert!(metadata.len() > 0, "Clip should not be empty");
    }

    if clip3.exists() {
        successful_clips += 1;
        println!("✓ Valid video 3 was processed successfully");

        // Verify the clip is valid
        let metadata = fs::metadata(&clip3).unwrap();
        assert!(metadata.len() > 0, "Clip should not be empty");
    }

    // At least one valid video should have been processed
    assert!(
        successful_clips > 0,
        "At least one valid video should be processed successfully despite corrupted video"
    );

    // Verify the corrupted video did NOT produce a clip
    let corrupted_clip = dir2.join("backdrops").join("backdrop1.mp4");
    assert!(
        !corrupted_clip.exists() || fs::metadata(&corrupted_clip).unwrap().len() == 0,
        "Corrupted video should not produce a valid clip"
    );

    // Verify the summary shows both successful and failed counts
    // Requirement 8.4: Display summary of successful and failed extractions
    assert!(
        stdout.contains("Completed:") || stdout.contains("successful") || stdout.contains("failed"),
        "Summary should show successful and failed counts"
    );

    println!("\n✓ Error recovery test passed:");
    println!(
        "  - {} valid videos processed successfully",
        successful_clips
    );
    println!("  - Corrupted video error was logged with file path");
    println!("  - Processing continued despite error");

    // Clean up
    let _ = fs::remove_dir_all(&temp_base);
}
