// Failure logging for debugging

use crate::processor::ProcessResult;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct FailureLogger {
    log_path: PathBuf,
}

impl FailureLogger {
    /// Create a new failure logger with a log file in the specified directory
    pub fn new(root_dir: &Path) -> std::io::Result<Self> {
        let log_path = root_dir.join("video_clip_extractor_failures.log");
        
        // Create or truncate the log file
        let mut file = File::create(&log_path)?;
        writeln!(file, "Video Clip Extractor - Failure Log")?;
        writeln!(file, "====================================")?;
        writeln!(file)?;
        
        Ok(Self { log_path })
    }

    /// Log a failed processing result with detailed error information
    pub fn log_failure(&self, result: &ProcessResult, ffmpeg_stderr: Option<&str>) {
        if let Err(e) = self.write_failure(result, ffmpeg_stderr) {
            eprintln!("Warning: Failed to write to log file: {}", e);
        }
    }

    fn write_failure(&self, result: &ProcessResult, ffmpeg_stderr: Option<&str>) -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .append(true)
            .open(&self.log_path)?;

        writeln!(file, "----------------------------------------")?;
        writeln!(file, "FAILURE: {}", result.video_path.display())?;
        writeln!(file, "----------------------------------------")?;
        
        if let Some(error_msg) = &result.error_message {
            writeln!(file, "Error: {}", error_msg)?;
        }
        
        if !result.output_path.as_os_str().is_empty() {
            writeln!(file, "Output Path: {}", result.output_path.display())?;
        }
        
        if let Some(stderr) = ffmpeg_stderr {
            writeln!(file)?;
            writeln!(file, "FFmpeg Error Output:")?;
            writeln!(file, "{}", stderr)?;
        }
        
        writeln!(file)?;
        
        Ok(())
    }

    /// Get the path to the log file
    pub fn log_path(&self) -> &Path {
        &self.log_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_logger_creation() {
        let temp_dir = std::env::temp_dir().join(format!("logger_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let logger = FailureLogger::new(&temp_dir);
        assert!(logger.is_ok());

        let logger = logger.unwrap();
        assert!(logger.log_path().exists());
        assert!(logger.log_path().is_file());

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_log_failure() {
        let temp_dir = std::env::temp_dir().join(format!("logger_failure_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let logger = FailureLogger::new(&temp_dir).unwrap();

        let result = ProcessResult {
            video_path: PathBuf::from("/test/video.mp4"),
            output_path: PathBuf::from("/test/backdrops/backdrop.mp4"),
            success: false,
            error_message: Some("Test error".to_string()),
            ffmpeg_stderr: None,
        };

        logger.log_failure(&result, Some("FFmpeg stderr output"));

        // Verify log file contains the failure
        let log_content = fs::read_to_string(logger.log_path()).unwrap();
        assert!(log_content.contains("FAILURE: /test/video.mp4"));
        assert!(log_content.contains("Test error"));
        assert!(log_content.contains("FFmpeg stderr output"));

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
