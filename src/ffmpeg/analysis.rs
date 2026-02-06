// Audio and motion intensity analysis

use std::path::Path;
use std::process::Command;

use super::constants::analysis;
use super::error::FFmpegError;

/// Audio segment with intensity information
#[derive(Debug, Clone)]
pub struct AudioSegment {
    pub start_time: f64,
    pub duration: f64,
    pub intensity: f64,
}

/// Motion segment with score information
#[derive(Debug, Clone)]
pub struct MotionSegment {
    pub start_time: f64,
    pub duration: f64,
    pub motion_score: f64,
}

/// Helper function to extract a numeric value after a label in a string
fn extract_value_after(line: &str, label: &str) -> Option<f64> {
    if let Some(pos) = line.find(label) {
        let after_label = &line[pos + label.len()..];
        let value_str: String = after_label
            .trim_start()
            .chars()
            .take_while(|c| c.is_numeric() || *c == '.' || *c == '-')
            .collect();

        value_str.parse::<f64>().ok()
    } else {
        None
    }
}

/// Groups time-series measurements into fixed-duration segments and calculates aggregate scores
fn group_measurements_into_segments<F>(
    measurements: &[(f64, f64)],
    video_duration: f64,
    analysis_duration: f64,
    segment_duration: f64,
    aggregate_fn: F,
) -> Vec<(f64, f64, f64)>
where
    F: Fn(&[f64]) -> f64,
{
    let scale_factor = video_duration / analysis_duration;
    let num_segments = (video_duration / segment_duration).ceil() as usize;

    let mut segments = Vec::new();

    for i in 0..num_segments {
        let segment_start = i as f64 * segment_duration;
        let segment_end = ((i + 1) as f64 * segment_duration).min(video_duration);
        let segment_duration_val = segment_end - segment_start;

        let analyzed_start = segment_start / scale_factor;
        let analyzed_end = segment_end / scale_factor;

        let segment_measurements: Vec<f64> = measurements
            .iter()
            .filter(|(time, _)| *time >= analyzed_start && *time < analyzed_end)
            .map(|(_, value)| *value)
            .collect();

        if !segment_measurements.is_empty() {
            let score = aggregate_fn(&segment_measurements);
            segments.push((segment_start, segment_duration_val, score));
        }
    }

    segments
}

/// Analyze audio intensity across the video duration
pub fn analyze_audio_intensity(
    video_path: &Path,
    duration: f64,
) -> Result<Vec<AudioSegment>, FFmpegError> {
    let analysis_duration = duration.min(analysis::MAX_ANALYSIS_DURATION);

    let args = vec![
        "-i".to_string(),
        video_path.to_string_lossy().to_string(),
        "-t".to_string(),
        analysis_duration.to_string(),
        "-af".to_string(),
        "astats=metadata=1:reset=1,ametadata=print:key=lavfi.astats.Overall.RMS_level:file=-"
            .to_string(),
        "-f".to_string(),
        "null".to_string(),
        "-".to_string(),
    ];

    let output = Command::new("ffmpeg").args(&args).output().map_err(|e| {
        FFmpegError::ExecutionFailed(format!(
            "Failed to execute ffmpeg for audio analysis: {}",
            e
        ))
    })?;

    let stderr = String::from_utf8_lossy(&output.stderr);

    if stderr.contains("Output file #0 does not contain any stream")
        || stderr.contains("Stream specifier ':a' in filtergraph")
        || stderr.contains("does not contain any stream")
    {
        return Err(FFmpegError::NoAudioTrack);
    }

    let mut measurements: Vec<(f64, f64)> = Vec::new();
    let mut current_time = 0.0;

    for line in stderr.lines() {
        #[allow(clippy::collapsible_if)]
        if line.contains("pts_time:") {
            if let Some(time) = extract_value_after(line, "pts_time:") {
                current_time = time;
            }
        }
        #[allow(clippy::collapsible_if)]
        if line.contains("lavfi.astats.Overall.RMS_level") {
            if let Some(level) =
                extract_value_after(line, "lavfi.astats.Overall.RMS_level=")
            {
                measurements.push((current_time, level));
            }
        }
    }

    if measurements.is_empty() {
        return analyze_audio_intensity_fallback(video_path, duration);
    }

    let segments_data = group_measurements_into_segments(
        &measurements,
        duration,
        analysis_duration,
        analysis::SEGMENT_DURATION,
        |values| values.iter().sum::<f64>() / values.len() as f64,
    );

    let mut segments: Vec<AudioSegment> = segments_data
        .into_iter()
        .map(|(start, dur, intensity)| AudioSegment {
            start_time: start,
            duration: dur,
            intensity,
        })
        .collect();

    segments.sort_by(|a, b| b.intensity.total_cmp(&a.intensity));

    Ok(segments)
}

