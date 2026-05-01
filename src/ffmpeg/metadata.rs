// Video metadata extraction using ffprobe

use serde::Deserialize;
use std::path::Path;
use std::process::Command;

use super::error::FFmpegError;

/// Video metadata information
#[derive(Debug, Clone)]
pub struct VideoMetadata {
    pub duration: f64,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub color_transfer: Option<String>,
    pub pix_fmt: Option<String>,
    /// Index of the preferred audio stream (English first, fallback to first stream)
    pub audio_stream_index: Option<usize>,
}

/// FFprobe JSON output structure
#[derive(Debug, Deserialize)]
struct FFprobeOutput {
    streams: Vec<FFprobeStreamWithIndex>,
    format: FFprobeFormat,
}

/// FFprobe stream with index for audio stream detection
#[derive(Debug, Deserialize)]
struct FFprobeStreamWithIndex {
    index: usize,
    codec_type: Option<String>,
    codec_name: Option<String>,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    color_transfer: Option<String>,
    pix_fmt: Option<String>,
    tags: Option<FFprobeStreamTags>,
}

/// FFprobe format information
#[derive(Debug, Deserialize)]
struct FFprobeFormat {
    duration: String,
}

#[derive(Debug, Deserialize)]
struct FFprobeStreamTags {
    language: Option<String>,
}

/// Get all video metadata in a single ffprobe call
pub fn get_video_metadata(video_path: &Path) -> Result<VideoMetadata, FFmpegError> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("stream=index,codec_type,codec_name,width,height,color_transfer,pix_fmt,tags:format=duration")
        .arg("-of")
        .arg("json")
        .arg(video_path)
        .output()
        .map_err(|e| {
            FFmpegError::ExecutionFailed(format!(
                "Failed to execute ffprobe on '{}': {}",
                video_path.display(),
                e
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);

        if stderr.contains("EBML header parsing failed")
            || stderr.contains("Invalid data found when processing input")
            || stderr.contains("moov atom not found")
            || stderr.contains("End of file")
        {
            return Err(FFmpegError::CorruptedFile(format!(
                "Video file '{}' appears to be corrupted or incomplete: {}",
                video_path.display(),
                stderr
            )));
        }

        return Err(FFmpegError::ExecutionFailed(format!(
            "ffprobe failed on '{}': {}",
            video_path.display(),
            stderr
        )));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let probe: FFprobeOutput = serde_json::from_str(&json_str).map_err(|e| {
        FFmpegError::ParseError(format!("Failed to parse JSON for '{}': {}", video_path.display(), e))
    })?;

    let format_duration = &probe.format.duration;
    if format_duration == "N/A" || format_duration.is_empty() {
        return Err(FFmpegError::CorruptedFile(format!(
            "Unable to determine video duration for '{}' - file may be corrupted or incomplete",
            video_path.display()
        )));
    }

    let duration: f64 = format_duration.parse().map_err(|e| {
        FFmpegError::ParseError(format!("Failed to parse duration '{}' for '{}': {}", format_duration, video_path.display(), e))
    })?;

    let video_stream = probe.streams.iter().find(|s| s.codec_type.as_deref() == Some("video")).ok_or_else(|| {
        FFmpegError::ParseError(format!("No video stream found in '{}'", video_path.display()))
    })?;

    let audio_stream_index = find_preferred_audio_stream_from_streams(&probe.streams);

    Ok(VideoMetadata {
        duration,
        codec: video_stream.codec_name.clone().unwrap_or_default(),
        width: video_stream.width,
        height: video_stream.height,
        color_transfer: video_stream.color_transfer.clone(),
        pix_fmt: video_stream.pix_fmt.clone(),
        audio_stream_index,
    })
}

fn find_preferred_audio_stream_from_streams(streams: &[FFprobeStreamWithIndex]) -> Option<usize> {
    let audio_streams: Vec<&FFprobeStreamWithIndex> = streams
        .iter()
        .filter(|s| s.codec_type.as_deref() == Some("audio"))
        .collect();

    if audio_streams.is_empty() {
        return None;
    }

    let english = audio_streams.iter().find(|s| {
        s.tags
            .as_ref()
            .and_then(|t| t.language.as_deref())
            .map(|lang| lang == "eng" || lang == "en")
            .unwrap_or(false)
    });

    Some(english.copied().unwrap_or(audio_streams[0]).index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_preferred_audio_stream_from_streams_english() {
        let streams = vec![
            FFprobeStreamWithIndex {
                index: 0,
                codec_type: Some("video".to_string()),
                codec_name: Some("h264".to_string()),
                width: 1920,
                height: 1080,
                color_transfer: None,
                pix_fmt: None,
                tags: None,
            },
            FFprobeStreamWithIndex {
                index: 1,
                codec_type: Some("audio".to_string()),
                codec_name: Some("aac".to_string()),
                width: 0,
                height: 0,
                color_transfer: None,
                pix_fmt: None,
                tags: Some(FFprobeStreamTags {
                    language: Some("eng".to_string()),
                }),
            },
            FFprobeStreamWithIndex {
                index: 2,
                codec_type: Some("audio".to_string()),
                codec_name: Some("aac".to_string()),
                width: 0,
                height: 0,
                color_transfer: None,
                pix_fmt: None,
                tags: Some(FFprobeStreamTags {
                    language: Some("spa".to_string()),
                }),
            },
        ];

        let result = find_preferred_audio_stream_from_streams(&streams);
        assert_eq!(result, Some(1));
    }

    #[test]
    fn test_find_preferred_audio_stream_from_streams_no_english() {
        let streams = vec![
            FFprobeStreamWithIndex {
                index: 0,
                codec_type: Some("video".to_string()),
                codec_name: Some("h264".to_string()),
                width: 1920,
                height: 1080,
                color_transfer: None,
                pix_fmt: None,
                tags: None,
            },
            FFprobeStreamWithIndex {
                index: 2,
                codec_type: Some("audio".to_string()),
                codec_name: Some("aac".to_string()),
                width: 0,
                height: 0,
                color_transfer: None,
                pix_fmt: None,
                tags: Some(FFprobeStreamTags {
                    language: Some("spa".to_string()),
                }),
            },
        ];

        let result = find_preferred_audio_stream_from_streams(&streams);
        assert_eq!(result, Some(2));
    }

    #[test]
    fn test_find_preferred_audio_stream_from_streams_no_audio() {
        let streams = vec![
            FFprobeStreamWithIndex {
                index: 0,
                codec_type: Some("video".to_string()),
                codec_name: Some("h264".to_string()),
                width: 1920,
                height: 1080,
                color_transfer: None,
                pix_fmt: None,
                tags: None,
            },
        ];

        let result = find_preferred_audio_stream_from_streams(&streams);
        assert_eq!(result, None);
    }
}
