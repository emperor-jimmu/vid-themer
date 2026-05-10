// Integration test for incremental clip generation
// Tests that running with -c 3 after -c 2 generates only the missing clip

use std::fs;
use std::process::Command;

mod common;

#[test]
#[ignore] // Requires FFmpeg and real video files
fn test_incremental_clip_generation() {
    // Create a temporary test directory
    let test_dir =
        std::env::temp_dir().join(format!("incremental_clip_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    // Create a test video using the common helper
    let video_path = test_dir.join("test_video.mp4");
    assert!(
        common::create_test_video(&video_path, 30, 1920, 1080),
        "Failed to create test video"
    );

    // Step 1: Run with -c 2 to generate 2 clips
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--",
            test_dir.to_str().unwrap(),
            "-c",
            "2",
            "--min-duration",
            "2",
            "--max-duration",
            "3",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "First run with -c 2 should succeed"
    );

    // Verify that backdrop1.mp4 and backdrop2.mp4 exist
    let backdrops_dir = test_dir.join("backdrops");
    let backdrop1 = backdrops_dir.join("backdrop1.mp4");
    let backdrop2 = backdrops_dir.join("backdrop2.mp4");
    let backdrop3 = backdrops_dir.join("backdrop3.mp4");

    assert!(
        backdrop1.exists(),
        "backdrop1.mp4 should exist after first run"
    );
    assert!(
        backdrop2.exists(),
        "backdrop2.mp4 should exist after first run"
    );
    assert!(
        !backdrop3.exists(),
        "backdrop3.mp4 should NOT exist after first run"
    );

    // Get file sizes to verify they're not regenerated
    let backdrop1_size = fs::metadata(&backdrop1).unwrap().len();
    let backdrop2_size = fs::metadata(&backdrop2).unwrap().len();

    assert!(backdrop1_size > 0, "backdrop1.mp4 should not be empty");
    assert!(backdrop2_size > 0, "backdrop2.mp4 should not be empty");

    // Step 2: Run with -c 3 to generate the 3rd clip
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--",
            test_dir.to_str().unwrap(),
            "-c",
            "3",
            "--min-duration",
            "2",
            "--max-duration",
            "3",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Second run with -c 3 should succeed"
    );

    // Verify that backdrop3.mp4 now exists
    assert!(
        backdrop3.exists(),
        "backdrop3.mp4 should exist after second run"
    );
    let backdrop3_size = fs::metadata(&backdrop3).unwrap().len();
    assert!(backdrop3_size > 0, "backdrop3.mp4 should not be empty");

    // Verify that backdrop1.mp4 and backdrop2.mp4 were NOT regenerated
    // (file sizes should be the same)
    let backdrop1_size_after = fs::metadata(&backdrop1).unwrap().len();
    let backdrop2_size_after = fs::metadata(&backdrop2).unwrap().len();

    assert_eq!(
        backdrop1_size, backdrop1_size_after,
        "backdrop1.mp4 should not be regenerated"
    );
    assert_eq!(
        backdrop2_size, backdrop2_size_after,
        "backdrop2.mp4 should not be regenerated"
    );

    // Step 3: Run with -c 2 again - should skip the video entirely
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--",
            test_dir.to_str().unwrap(),
            "-c",
            "2",
            "--min-duration",
            "2",
            "--max-duration",
            "3",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Third run with -c 2 should succeed (skip video)"
    );

    // Verify output indicates the video was skipped
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("already have backdrop clips") || stdout.contains("Skipped"),
        "Output should indicate video was skipped"
    );

    // Step 4: Run with -c 1 - should also skip the video
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--",
            test_dir.to_str().unwrap(),
            "-c",
            "1",
            "--min-duration",
            "2",
            "--max-duration",
            "3",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Fourth run with -c 1 should succeed (skip video)"
    );

    // Clean up
    let _ = fs::remove_dir_all(&test_dir);
}

#[test]
fn test_skip_when_enough_clips_exist() {
    // Test that videos with enough clips are skipped during scanning

    let test_dir =
        std::env::temp_dir().join(format!("skip_enough_clips_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&test_dir);
    fs::create_dir_all(&test_dir).unwrap();

    // Create a test video
    let video_path = test_dir.join("test_video.mp4");
    if !common::create_test_video(&video_path, 30, 1920, 1080) {
        eprintln!("Skipping test_skip_when_enough_clips_exist: FFmpeg not available");
        let _ = fs::remove_dir_all(&test_dir);
        return;
    }

    // Manually create 2 backdrop files
    let backdrops_dir = test_dir.join("backdrops");
    fs::create_dir_all(&backdrops_dir).unwrap();

    let backdrop1 = backdrops_dir.join("backdrop1.mp4");
    let backdrop2 = backdrops_dir.join("backdrop2.mp4");

    // Create dummy backdrop files with some content
    fs::write(&backdrop1, b"dummy backdrop 1 content").unwrap();
    fs::write(&backdrop2, b"dummy backdrop 2 content").unwrap();

    // Run with -c 2. Without done.ext, scanner should not skip; processor should detect
    // enough clips and short-circuit by writing done.ext.
    let output = Command::new("cargo")
        .args(&["run", "--", test_dir.to_str().unwrap(), "-c", "2"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Run with -c 2 should succeed");
    assert!(
        backdrops_dir.join("done.ext").exists(),
        "done.ext should be written"
    );

    // Run with -c 1 - scanner should now skip due to done.ext
    let output = Command::new("cargo")
        .args(&["run", "--", test_dir.to_str().unwrap(), "-c", "1"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Run with -c 1 should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("already have backdrop clips") || stdout.contains("Skipped"),
        "Output should indicate video was skipped after done.ext is present"
    );

    // Clean up
    let _ = fs::remove_dir_all(&test_dir);
}
