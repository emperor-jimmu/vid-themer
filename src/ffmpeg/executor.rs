// FFmpeg command execution

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use crate::cli::Resolution;
use crate::selector::TimeRange;
use std::path::PathBuf;

use super::command_builder;
use super::constants::fade;
use super::error::FFmpegError;
use super::metadata::{VideoMetadata, get_video_metadata};

struct TempFileGuard {
    path: Option<PathBuf>,
    should_clean: bool,
}

impl TempFileGuard {
    fn new(path: PathBuf) -> Self {
        Self {
            path: Some(path),
            should_clean: true,
        }
    }

    fn take(&mut self) -> Option<PathBuf> {
        self.should_clean = false;
        self.path.take()
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if self.should_clean
            && let Some(path) = &self.path
        {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// FFmpeg executor with configuration
pub struct FFmpegExecutor {
    pub resolution: Resolution,
    pub include_audio: bool,
    pub use_hw_accel: bool,
}

impl FFmpegExecutor {
    pub fn new(resolution: Resolution, include_audio: bool, use_hw_accel: bool) -> Self {
        Self {
            resolution,
            include_audio,
            use_hw_accel,
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
    fn get_video_metadata(&self, video_path: &Path) -> Result<VideoMetadata, FFmpegError> {
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

        let temp_path = output_path.with_extension(format!("{}.tmp.mp4", std::process::id()));

        let mut _guard = TempFileGuard::new(temp_path.clone());

        match self.extract_clip_internal(
            video_path,
            time_range,
            &temp_path,
            source_resolution,
            codec,
            &metadata,
            false,
        ) {
            Ok(_) => {
                if let Err(e) = validate_clip_duration(&temp_path, time_range.duration_seconds) {
                    eprintln!(
                        "Warning: Initial extraction produced incorrect duration for '{}': {}. Retrying with conservative seeking...",
                        video_path.display(),
                        e
                    );

                    self.extract_clip_internal(
                        video_path,
                        time_range,
                        &temp_path,
                        source_resolution,
                        codec,
                        &metadata,
                        true,
                    )?;

                    validate_clip_duration(&temp_path, time_range.duration_seconds)?;
                }

                apply_fade_effect(&temp_path, output_path, time_range.duration_seconds)?;
                validate_output(output_path)?;
                _guard.take();
                Ok(())
            }
            Err(e) => {
                let stderr = e.stderr().map(|s| s.to_string());

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
            target_resolution: self.resolution,
            include_audio: self.include_audio,
            use_hw_accel: self.use_hw_accel,
            audio_stream_index: metadata.audio_stream_index,
        };

        let args = if use_conservative_seeking {
            // Conservative seeking: only accurate seek, no fast seek
            let mut args: Vec<OsString> =
                vec!["-err_detect".into(), "ignore_err".into(), "-i".into()];
            args.push(video_path.into());
            args.extend([
                "-ss".into(),
                time_range.start_seconds.to_string().into(),
                "-avoid_negative_ts".into(),
                "make_zero".into(),
                "-t".into(),
                time_range.duration_seconds.to_string().into(),
            ]);

            // Add remaining standard args (mapping, codec, filters, etc.)
            let standard_args = command_builder::build_extract_command(&config);
            // Skip to -map (standard args start with -err_detect, -i, seeking, -avoid_negative_ts, -t, then -map)
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

            if let Some(e) = classify_stderr_error(&stderr) {
                return Err(e);
            }

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

    /// Extract clip with error recovery, optionally stripping audio
    fn extract_clip_with_recovery(
        &self,
        video_path: &Path,
        time_range: &TimeRange,
        output_path: &Path,
        source_resolution: (u32, u32),
        codec: &str,
    ) -> Result<(), FFmpegError> {
        self.extract_clip_error_tolerant(
            video_path,
            time_range,
            output_path,
            source_resolution,
            codec,
            self.include_audio,
        )
    }

    /// Error-tolerant extraction with configurable audio
    fn extract_clip_error_tolerant(
        &self,
        video_path: &Path,
        time_range: &TimeRange,
        output_path: &Path,
        source_resolution: (u32, u32),
        codec: &str,
        include_audio: bool,
    ) -> Result<(), FFmpegError> {
        // Get metadata for color information
        let metadata = self.get_video_metadata(video_path)?;

        let config = command_builder::ExtractConfig {
            video_path,
            time_range,
            output_path,
            source_resolution,
            codec,
            color_transfer: metadata.color_transfer.as_deref(),
            target_resolution: self.resolution,
            include_audio,
            use_hw_accel: self.use_hw_accel,
            audio_stream_index: metadata.audio_stream_index,
        };

        // Prepend extra error-tolerance flags before the standard args.
        let standard_args = command_builder::build_extract_command(&config);
        let mut args: Vec<OsString> = vec![
            "-fflags".into(),
            "+genpts+igndts".into(),
            "-max_error_rate".into(),
            "1.0".into(),
        ];
        args.extend(standard_args);

        let output = Command::new("ffmpeg").args(&args).output().map_err(|e| {
            FFmpegError::ExecutionFailed(format!(
                "Failed to execute ffmpeg recovery for '{}': {}",
                video_path.display(),
                e
            ))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            if let Some(e) = classify_stderr_error(&stderr) {
                return Err(e);
            }

            // If audio decoding failed and we haven't already stripped it, try without audio
            if include_audio
                && (stderr.contains("Error submitting packet to decoder")
                    || stderr.contains("aac")
                    || stderr.contains("Could not open encoder before EOF"))
            {
                return self.extract_clip_error_tolerant(
                    video_path,
                    time_range,
                    output_path,
                    source_resolution,
                    codec,
                    false,
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
}

/// Classify an FFmpeg stderr string into a typed error.
/// Returns None if none of the known patterns match (caller should use a generic error).
fn classify_stderr_error(stderr: &str) -> Option<FFmpegError> {
    // Use contains() directly — FFmpeg messages have consistent casing, no need to lowercase the whole string.
    if stderr.contains("Unknown encoder")
        || (stderr.contains("Encoder") && stderr.contains("not found"))
        || stderr.contains("Codec not found")
        || stderr.contains("codec not found")
        || stderr.contains("encoder not found")
        || stderr.contains("unknown encoder")
    {
        return Some(FFmpegError::CodecNotFound(stderr.trim().to_string()));
    }
    if stderr.contains("Invalid data found when processing input")
        || stderr.contains("Unsupported codec")
        || stderr.contains("unsupported codec")
        || stderr.contains("Invalid argument")
        || stderr.contains("invalid argument")
    {
        return Some(FFmpegError::InvalidFormat(stderr.trim().to_string()));
    }
    if stderr.contains("Hardware acceleration")
        || stderr.contains("hardware acceleration")
        || stderr.contains("Failed to load")
        || stderr.contains("failed to load")
        || stderr.contains("not available for this device")
    {
        return Some(FFmpegError::HWAccelNotAvailable(stderr.trim().to_string()));
    }
    None
}

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
        ])
        .arg(output_path)
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
    use std::fs;

    fn make_temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "executor_test_{}_{}_{}",
            name,
            std::process::id(),
            rand::random::<u32>()
        ))
    }

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
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true, false);
        assert!(!executor.use_hw_accel);
    }

    #[test]
    fn test_classify_stderr_error_codec_not_found() {
        let err = classify_stderr_error("Unknown encoder 'h264_videotoolbox'");
        assert!(matches!(err, Some(FFmpegError::CodecNotFound(_))));
    }

    #[test]
    fn test_classify_stderr_error_invalid_format() {
        let err = classify_stderr_error("Invalid data found when processing input");
        assert!(matches!(err, Some(FFmpegError::InvalidFormat(_))));
    }

    #[test]
    fn test_classify_stderr_error_hw_accel_not_available() {
        let err = classify_stderr_error("Hardware acceleration not available for this device");
        assert!(matches!(err, Some(FFmpegError::HWAccelNotAvailable(_))));
    }

    #[test]
    fn test_classify_stderr_error_unknown_returns_none() {
        let err = classify_stderr_error("some unrelated ffmpeg output");
        assert!(err.is_none());
    }

    #[test]
    fn test_validate_output_missing_file() {
        let path = make_temp_path("missing.mp4");
        let err = validate_output(&path).unwrap_err();
        assert!(matches!(err, FFmpegError::ExecutionFailed(_)));
    }

    #[test]
    fn test_validate_output_zero_byte_file() {
        let path = make_temp_path("zero.mp4");
        fs::File::create(&path).unwrap();

        let err = validate_output(&path).unwrap_err();
        assert!(matches!(err, FFmpegError::ExecutionFailed(_)));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_validate_output_too_small_file() {
        let path = make_temp_path("tiny.mp4");
        fs::write(&path, b"small").unwrap();

        let err = validate_output(&path).unwrap_err();
        assert!(matches!(err, FFmpegError::ExecutionFailed(_)));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_validate_output_valid_size_file() {
        let path = make_temp_path("valid.mp4");
        fs::write(&path, vec![0u8; 2048]).unwrap();

        assert!(validate_output(&path).is_ok());

        let _ = fs::remove_file(&path);
    }
}
