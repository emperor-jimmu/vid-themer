// Test for 0-byte backdrop file regeneration

use std::fs;
use std::io::Write;
use std::process::Command;

#[test]
fn test_zero_byte_backdrop_regeneration() {
    // Create a temporary test directory
    let temp_dir = std::env::temp_dir().join(format!(
        "zero_byte_test_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create a fake video file
    let video_path = temp_dir.join("test_video.mp4");
    let mut video_file = fs::File::create(&video_path).unwrap();
    video_file.write_all(b"fake video content").unwrap();

    // Create backdrops directory with a 0-byte backdrop.mp4
    let backdrops_dir = temp_dir.join("backdrops");
    fs::create_dir_all(&backdrops_dir).unwrap();
    let backdrop_path = backdrops_dir.join("backdrop.mp4");
    
    // Create a 0-byte file
    fs::File::create(&backdrop_path).unwrap();
    
    // Verify it's 0 bytes
    let metadata = fs::metadata(&backdrop_path).unwrap();
    assert_eq!(metadata.len(), 0, "Backdrop should be 0 bytes initially");

    // Run the video-clip-extractor on this directory
    // It should process the video because the backdrop is 0 bytes
    let output = Command::new(env!("CARGO_BIN_EXE_video-clip-extractor"))
        .arg(&temp_dir)
        .output();

    // The command will fail because the video is fake, but we can check
    // that it attempted to process it (not skipped)
    if let Ok(result) = output {
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        
        // Should show "Found 1 videos to process" (not 0)
        assert!(
            stdout.contains("Found 1 videos") || stdout.contains("Found 1 video"),
            "Should find 1 video to process (0-byte backdrop should not cause skip). stdout: {}, stderr: {}",
            stdout, stderr
        );
    }

    // Clean up
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_non_zero_byte_backdrop_skipped() {
    // Create a temporary test directory
    let temp_dir = std::env::temp_dir().join(format!(
        "non_zero_byte_test_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create a fake video file
    let video_path = temp_dir.join("test_video.mp4");
    let mut video_file = fs::File::create(&video_path).unwrap();
    video_file.write_all(b"fake video content").unwrap();

    // Create backdrops directory with a non-zero backdrop.mp4
    let backdrops_dir = temp_dir.join("backdrops");
    fs::create_dir_all(&backdrops_dir).unwrap();
    let backdrop_path = backdrops_dir.join("backdrop.mp4");
    
    // Create a file with content
    let mut backdrop_file = fs::File::create(&backdrop_path).unwrap();
    backdrop_file.write_all(b"some backdrop content").unwrap();
    
    // Verify it's non-zero bytes
    let metadata = fs::metadata(&backdrop_path).unwrap();
    assert!(metadata.len() > 0, "Backdrop should have content");

    // Run the video-clip-extractor on this directory
    // It should skip the video because the backdrop has content
    let output = Command::new(env!("CARGO_BIN_EXE_video-clip-extractor"))
        .arg(&temp_dir)
        .output();

    if let Ok(result) = output {
        let stdout = String::from_utf8_lossy(&result.stdout);
        
        // Should show that it skipped the directory
        assert!(
            stdout.contains("already have backdrop") || stdout.contains("Skipped 1 director"),
            "Should skip directory with valid backdrop. stdout: {}",
            stdout
        );
    }

    // Clean up
    let _ = fs::remove_dir_all(&temp_dir);
}
