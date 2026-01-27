// Failure logging for debugging

use crate::processor::ProcessResult;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct FailureLogger {
    log_path: PathBuf,
}

impl FailureLogger {
    /// Create a new failure logger with a timestamped log file in the current working directory
    pub fn new(_root_dir: &Path) -> std::io::Result<Self> {
        // Get current working directory (where the executable is running from)
        let current_dir = std::env::current_dir()?;

        // Generate timestamp for log filename (format: YYYY-MM-DD-HH-MM-SS.log)
        let now = std::time::SystemTime::now();
        let datetime = now
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(std::io::Error::other)?;

        // Convert to local time components
        let total_seconds = datetime.as_secs();
        let days_since_epoch = total_seconds / 86400;
        let seconds_today = total_seconds % 86400;

        // Calculate date (simplified calculation from Unix epoch)
        let mut year = 1970;
        let mut remaining_days = days_since_epoch;

        loop {
            let days_in_year = if Self::is_leap_year(year) { 366 } else { 365 };
            if remaining_days < days_in_year {
                break;
            }
            remaining_days -= days_in_year;
            year += 1;
        }

        let (month, day) =
            Self::day_of_year_to_month_day(remaining_days as u32 + 1, Self::is_leap_year(year));

        // Calculate time
        let hour = seconds_today / 3600;
        let minute = (seconds_today % 3600) / 60;
        let second = seconds_today % 60;

        let timestamp = format!(
            "{:04}-{:02}-{:02}-{:02}-{:02}-{:02}.log",
            year, month, day, hour, minute, second
        );

        let log_path = current_dir.join(timestamp);

        // Create or truncate the log file
        let mut file = File::create(&log_path)?;
        writeln!(file, "Video Clip Extractor - Failure Log")?;
        writeln!(file, "====================================")?;
        writeln!(file)?;

        Ok(Self { log_path })
    }

    /// Check if a year is a leap year
    fn is_leap_year(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    /// Convert day of year to (month, day)
    fn day_of_year_to_month_day(day_of_year: u32, is_leap: bool) -> (u32, u32) {
        let days_in_months = if is_leap {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };

        let mut remaining = day_of_year;
        for (month_idx, &days) in days_in_months.iter().enumerate() {
            if remaining <= days {
                return ((month_idx + 1) as u32, remaining);
            }
            remaining -= days;
        }

        // Fallback (should not happen)
        (12, 31)
    }

    /// Log a failed processing result with detailed error information
    pub fn log_failure(&self, result: &ProcessResult, ffmpeg_stderr: Option<&str>) {
        if let Err(e) = self.write_failure(result, ffmpeg_stderr) {
            eprintln!("Warning: Failed to write to log file: {}", e);
        }
    }

    fn write_failure(
        &self,
        result: &ProcessResult,
        ffmpeg_stderr: Option<&str>,
    ) -> std::io::Result<()> {
        let mut file = OpenOptions::new().append(true).open(&self.log_path)?;

        writeln!(file, "----------------------------------------")?;
        writeln!(file, "FAILURE: {}", result.video_path.display())?;
        writeln!(file, "----------------------------------------")?;

        if let Some(error_msg) = &result.error_message {
            writeln!(file, "Error: {}", error_msg)?;
        }

        // Include clip generation information for multi-clip failures
        writeln!(file, "Clips Generated: {}", result.clips_generated)?;

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
        let temp_dir =
            std::env::temp_dir().join(format!("logger_failure_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        // Sleep briefly to ensure unique timestamp
        std::thread::sleep(std::time::Duration::from_millis(10));

        let logger = FailureLogger::new(&temp_dir).unwrap();

        // Use a realistic path that will work on all platforms
        let video_path = temp_dir.join("test").join("video.mp4");
        let output_path = temp_dir.join("test").join("backdrops").join("backdrop.mp4");

        let result = ProcessResult {
            video_path: video_path.clone(),
            output_path,
            success: false,
            error_message: Some("Test error".to_string()),
            ffmpeg_stderr: None,
            clips_generated: 0,
        };

        logger.log_failure(&result, Some("FFmpeg stderr output"));

        // Verify log file contains the failure
        let log_content = fs::read_to_string(logger.log_path()).unwrap();
        // Check for the full video path (platform-agnostic)
        let video_path_str = video_path.display().to_string();
        assert!(
            log_content.contains(&video_path_str) || log_content.contains("video.mp4"),
            "Log should contain video path or filename. Expected path: {}, Got log content:\n{}",
            video_path_str,
            log_content
        );
        assert!(log_content.contains("FAILURE:"));
        assert!(log_content.contains("Test error"));
        assert!(log_content.contains("FFmpeg stderr output"));
        assert!(log_content.contains("Clips Generated: 0"));

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
        // Clean up the log file created in current directory
        let _ = fs::remove_file(logger.log_path());
    }

    #[test]
    fn test_log_failure_with_multiple_clips() {
        let temp_dir =
            std::env::temp_dir().join(format!("logger_multi_clip_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        // Sleep briefly to ensure unique timestamp
        std::thread::sleep(std::time::Duration::from_millis(20));

        let logger = FailureLogger::new(&temp_dir).unwrap();

        // Use realistic paths
        let video_path = temp_dir.join("test").join("video.mp4");
        let output_path = temp_dir.join("test").join("backdrops").join("backdrop2.mp4");

        let result = ProcessResult {
            video_path: video_path.clone(),
            output_path,
            success: false,
            error_message: Some("Failed to extract clip 2 of 3".to_string()),
            ffmpeg_stderr: None,
            clips_generated: 1,
        };

        logger.log_failure(&result, None);

        // Verify log file contains the multi-clip failure information
        let log_content = fs::read_to_string(logger.log_path()).unwrap();
        let video_path_str = video_path.display().to_string();
        assert!(
            log_content.contains(&video_path_str) || log_content.contains("video.mp4"),
            "Log should contain video path or filename. Expected path: {}, Got log content:\n{}",
            video_path_str,
            log_content
        );
        assert!(
            log_content.contains("FAILURE:"),
            "Log content:\n{}",
            log_content
        );
        assert!(
            log_content.contains("Failed to extract clip 2 of 3"),
            "Log content:\n{}",
            log_content
        );
        assert!(
            log_content.contains("Clips Generated: 1"),
            "Log content:\n{}",
            log_content
        );

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
        // Clean up the log file created in current directory
        let _ = fs::remove_file(logger.log_path());
    }
}
