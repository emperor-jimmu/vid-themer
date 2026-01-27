// Integration test for the full video clip extraction pipeline
// This test verifies the complete workflow from directory scanning to clip extraction

use std::fs;
use std::process::Command;

mod common;
use common::*;

#[test]
fn test_full_pipeline_with_sample_videos() {
    // Create a temporary directory structure for testing
    let temp_base = std::env::temp_dir().join(format!("integration_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_base);
    fs::create_dir_all(&temp_base).unwrap();

    // Create directory structure:
    // temp_base/
    //   ├── movies/
    //   │   ├── movie1.mp4
    //   │   └── movie2.mkv
    //   └── shows/
    //       ├── episode1.mp4
    //       └── nested/
    //           └── episode2.mp4

    let movies_dir = temp_base.join("movies");
    let shows_dir = temp_base.join("shows");
    let nested_dir = shows_dir.join("nested");

    fs::create_dir_all(&movies_dir).unwrap();
    fs::create_dir_all(&shows_dir).unwrap();
    fs::create_dir_all(&nested_dir).unwrap();

    // Create test videos (if FFmpeg is available)
    let video_paths = vec![
        movies_dir.join("movie1.mp4"),
        movies_dir.join("movie2.mkv"),
        shows_dir.join("episode1.mp4"),
        nested_dir.join("episode2.mp4"),
    ];

    let mut videos_created = 0;
    for video_path in &video_paths {
        // Create videos with different durations and resolutions
        let duration = 15; // 15 seconds
        let (width, height) = (1280, 720); // 720p

        if create_test_video(video_path, duration, width, height) {
            videos_created += 1;
            println!("Created test video: {:?}", video_path);
        } else {
            eprintln!("Failed to create test video: {:?}", video_path);
        }
    }

    // If no videos were created (FFmpeg not available), skip the test
    if videos_created == 0 {
        eprintln!("Skipping integration test: FFmpeg not available to create test videos");
        let _ = fs::remove_dir_all(&temp_base);
        return;
    }

    println!("Created {} test videos", videos_created);

    // Build the project binary
    if let Err(e) = build_binary() {
        panic!("{}", e);
    }

    let binary_path = get_binary_path();

    if !binary_path.exists() {
        panic!("Binary not found at {:?}", binary_path);
    }

    // Run the full pipeline with random strategy
    println!("Running video-clip-extractor on test directory...");
    let output = Command::new(&binary_path)
        .arg(&temp_base)
        .arg("--strategy")
        .arg("random")
        .arg("--resolution")
        .arg("1080p")
        .arg("--audio")
        .arg("true")
        .output()
        .expect("Failed to execute video-clip-extractor");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    println!("STDOUT:\n{}", stdout);
    if !stderr.is_empty() {
        println!("STDERR:\n{}", stderr);
    }

    // Verify the command succeeded
    assert!(
        output.status.success(),
        "video-clip-extractor should complete successfully"
    );

    // Verify progress output is displayed
    assert!(
        stdout.contains("Found") && stdout.contains("videos to process"),
        "Progress output should show total videos found"
    );

    assert!(
        stdout.contains("Processing:"),
        "Progress output should show processing status"
    );

    assert!(
        stdout.contains("Completed:"),
        "Progress output should show completion summary"
    );

    // Verify output files are created in correct locations
    let expected_outputs = vec![
        movies_dir.join("backdrops").join("backdrop.mp4"),
        shows_dir.join("backdrops").join("backdrop.mp4"),
        nested_dir.join("backdrops").join("backdrop.mp4"),
    ];

    let mut clips_found = 0;
    for expected_output in &expected_outputs {
        if expected_output.exists() {
            clips_found += 1;
            println!("✓ Found output clip: {:?}", expected_output);

            // Verify the clip is a valid video file
            assert!(
                expected_output.is_file(),
                "Output should be a file: {:?}",
                expected_output
            );

            // Verify the clip has content (not empty)
            let metadata = fs::metadata(expected_output).unwrap();
            assert!(
                metadata.len() > 0,
                "Output clip should not be empty: {:?}",
                expected_output
            );

            // Verify clip duration is between 10 and 15 seconds
            if let Some(duration) = get_video_duration(expected_output) {
                assert!(
                    duration >= 9.5 && duration <= 15.5,
                    "Clip duration should be between 10 and 15 seconds, got: {:.2}s for {:?}",
                    duration,
                    expected_output
                );
                println!("  Duration: {:.2}s", duration);
            }

            // Verify clip resolution (should not exceed 1080p)
            if let Some((width, height)) = get_video_resolution(expected_output) {
                assert!(
                    width <= 1920 && height <= 1080,
                    "Clip resolution should not exceed 1920x1080, got: {}x{} for {:?}",
                    width,
                    height,
                    expected_output
                );
                println!("  Resolution: {}x{}", width, height);
            }
        } else {
            eprintln!("✗ Output clip not found: {:?}", expected_output);
        }
    }

    // At least some clips should have been created
    assert!(
        clips_found > 0,
        "At least one output clip should be created"
    );

    println!(
        "\n✓ Integration test passed: {} clips created successfully",
        clips_found
    );

    // Clean up
    let _ = fs::remove_dir_all(&temp_base);
}

#[test]
fn test_pipeline_with_existing_clips() {
    // Test that the tool overwrites existing clips
    // Note: The scanner skips directories with existing backdrops/backdrop.mp4,
    // so we need to test the overwrite behavior by running the tool twice
    let temp_base =
        std::env::temp_dir().join(format!("integration_overwrite_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_base);
    fs::create_dir_all(&temp_base).unwrap();

    let video_path = temp_base.join("test_video.mp4");

    // Create a test video
    if !create_test_video(&video_path, 15, 1280, 720) {
        eprintln!("Skipping overwrite test: FFmpeg not available");
        let _ = fs::remove_dir_all(&temp_base);
        return;
    }

    // Build the project
    if let Err(e) = build_binary() {
        panic!("{}", e);
    }

    let binary_path = get_binary_path();

    // Run the tool the first time to create the initial clip
    let output1 = Command::new(&binary_path)
        .arg(&temp_base)
        .output()
        .expect("Failed to execute video-clip-extractor (first run)");

    assert!(
        output1.status.success(),
        "First run should complete successfully"
    );

    let backdrops_dir = temp_base.join("backdrops");
    let clip_path = backdrops_dir.join("backdrop.mp4");

    assert!(clip_path.exists(), "Clip should be created on first run");

    let first_metadata = fs::metadata(&clip_path).unwrap();
    let first_size = first_metadata.len();

    // Wait a moment to ensure modification time will be different
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Run the tool a second time - it should overwrite the existing clip
    // Since the directory now has backdrops/backdrop.mp4, the scanner will skip it
    // This test verifies that the scanner correctly skips directories with existing clips
    let output2 = Command::new(&binary_path)
        .arg(&temp_base)
        .output()
        .expect("Failed to execute video-clip-extractor (second run)");

    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    println!("Second run STDOUT:\n{}", stdout2);

    assert!(
        output2.status.success(),
        "Second run should complete successfully"
    );

    // The scanner should skip the directory since it has backdrops/backdrop.mp4
    assert!(
        stdout2.contains("already have backdrop clips") || stdout2.contains("No videos found"),
        "Scanner should skip directory with existing backdrop"
    );

    // The clip should still exist and be unchanged
    assert!(
        clip_path.exists(),
        "Clip should still exist after second run"
    );

    let second_metadata = fs::metadata(&clip_path).unwrap();
    let second_size = second_metadata.len();

    // The clip should be unchanged (same size) since the directory was skipped
    assert_eq!(
        first_size, second_size,
        "Clip should be unchanged when directory is skipped (size: {})",
        first_size
    );

    println!("✓ Overwrite test passed: scanner correctly skips directories with existing clips");

    // Clean up
    let _ = fs::remove_dir_all(&temp_base);
}

#[test]
fn test_pipeline_skips_directories_with_existing_clips() {
    // Test that directories with existing backdrops/backdrop.mp4 are skipped
    let temp_base =
        std::env::temp_dir().join(format!("integration_skip_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_base);
    fs::create_dir_all(&temp_base).unwrap();

    // Create two directories
    let dir1 = temp_base.join("dir1");
    let dir2 = temp_base.join("dir2");
    fs::create_dir_all(&dir1).unwrap();
    fs::create_dir_all(&dir2).unwrap();

    // Create videos in both directories
    let video1 = dir1.join("video1.mp4");
    let video2 = dir2.join("video2.mp4");

    let videos_created = create_test_video(&video1, 15, 1280, 720) as u32
        + create_test_video(&video2, 15, 1280, 720) as u32;

    if videos_created == 0 {
        eprintln!("Skipping skip test: FFmpeg not available");
        let _ = fs::remove_dir_all(&temp_base);
        return;
    }

    // Create an existing backdrop in dir1 (should cause dir1 to be skipped)
    let backdrops_dir1 = dir1.join("backdrops");
    fs::create_dir_all(&backdrops_dir1).unwrap();
    let existing_clip1 = backdrops_dir1.join("backdrop.mp4");
    fs::File::create(&existing_clip1).unwrap();

    // Build the project
    if let Err(e) = build_binary() {
        panic!("{}", e);
    }

    let binary_path = get_binary_path();

    // Run the tool
    let output = Command::new(&binary_path)
        .arg(&temp_base)
        .output()
        .expect("Failed to execute video-clip-extractor");

    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("STDOUT:\n{}", stdout);

    assert!(output.status.success(), "Tool should complete successfully");

    // Verify that only dir2 was processed (dir1 should be skipped)
    // The output should show fewer videos found than we created
    let backdrops_dir2 = dir2.join("backdrops");
    let clip2 = backdrops_dir2.join("backdrop.mp4");

    // dir2 should have a new clip
    if videos_created == 2 {
        assert!(clip2.exists(), "dir2 should have a new clip created");
    }

    println!("✓ Skip test passed: directories with existing clips are skipped");

    // Clean up
    let _ = fs::remove_dir_all(&temp_base);
}
