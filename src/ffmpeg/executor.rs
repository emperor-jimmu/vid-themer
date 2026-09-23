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
use super::metadata::VideoMetadata;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SeekMode {
    Hybrid,
    Conservative,
    Recovery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Attempt {
    pub seek: SeekMode,
    pub include_audio: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    DurationOk,
    DurationMiss,
    Corrupt,
    AudioFail,
    Fail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    Run(Attempt),
    Fade,
    Fail,
}

/// Next extract step given the attempts already tried. Pure: tests feed outcomes, production runs them.
pub(crate) fn decide(want_audio: bool, history: &[(Attempt, Outcome)]) -> Decision {
    let Some((last, outcome)) = history.last() else {
        return Decision::Run(Attempt {
            seek: SeekMode::Hybrid,
            include_audio: want_audio,
        });
    };
    match outcome {
        Outcome::DurationOk => Decision::Fade,
        Outcome::DurationMiss => {
            if history
                .iter()
                .any(|(attempt, _)| attempt.seek == SeekMode::Conservative)
            {
                Decision::Fail
            } else {
                Decision::Run(Attempt {
                    seek: SeekMode::Conservative,
                    include_audio: last.include_audio,
                })
            }
        }
        Outcome::Corrupt => {
            if history
                .iter()
                .any(|(attempt, _)| attempt.seek == SeekMode::Recovery)
            {
                Decision::Fail
            } else {
                Decision::Run(Attempt {
                    seek: SeekMode::Recovery,
                    include_audio: last.include_audio,
                })
            }
        }
        Outcome::AudioFail => {
            let dropped = history.iter().any(|(attempt, _)| !attempt.include_audio);
            if want_audio && last.include_audio && !dropped {
                Decision::Run(Attempt {
                    seek: last.seek,
                    include_audio: false,
                })
            } else {
                Decision::Fail
            }
        }
        Outcome::Fail => Decision::Fail,
    }
}

#[derive(Clone)]
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

    pub fn check_availability() -> Result<(), FFmpegError> {
        match Command::new("ffmpeg").arg("-version").output() {
            Ok(output) if output.status.success() => Ok(()),
            _ => Err(FFmpegError::NotFound),
        }
    }

    pub fn extract_clip(
        &self,
        video_path: &Path,
        time_range: &TimeRange,
        output_path: &Path,
        metadata: &VideoMetadata,
    ) -> Result<(), FFmpegError> {
        let temp_path = output_path.with_file_name(
            output_path
                .file_stem()
                .map(|stem| {
                    let mut name = stem.to_os_string();
                    name.push(format!(".{}.tmp", std::process::id()));
                    name
                })
                .unwrap_or_else(|| OsString::from(format!("tmp.{}.mp4", std::process::id()))),
        );
        let temp_path = temp_path.with_extension("mp4");
        let mut guard = TempFileGuard::new(temp_path.clone());
        let mut history = Vec::new();
        let mut last_stderr = None;

        loop {
            if history.len() > 8 {
                return Err(FFmpegError::failed(
                    format!("Extract gave up for '{}'", video_path.display()),
                    last_stderr,
                ));
            }
            match decide(self.include_audio, &history) {
                Decision::Fade => {
                    apply_fade_effect(&temp_path, output_path, time_range.duration_seconds)?;
                    validate_output(output_path)?;
                    guard.take();
                    return Ok(());
                }
                Decision::Fail => {
                    return Err(FFmpegError::failed(
                        format!(
                            "FFmpeg clip extraction failed for '{}' at {:.2}s",
                            video_path.display(),
                            time_range.start_seconds
                        ),
                        last_stderr,
                    ));
                }
                Decision::Run(attempt) => {
                    let (outcome, stderr) =
                        self.run_attempt(video_path, time_range, &temp_path, metadata, attempt)?;
                    last_stderr = stderr.or(last_stderr);
                    history.push((attempt, outcome));
                }
            }
        }
    }

    fn run_attempt(
        &self,
        video_path: &Path,
        time_range: &TimeRange,
        output_path: &Path,
        metadata: &VideoMetadata,
        attempt: Attempt,
    ) -> Result<(Outcome, Option<String>), FFmpegError> {
        let config = command_builder::ExtractConfig {
            video_path,
            time_range,
            output_path,
            source_resolution: (metadata.width, metadata.height),
            codec: &metadata.codec,
            color_transfer: metadata.color_transfer.as_deref(),
            pix_fmt: None,
            target_resolution: self.resolution.clone(),
            include_audio: attempt.include_audio,
            use_hw_accel: self.use_hw_accel,
            audio_stream_index: metadata.audio_stream_index,
            conservative_seek: attempt.seek == SeekMode::Conservative,
            recovery: attempt.seek == SeekMode::Recovery,
        };
        let args = command_builder::build_extract_command(&config);
        let output = Command::new("ffmpeg").args(&args).output().map_err(|e| {
            FFmpegError::failed(
                format!("Failed to execute ffmpeg for '{}': {}", video_path.display(), e),
                None,
            )
        })?;
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if !output.status.success() {
            if let Some(outcome) = outcome_from_failure(attempt, &stderr) {
                return Ok((outcome, Some(stderr)));
            }
            if let Some(error) = classify_stderr_error(&stderr) {
                return Err(error);
            }
            return Ok((Outcome::Fail, Some(stderr)));
        }
        match duration_matches(output_path, time_range.duration_seconds)? {
            true => Ok((Outcome::DurationOk, None)),
            false => Ok((Outcome::DurationMiss, Some(stderr))),
        }
    }
}

