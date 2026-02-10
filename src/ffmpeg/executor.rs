// FFmpeg command execution

use std::path::Path;
use std::process::Command;

use crate::cli::Resolution;
use crate::selector::TimeRange;

use super::command_builder;
use super::constants::fade;
use super::error::FFmpegError;
use super::metadata::{VideoMetadata, get_video_metadata};

/// FFmpeg executor with configuration
#[derive(Clone)]
pub struct FFmpegExecutor {
    pub resolution: Resolution,
    pub include_audio: bool,
    pub use_hw_accel: bool,
}

impl FFmpegExecutor {
    pub fn new(resolution: Resolution, include_audio: bool) -> Self {
        Self {
            resolution,
            include_audio,
            use_hw_accel: false,
        }
    }

    /// Check if FFmpeg is available in the system PATH
    pub fn check_availability() -> Result<(), FFmpegError> {
        let result = Command::new("ffmpeg").arg("-version").output();

        match result {
            Ok(output) => {
                if output.status.success() {
                    Ok(())
                } else {
                    Err(FFmpegError::NotFound)
                }
            }
            Err(_) => Err(FFmpegError::NotFound),
        }
    }

    /// Get video metadata
    pub fn get_video_metadata(&self, video_path: &Path) -> Result<VideoMetadata, FFmpegError> {
        get_video_metadata(video_path)
    }

    /// Get video duration (legacy method)
    pub fn get_duration(&self, video_path: &Path) -> Result<f64, FFmpegError> {
        let metadata = self.get_video_metadata(video_path)?;
        Ok(metadata.duration)
    }

    /// Extract a clip from a video file
    pub fn extract_clip(
        &self,
        video_path: &Path,
        time_range: &TimeRange,
        output_path: &Path,
    ) -> Result<(), FFmpegError> {
        let metadata = self.get_video_metadata(video_path)?;
        let source_resolution = (metadata.width, metadata.height);
        let codec = &metadata.codec;

        let temp_path = output_path.with_extension("tmp.mp4");

        let config = command_builder::ExtractConfig {
            video_path,
            time_range,
            output_path: &temp_path,
            source_resolution,
            codec,
            target_resolution: self.resolution.clone(),
            include_audio: self.include_audio,
            use_hw_accel: self.use_hw_accel,
        };

        let args = command_builder::build_extract_command(&config);

        let output = Command::new("ffmpeg").args(&args).output().map_err(|e| {
            FFmpegError::ExecutionFailed(format!(
                "Failed to execute ffmpeg for '{}': {}",
                video_path.display(),
                e
            ))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let _ = std::fs::remove_file(&temp_path);

            if stderr.contains("corrupt")
                || stderr.contains("Invalid NAL unit")
                || stderr.contains("concealing")
                || stderr.contains("error while decoding")
                || stderr.contains("missing picture in access unit")
            {
                return self.extract_clip_with_recovery(
                    video_path,
                    time_range,
                    output_path,
                    source_resolution,
                    codec,
                );
            }

            return Err(FFmpegError::ExecutionFailed(format!(
                "FFmpeg clip extraction failed for '{}' at {:.2}s-{:.2}s: {}",
                video_path.display(),
                time_range.start_seconds,
                time_range.start_seconds + time_range.duration_seconds,
                stderr
            )));
        }

        validate_output(&temp_path)?;
        apply_fade_effect(&temp_path, output_path, time_range.duration_seconds)?;
        validate_output(output_path)?;

        Ok(())
    }

    /// Extract clip with error recovery
    fn extract_clip_with_recovery(
        &self,
        video_path: &Path,
        time_range: &TimeRange,
        output_path: &Path,
        source_resolution: (u32, u32),
        codec: &str,
    ) -> Result<(), FFmpegError> {
        let mut args = vec![
            "-err_detect".to_string(),
            "ignore_err".to_string(),
            "-fflags".to_string(),
            "+genpts+igndts".to_string(),
            "-max_error_rate".to_string(),
            "1.0".to_string(),
        ];

        let config = command_builder::ExtractConfig {
            video_path,
            time_range,
            output_path,
            source_resolution,
            codec,
            target_resolution: self.resolution.clone(),
            include_audio: self.include_audio,
            use_hw_accel: self.use_hw_accel,
        };

        let standard_args = command_builder::build_extract_command(&config);

        args.extend(standard_args.into_iter().skip(2));

        let output = Command::new("ffmpeg").args(&args).output().map_err(|e| {
            FFmpegError::ExecutionFailed(format!(
                "Failed to execute ffmpeg recovery for '{}': {}",
                video_path.display(),
                e
            ))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FFmpegError::ExecutionFailed(format!(
                "FFmpeg clip extraction failed even with recovery for '{}' at {:.2}s-{:.2}s: {}",
                video_path.display(),
                time_range.start_seconds,
                time_range.start_seconds + time_range.duration_seconds,
                stderr
            )));
        }

        validate_output(output_path)?;
        Ok(())
    }
}

/// Apply fade effect to an extracted clip
fn apply_fade_effect(
    input_path: &Path,
    output_path: &Path,
    duration: f64,
) -> Result<(), FFmpegError> {
    let fade_out_start = duration - fade::FADE_OUT_DURATION;

    if fade_out_start <= fade::FADE_IN_DURATION {
        std::fs::rename(input_path, output_path)
            .map_err(|e| FFmpegError::ExecutionFailed(format!("Failed to rename file: {}", e)))?;
        return Ok(());
    }

    let args = command_builder::build_fade_command(input_path, output_path, duration);

    let output = Command::new("ffmpeg")
        .args(&args)
        .output()
        .map_err(|e| FFmpegError::ExecutionFailed(format!("Failed to apply fade effect: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(FFmpegError::ExecutionFailed(format!(
            "Failed to apply fade effect: {}",
            stderr
        )));
    }

    let _ = std::fs::remove_file(input_path);
    Ok(())
}

/// Validate that the output file exists and has content
fn validate_output(output_path: &Path) -> Result<(), FFmpegError> {
    if !output_path.exists() {
        return Err(FFmpegError::ExecutionFailed(
            "Output file was not created".to_string(),
        ));
    }

    let metadata = std::fs::metadata(output_path)
        .map_err(|e| FFmpegError::ExecutionFailed(format!("Cannot read output file: {}", e)))?;

    if metadata.len() == 0 {
        return Err(FFmpegError::ExecutionFailed(
            "Output file is empty (0 bytes)".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffmpeg_availability() {
        let result = FFmpegExecutor::check_availability();
        match result {
            Ok(_) => println!("FFmpeg is available"),
            Err(FFmpegError::NotFound) => println!("FFmpeg not found"),
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_executor_creation() {
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        assert!(!executor.use_hw_accel);
    }
}
