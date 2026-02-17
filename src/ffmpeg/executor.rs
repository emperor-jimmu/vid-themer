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

        // First attempt: Standard extraction with hybrid seeking
        match self.extract_clip_internal(
            video_path,
            time_range,
            &temp_path,
            source_resolution,
            codec,
            &metadata,
            false, // use_conservative_seeking
        ) {
            Ok(_) => {
                // Validate duration
                if let Err(e) = validate_clip_duration(&temp_path, time_range.duration_seconds) {
                    eprintln!(
                        "Warning: Initial extraction produced incorrect duration for '{}': {}. Retrying with conservative seeking...",
                        video_path.display(),
                        e
                    );
                    let _ = std::fs::remove_file(&temp_path);

                    // Retry with conservative seeking (no fast seek)
                    self.extract_clip_internal(
                        video_path,
                        time_range,
                        &temp_path,
                        source_resolution,
                        codec,
                        &metadata,
                        true, // use_conservative_seeking
                    )?;

                    validate_clip_duration(&temp_path, time_range.duration_seconds)?;
                }

                apply_fade_effect(&temp_path, output_path, time_range.duration_seconds)?;
                validate_output(output_path)?;
                Ok(())
            }
            Err(e) => {
                let stderr = e.stderr().map(|s| s.to_string());
                let _ = std::fs::remove_file(&temp_path);

                if stderr.as_ref().is_some_and(|s| {
                    s.contains("corrupt")
                        || s.contains("Invalid NAL unit")
                        || s.contains("concealing")
                        || s.contains("error while decoding")
                        || s.contains("missing picture in access unit")
                        || s.contains("Error submitting packet to decoder")
                        || s.contains("Error splitting the input into NAL units")
                        || s.contains("Invalid data found when processing input")
                }) {
                    return self.extract_clip_with_recovery(
                        video_path,
                        time_range,
                        output_path,
                        source_resolution,
                        codec,
                    );
                }

                Err(e)
            }
        }
    }

    /// Internal extraction method with configurable seeking strategy
    #[allow(clippy::too_many_arguments)]
    fn extract_clip_internal(
        &self,
        video_path: &Path,
        time_range: &TimeRange,
        output_path: &Path,
        source_resolution: (u32, u32),
        codec: &str,
        metadata: &VideoMetadata,
        use_conservative_seeking: bool,
    ) -> Result<(), FFmpegError> {
        let config = command_builder::ExtractConfig {
            video_path,
            time_range,
            output_path,
            source_resolution,
            codec,
            color_transfer: metadata.color_transfer.as_deref(),
            pix_fmt: metadata.pix_fmt.as_deref(),
            target_resolution: self.resolution.clone(),
            include_audio: self.include_audio,
            use_hw_accel: self.use_hw_accel,
        };

        let args = if use_conservative_seeking {
            // Conservative seeking: only accurate seek, no fast seek
            let mut args = vec!["-err_detect".to_string(), "ignore_err".to_string()];

            // Input file
            args.extend(vec![
                "-i".to_string(),
                video_path.to_string_lossy().to_string(),
            ]);

            // Accurate seek only
            args.extend(vec![
                "-ss".to_string(),
                time_range.start_seconds.to_string(),
            ]);

            // Build rest of command
            args.extend(vec![
                "-avoid_negative_ts".to_string(),
                "make_zero".to_string(),
                "-t".to_string(),
                time_range.duration_seconds.to_string(),
            ]);

            // Add remaining standard args (mapping, codec, filters, etc.)
            let standard_args = command_builder::build_extract_command(&config);
            // Skip the first parts we already added and add the rest
            let skip_until = standard_args.iter().position(|s| s == "-map").unwrap_or(0);
            args.extend(standard_args.into_iter().skip(skip_until));

            args
        } else {
            // Standard hybrid seeking
            command_builder::build_extract_command(&config)
        };

        let output = Command::new("ffmpeg").args(&args).output().map_err(|e| {
            FFmpegError::ExecutionFailed(format!(
                "Failed to execute ffmpeg for '{}': {}",
                video_path.display(),
                e
            ))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FFmpegError::ExecutionFailed(format!(
                "FFmpeg clip extraction failed for '{}' at {:.2}s-{:.2}s: {}",
                video_path.display(),
                time_range.start_seconds,
                time_range.start_seconds + time_range.duration_seconds,
                stderr
            )));
        }

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
        // Get metadata for color information
        let metadata = self.get_video_metadata(video_path)?;

        // First attempt: Try with audio but with error tolerance
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
            color_transfer: metadata.color_transfer.as_deref(),
            pix_fmt: metadata.pix_fmt.as_deref(),
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

            // If audio decoding failed, try without audio
            if self.include_audio
                && (stderr.contains("Error submitting packet to decoder")
                    || stderr.contains("aac")
                    || stderr.contains("Could not open encoder before EOF"))
            {
                return self.extract_clip_without_audio(
                    video_path,
                    time_range,
                    output_path,
                    source_resolution,
                    codec,
                    &metadata,
                );
            }

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

    /// Extract clip without audio (last resort for corrupted audio streams)
    fn extract_clip_without_audio(
        &self,
        video_path: &Path,
        time_range: &TimeRange,
        output_path: &Path,
        source_resolution: (u32, u32),
        codec: &str,
        metadata: &VideoMetadata,
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
            color_transfer: metadata.color_transfer.as_deref(),
            pix_fmt: metadata.pix_fmt.as_deref(),
            target_resolution: self.resolution.clone(),
            include_audio: false, // Force no audio
            use_hw_accel: self.use_hw_accel,
        };

        let standard_args = command_builder::build_extract_command(&config);
        args.extend(standard_args.into_iter().skip(2));

        let output = Command::new("ffmpeg").args(&args).output().map_err(|e| {
            FFmpegError::ExecutionFailed(format!(
                "Failed to execute ffmpeg without audio for '{}': {}",
                video_path.display(),
                e
            ))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FFmpegError::ExecutionFailed(format!(
                "FFmpeg clip extraction failed even without audio for '{}' at {:.2}s-{:.2}s: {}",
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

        // If fade fails due to corrupted input, just use the file without fade
        if stderr.contains("unspecified pixel format")
            || stderr.contains("Cannot determine format")
            || stderr.contains("Could not find codec parameters")
        {
            // Try to rename the temp file to output (skip fade)
            if std::fs::rename(input_path, output_path).is_ok() {
                return Ok(());
            }
        }

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

    // Basic validation: file should be at least 1KB for a valid video
    if metadata.len() < 1024 {
        return Err(FFmpegError::ExecutionFailed(format!(
            "Output file is too small ({} bytes), likely corrupted",
            metadata.len()
        )));
    }

    Ok(())
}

/// Validate that the extracted clip has the expected duration
/// Allows for a small tolerance (0.5 seconds) to account for keyframe alignment
fn validate_clip_duration(output_path: &Path, expected_duration: f64) -> Result<(), FFmpegError> {
    // Use ffprobe to get the actual duration of the extracted clip
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            output_path.to_string_lossy().as_ref(),
        ])
        .output()
        .map_err(|e| FFmpegError::ExecutionFailed(format!("Failed to run ffprobe: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(FFmpegError::ExecutionFailed(format!(
            "ffprobe failed to get clip duration: {}",
            stderr
        )));
    }

    let duration_str = String::from_utf8_lossy(&output.stdout);
    let actual_duration: f64 = duration_str.trim().parse().map_err(|e| {
        FFmpegError::ExecutionFailed(format!(
            "Failed to parse clip duration '{}': {}",
            duration_str.trim(),
            e
        ))
    })?;

    // Allow 0.5 second tolerance for keyframe alignment
    const DURATION_TOLERANCE: f64 = 0.5;
    let duration_diff = (actual_duration - expected_duration).abs();

    if duration_diff > DURATION_TOLERANCE {
        return Err(FFmpegError::ExecutionFailed(format!(
            "Extracted clip duration ({:.2}s) differs significantly from expected ({:.2}s). \
             This may indicate seeking issues or keyframe problems in the source video.",
            actual_duration, expected_duration
        )));
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