fn is_corrupt(stderr: &str) -> bool {
    stderr.contains("corrupt")
        || stderr.contains("Invalid NAL unit")
        || stderr.contains("concealing")
        || stderr.contains("error while decoding")
        || stderr.contains("missing picture in access unit")
        || stderr.contains("Error submitting packet to decoder")
        || stderr.contains("Error splitting the input into NAL units")
        || stderr.contains("Invalid data found when processing input")
}

fn is_audio_failure(stderr: &str) -> bool {
    stderr.contains("Error submitting packet to decoder")
        || stderr.contains("aac")
        || stderr.contains("Could not open encoder before EOF")
}
fn outcome_from_failure(attempt: Attempt, stderr: &str) -> Option<Outcome> {
    if attempt.seek == SeekMode::Recovery && attempt.include_audio && is_audio_failure(stderr) {
        return Some(Outcome::AudioFail);
    }
    if is_corrupt(stderr) {
        return Some(Outcome::Corrupt);
    }
    if attempt.include_audio && is_audio_failure(stderr) {
        return Some(Outcome::AudioFail);
    }
    None
}

fn classify_stderr_error(stderr: &str) -> Option<FFmpegError> {
    if stderr.contains("Unknown encoder")
        || (stderr.contains("Encoder") && stderr.contains("not found"))
        || stderr.contains("Codec not found")
        || stderr.contains("codec not found")
        || stderr.contains("encoder not found")
        || stderr.contains("unknown encoder")
    {
        return Some(FFmpegError::CodecNotFound(stderr.trim().to_string()));
    }
    if stderr.contains("Unsupported codec")
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

fn apply_fade_effect(input_path: &Path, output_path: &Path, duration: f64) -> Result<(), FFmpegError> {
    let fade_out_start = duration - fade::FADE_OUT_DURATION;
    if fade_out_start <= fade::FADE_IN_DURATION {
        std::fs::rename(input_path, output_path).map_err(|e| {
            FFmpegError::failed(format!("Failed to rename file: {}", e), None)
        })?;
        return Ok(());
    }

    let args = command_builder::build_fade_command(input_path, output_path, duration);
    let output = Command::new("ffmpeg").args(&args).output().map_err(|e| {
        FFmpegError::failed(format!("Failed to apply fade effect: {}", e), None)
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if (stderr.contains("unspecified pixel format")
            || stderr.contains("Cannot determine format")
            || stderr.contains("Could not find codec parameters"))
            && std::fs::rename(input_path, output_path).is_ok()
        {
            return Ok(());
        }
        return Err(FFmpegError::failed(
            "Failed to apply fade effect".to_string(),
            Some(stderr),
        ));
    }
    let _ = std::fs::remove_file(input_path);
    Ok(())
}

fn validate_output(output_path: &Path) -> Result<(), FFmpegError> {
    if !output_path.exists() {
        return Err(FFmpegError::failed("Output file was not created".to_string(), None));
    }
    let metadata = std::fs::metadata(output_path)
        .map_err(|e| FFmpegError::failed(format!("Cannot read output file: {}", e), None))?;
    if metadata.len() == 0 {
        return Err(FFmpegError::failed("Output file is empty (0 bytes)".to_string(), None));
    }
    if metadata.len() < 1024 {
        return Err(FFmpegError::failed(
            format!("Output file is too small ({} bytes), likely corrupted", metadata.len()),
            None,
        ));
    }
    Ok(())
}

fn duration_matches(output_path: &Path, expected_duration: f64) -> Result<bool, FFmpegError> {
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
        .map_err(|e| FFmpegError::failed(format!("Failed to run ffprobe: {}", e), None))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(FFmpegError::failed(
            "ffprobe failed to get clip duration".to_string(),
            Some(stderr),
        ));
    }
    let duration_str = String::from_utf8_lossy(&output.stdout);
    let actual_duration: f64 = duration_str.trim().parse().map_err(|e| {
        FFmpegError::failed(
            format!("Failed to parse clip duration '{}': {}", duration_str.trim(), e),
            None,
        )
    })?;
    Ok((actual_duration - expected_duration).abs() <= 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walk(want_audio: bool, outcomes: &[Outcome]) -> Vec<Decision> {
        let mut history = Vec::new();
        let mut steps = Vec::new();
        loop {
            let decision = decide(want_audio, &history);
            steps.push(decision);
            match decision {
                Decision::Run(attempt) => {
                    history.push((attempt, outcomes[history.len()]));
                }
                Decision::Fade | Decision::Fail => break,
            }
        }
        steps
    }

    #[test]
    fn duration_ok_fades_after_hybrid() {
        let steps = walk(true, &[Outcome::DurationOk]);
        assert_eq!(
            steps,
            vec![
                Decision::Run(Attempt {
                    seek: SeekMode::Hybrid,
                    include_audio: true,
                }),
                Decision::Fade,
            ]
        );
    }

    #[test]
    fn duration_miss_retries_conservative_once() {
        let steps = walk(true, &[Outcome::DurationMiss, Outcome::DurationOk]);
        assert_eq!(steps[1], Decision::Run(Attempt {
            seek: SeekMode::Conservative,
            include_audio: true,
        }));
        assert_eq!(steps[2], Decision::Fade);
    }

    #[test]
    fn second_duration_miss_fails() {
        let steps = walk(true, &[Outcome::DurationMiss, Outcome::DurationMiss]);
        assert_eq!(*steps.last().unwrap(), Decision::Fail);
    }

    #[test]
    fn corrupt_then_ok_uses_recovery_and_fades() {
        let steps = walk(true, &[Outcome::Corrupt, Outcome::DurationOk]);
        assert_eq!(
            steps[1],
            Decision::Run(Attempt {
                seek: SeekMode::Recovery,
                include_audio: true,
            })
        );
        assert_eq!(steps[2], Decision::Fade);
    }

    #[test]
    fn audio_failure_drops_audio_then_fades() {
        let steps = walk(true, &[Outcome::AudioFail, Outcome::DurationOk]);
        assert_eq!(
            steps[1],
            Decision::Run(Attempt {
                seek: SeekMode::Hybrid,
                include_audio: false,
            })
        );
        assert_eq!(steps[2], Decision::Fade);
    }

    #[test]
    fn recovery_audio_failure_drops_audio_and_still_fades() {
        let steps = walk(true, &[Outcome::Corrupt, Outcome::AudioFail, Outcome::DurationOk]);
        assert_eq!(
            steps[2],
            Decision::Run(Attempt {
                seek: SeekMode::Recovery,
                include_audio: false,
            })
        );
        assert_eq!(steps[3], Decision::Fade);
    }
    #[test]
    fn recovery_packet_error_is_an_audio_drop() {
        let stderr = "Error submitting packet to decoder";
        assert_eq!(
            outcome_from_failure(
                Attempt {
                    seek: SeekMode::Hybrid,
                    include_audio: true,
                },
                stderr,
            ),
            Some(Outcome::Corrupt)
        );
        assert_eq!(
            outcome_from_failure(
                Attempt {
                    seek: SeekMode::Recovery,
                    include_audio: true,
                },
                stderr,
            ),
            Some(Outcome::AudioFail)
        );
    }
}