/// Fallback audio analysis using ebur128 filter
fn analyze_audio_intensity_fallback(
    video_path: &Path,
    duration: f64,
) -> Result<Vec<AudioSegment>, FFmpegError> {
    const MAX_ANALYSIS_DURATION: f64 = 180.0;
    let analysis_duration = duration.min(MAX_ANALYSIS_DURATION);

    let output = Command::new("ffmpeg")
        .arg("-t")
        .arg(analysis_duration.to_string())
        .arg("-i")
        .arg(video_path)
        .arg("-filter_complex")
        .arg("ebur128=peak=true")
        .arg("-f")
        .arg("null")
        .arg("-")
        .output()
        .map_err(|e| {
            FFmpegError::ExecutionFailed(format!(
                "Failed to execute ffmpeg for audio analysis: {}",
                e
            ))
        })?;

    let stderr = String::from_utf8_lossy(&output.stderr);

    if stderr.contains("Output file #0 does not contain any stream")
        || stderr.contains("Stream specifier ':a' in filtergraph")
    {
        return Err(FFmpegError::NoAudioTrack);
    }

    let mut measurements: Vec<(f64, f64)> = Vec::new();

    for line in stderr.lines() {
        #[allow(clippy::collapsible_if)]
        if line.contains("Parsed_ebur128") && line.contains("t:") {
            #[allow(clippy::collapsible_if)]
            if let Some(time) = extract_value_after(line, "t:") {
                if let Some(peak) = extract_value_after(line, "FTPK:") {
                    measurements.push((time, peak));
                }
            }
        }
    }

    if measurements.is_empty() {
        return Err(FFmpegError::NoAudioTrack);
    }

    let segments_data = group_measurements_into_segments(
        &measurements,
        duration,
        analysis_duration,
        analysis::SEGMENT_DURATION,
        |values| values.iter().sum::<f64>() / values.len() as f64,
    );

    let mut segments: Vec<AudioSegment> = segments_data
        .into_iter()
        .map(|(start, dur, intensity)| AudioSegment {
            start_time: start,
            duration: dur,
            intensity,
        })
        .collect();

    segments.sort_by(|a, b| b.intensity.total_cmp(&a.intensity));

    Ok(segments)
}

/// Analyze motion intensity using scene detection
pub fn analyze_motion_intensity(
    video_path: &Path,
    duration: f64,
) -> Result<Vec<MotionSegment>, FFmpegError> {
    let analysis_duration = duration.min(analysis::MAX_ANALYSIS_DURATION);

    let args = vec![
        "-i".to_string(),
        video_path.to_string_lossy().to_string(),
        "-t".to_string(),
        analysis_duration.to_string(),
        "-vf".to_string(),
        "select=gt(scene\\,0.3),showinfo".to_string(),
        "-f".to_string(),
        "null".to_string(),
        "-".to_string(),
    ];

    let output = Command::new("ffmpeg").args(&args).output().map_err(|e| {
        FFmpegError::ExecutionFailed(format!(
            "Failed to execute ffmpeg for motion analysis: {}",
            e
        ))
    })?;

    let stderr = String::from_utf8_lossy(&output.stderr);

    let mut measurements: Vec<(f64, f64)> = Vec::new();

    for line in stderr.lines() {
        if line.contains("Parsed_showinfo")
            && line.contains("pts_time:")
            && line.contains("scene:")
            && let Some(time) = extract_value_after(line, "pts_time:")
                && let Some(score) = extract_value_after(line, "scene:") {
                    measurements.push((time, score));
                }
    }

    if measurements.is_empty() {
        return Ok(Vec::new());
    }

    let segments_data = group_measurements_into_segments(
        &measurements,
        duration,
        analysis_duration,
        analysis::SEGMENT_DURATION,
        |values| values.iter().sum::<f64>(),
    );

    let mut segments: Vec<MotionSegment> = segments_data
        .into_iter()
        .map(|(start, dur, score)| MotionSegment {
            start_time: start,
            duration: dur,
            motion_score: score,
        })
        .collect();

    segments.sort_by(|a, b| b.motion_score.total_cmp(&a.motion_score));

    Ok(segments)
}
