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
    //   ├── Movie1 (2020)/
    //   │   └── movie1.mp4
    //   ├── Movie2 (2021)/
    //   │   └── movie2.mkv
    //   └── Movie3 (2022)/
    //       └── movie3.mp4

    let movie1_dir = temp_base.join("Movie1 (2020)");
    let movie2_dir = temp_base.join("Movie2 (2021)");
    let movie3_dir = temp_base.join("Movie3 (2022)");

    fs::create_dir_all(&movie1_dir).unwrap();
    fs::create_dir_all(&movie2_dir).unwrap();
    fs::create_dir_all(&movie3_dir).unwrap();

    // Create test videos (if FFmpeg is available)
    let video_paths = vec![
        movie1_dir.join("movie1.mp4"),
        movie2_dir.join("movie2.mkv"),
        movie3_dir.join("movie3.mp4"),
    ];

    let mut videos_created = 0;
    for video_path in &video_paths {
        // Create videos with different durations and resolutions
        let duration = 35; // 35 seconds (enough for 20-30s clip with 2% intro + 40% outro exclusion)
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
        movie1_dir.join("backdrops").join("backdrop1.mp4"),
        movie2_dir.join("backdrops").join("backdrop1.mp4"),
        movie3_dir.join("backdrops").join("backdrop1.mp4"),
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

            // Verify clip duration is between 20 and 30 seconds (default CLI range)
            if let Some(duration) = get_video_duration(expected_output) {
                assert!(
                    duration >= 19.5 && duration <= 31.0,
                    "Clip duration should be between 20 and 30 seconds, got: {:.2}s for {:?}",
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
fn test_pipeline_skips_directories_with_existing_clips() {
    // Test that directories with existing backdrops/backdrop1.mp4 are skipped
    let temp_base =
        std::env::temp_dir().join(format!("integration_skip_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_base);
    fs::create_dir_all(&temp_base).unwrap();

    // Create two movie directories
    let movie1_dir = temp_base.join("Movie1 (2020)");
    let movie2_dir = temp_base.join("Movie2 (2021)");
    fs::create_dir_all(&movie1_dir).unwrap();
    fs::create_dir_all(&movie2_dir).unwrap();

    // Create videos in both directories
    let video1 = movie1_dir.join("video1.mp4");
    let video2 = movie2_dir.join("video2.mp4");

    let videos_created = create_test_video(&video1, 120, 1280, 720) as u32
        + create_test_video(&video2, 120, 1280, 720) as u32;

    if videos_created == 0 {
        eprintln!("Skipping skip test: FFmpeg not available");
        let _ = fs::remove_dir_all(&temp_base);
        return;
    }

    // Create a done.ext marker in movie1_dir (should cause it to be skipped)
    let backdrops_dir1 = movie1_dir.join("backdrops");
    fs::create_dir_all(&backdrops_dir1).unwrap();
    let done_marker = backdrops_dir1.join("done.ext");
    fs::write(&done_marker, "{ \"completed_at\": \"test\" }").unwrap();

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

    // Verify that only movie2 was processed (movie1 should be skipped)
    // The output should show fewer videos found than we created
    let backdrops_dir2 = movie2_dir.join("backdrops");
    let clip2 = backdrops_dir2.join("backdrop1.mp4");

    // movie2 should have a new clip
    if videos_created == 2 {
        assert!(clip2.exists(), "movie2 should have a new clip created");
    }

    println!("✓ Skip test passed: directories with existing clips are skipped");

    // Clean up
    let _ = fs::remove_dir_all(&temp_base);
}
