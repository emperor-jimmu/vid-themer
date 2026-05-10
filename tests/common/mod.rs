// Common test utilities shared across integration tests

use std::path::Path;
use std::process::Command;

/// Helper function to create a simple test video using FFmpeg
/// Returns true if video was created successfully, false otherwise
pub fn create_test_video(path: &Path, duration_secs: u32, width: u32, height: u32) -> bool {
    // Check if FFmpeg is available
    let ffmpeg_check = Command::new("ffmpeg").arg("-version").output();

    if ffmpeg_check.is_err() {
        eprintln!("FFmpeg not available, skipping video creation");
        return false;
    }

    // Create a test video with color bars pattern
    let output = Command::new("ffmpeg")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg(format!(
            "testsrc=duration={}:size={}x{}:rate=30",
            duration_secs, width, height
        ))
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg(format!("sine=frequency=1000:duration={}", duration_secs))
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("ultrafast")
        .arg("-c:a")
        .arg("aac")
        .arg("-y")
        .arg(path)
        .output();

    match output {
        Ok(result) => result.status.success(),
        Err(_) => false,
    }
}

/// Helper function to get video duration using ffprobe
pub fn get_video_duration(path: &Path) -> Option<f64> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let duration_str = String::from_utf8_lossy(&output.stdout);
    duration_str.trim().parse::<f64>().ok()
}

/// Helper function to get video resolution using ffprobe
pub fn get_video_resolution(path: &Path) -> Option<(u32, u32)> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=width,height")
        .arg("-of")
        .arg("csv=s=x:p=0")
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let resolution_str = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = resolution_str.trim().split('x').collect();

    if parts.len() == 2 {
        let width = parts[0].parse::<u32>().ok()?;
        let height = parts[1].parse::<u32>().ok()?;
        Some((width, height))
    } else {
        None
    }
}

/// Get the path to the compiled binary
pub fn get_binary_path() -> std::path::PathBuf {
    if cfg!(windows) {
        std::path::PathBuf::from("target/release/video-clip-extractor.exe")
    } else {
        std::path::PathBuf::from("target/release/video-clip-extractor")
    }
}

/// Build the project binary
pub fn build_binary() -> Result<(), String> {
    let build_output = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .output()
        .map_err(|e| format!("Failed to execute cargo build: {}", e))?;

    if !build_output.status.success() {
        return Err(format!(
            "Failed to build project: {}",
            String::from_utf8_lossy(&build_output.stderr)
        ));
    }

    Ok(())
}
