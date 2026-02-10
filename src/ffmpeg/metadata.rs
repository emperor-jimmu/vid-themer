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
}

/// FFprobe JSON output structure
#[derive(Debug, Deserialize)]
struct FFprobeOutput {
    streams: Vec<FFprobeStream>,
    format: FFprobeFormat,
}

/// FFprobe stream information
#[derive(Debug, Deserialize)]
struct FFprobeStream {
    codec_name: String,
    width: u32,
    height: u32,
    color_transfer: Option<String>,
    pix_fmt: Option<String>,
}

/// FFprobe format information
#[derive(Debug, Deserialize)]
struct FFprobeFormat {
    duration: String,
}

/// Get all video metadata in a single ffprobe call (3x faster than separate calls)
pub fn get_video_metadata(video_path: &Path) -> Result<VideoMetadata, FFmpegError> {
    // Execute ffprobe to get all metadata at once using JSON output
    // Command: ffprobe -v error -select_streams v:0 -show_entries stream=codec_name,width,height,color_transfer,pix_fmt:format=duration -of json <video>
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=codec_name,width,height,color_transfer,pix_fmt:format=duration")
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

        // Check for specific corruption indicators
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

    // Parse JSON output
    let json_str = String::from_utf8_lossy(&output.stdout);
    parse_metadata_json(&json_str, video_path)
}

/// Parse ffprobe JSON output to extract metadata
fn parse_metadata_json(json_str: &str, video_path: &Path) -> Result<VideoMetadata, FFmpegError> {
    // Use serde_json for robust JSON parsing
    let output: FFprobeOutput = serde_json::from_str(json_str).map_err(|e| {
        FFmpegError::ParseError(format!(
            "Failed to parse JSON for '{}': {}",
            video_path.display(),
            e
        ))
    })?;

    // Get the first video stream
    let stream = output.streams.first().ok_or_else(|| {
        FFmpegError::ParseError(format!(
            "No video stream found in JSON for '{}'",
            video_path.display()
        ))
    })?;

    // Validate duration (check for "N/A" or empty)
    if output.format.duration == "N/A" || output.format.duration.is_empty() {
        return Err(FFmpegError::CorruptedFile(format!(
            "Unable to determine video duration for '{}' - file may be corrupted or incomplete",
            video_path.display()
        )));
    }

    // Parse duration string to f64
    let duration = output.format.duration.parse::<f64>().map_err(|e| {
        FFmpegError::ParseError(format!(
            "Failed to parse duration '{}' for '{}': {}",
            output.format.duration,
            video_path.display(),
            e
        ))
    })?;

    Ok(VideoMetadata {
        duration,
        codec: stream.codec_name.clone(),
        width: stream.width,
        height: stream.height,
        color_transfer: stream.color_transfer.clone(),
        pix_fmt: stream.pix_fmt.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_metadata_json_valid() {
        let json = r#"{"streams":[{"codec_name":"h264","width":1920,"height":1080}],"format":{"duration":"123.45"}}"#;
        let video_path = PathBuf::from("/test/video.mp4");

        let result = parse_metadata_json(json, &video_path);
        assert!(result.is_ok());

        let metadata = result.unwrap();
        assert_eq!(metadata.codec, "h264");
        assert_eq!(metadata.width, 1920);
        assert_eq!(metadata.height, 1080);
        assert!((metadata.duration - 123.45).abs() < 0.001);
    }

    #[test]
    fn test_parse_metadata_json_na_duration() {
        let json = r#"{"streams":[{"codec_name":"h264","width":1920,"height":1080}],"format":{"duration":"N/A"}}"#;
        let video_path = PathBuf::from("/test/video.mp4");

        let result = parse_metadata_json(json, &video_path);
        assert!(result.is_err());

        match result {
            Err(FFmpegError::CorruptedFile(msg)) => {
                assert!(msg.contains("Unable to determine video duration"));
            }
            _ => panic!("Expected CorruptedFile error for N/A duration"),
        }
    }

    #[test]
    fn test_parse_metadata_json_no_streams() {
        let json = r#"{"streams":[],"format":{"duration":"123.45"}}"#;
        let video_path = PathBuf::from("/test/video.mp4");

        let result = parse_metadata_json(json, &video_path);
        assert!(result.is_err());

        match result {
            Err(FFmpegError::ParseError(msg)) => {
                assert!(msg.contains("No video stream found"));
            }
            _ => panic!("Expected ParseError for no streams"),
        }
    }
}
