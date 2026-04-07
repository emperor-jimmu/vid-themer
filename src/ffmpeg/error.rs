// FFmpeg error types

#[derive(Debug, thiserror::Error)]
pub enum FFmpegError {
    #[error("FFmpeg not found in PATH")]
    NotFound,

    #[error("Failed to execute FFmpeg: {0}")]
    ExecutionFailed(String),

    #[error("Failed to parse FFmpeg output: {0}")]
    ParseError(String),

    #[error("Video has no audio track")]
    NoAudioTrack,

    #[error("Codec not found or not supported: {0}")]
    CodecNotFound(String),

    #[error("Invalid or unsupported video format: {0}")]
    InvalidFormat(String),

    #[error("Hardware acceleration not available: {0}")]
    HWAccelNotAvailable(String),

    #[error("Corrupted or invalid video file: {0}")]
    CorruptedFile(String),
}

impl FFmpegError {
    /// Extracts stderr output from execution errors
    ///
    /// Returns the stderr content if this is an ExecutionFailed error.
    /// For other error types, returns None.
    ///
    /// # Behavior
    /// - For ExecutionFailed errors, attempts to strip known prefixes to extract raw stderr
    /// - If no known prefix is found, returns the entire error message
    /// - For all other error variants, returns None
    pub fn stderr(&self) -> Option<&str> {
        match self {
            FFmpegError::ExecutionFailed(msg) => {
                // Try to extract stderr by stripping known prefixes
                // Format: "FFmpeg clip extraction failed for '<path>' at <start>s-<end>s: <stderr>"
                msg.strip_prefix("FFmpeg clip extraction failed for ")
                    .and_then(|s| s.split_once(": ").map(|(_, stderr)| stderr))
                    // Format: "FFmpeg clip extraction failed even with recovery for '<path>' at <start>s-<end>s: <stderr>"
                    .or_else(|| {
                        msg.strip_prefix("FFmpeg clip extraction failed even with recovery for ")
                            .and_then(|s| s.split_once(": ").map(|(_, stderr)| stderr))
                    })
                    // Format: "ffprobe failed on '<path>': <stderr>"
                    .or_else(|| {
                        msg.strip_prefix("ffprobe failed on ")
                            .and_then(|s| s.split_once(": ").map(|(_, stderr)| stderr))
                    })
                    // Format: "Failed to execute ffprobe on '<path>': <stderr>"
                    .or_else(|| {
                        msg.strip_prefix("Failed to execute ffprobe on ")
                            .and_then(|s| s.split_once(": ").map(|(_, stderr)| stderr))
                    })
                    // Format: "Failed to execute ffmpeg for '<path>': <stderr>"
                    .or_else(|| {
                        msg.strip_prefix("Failed to execute ffmpeg for ")
                            .and_then(|s| s.split_once(": ").map(|(_, stderr)| stderr))
                    })
                    // Format: "Failed to execute ffmpeg recovery for '<path>': <stderr>"
                    .or_else(|| {
                        msg.strip_prefix("Failed to execute ffmpeg recovery for ")
                            .and_then(|s| s.split_once(": ").map(|(_, stderr)| stderr))
                    })
                    // Fallback: return entire message if no known prefix matches
                    .or(Some(msg.as_str()))
            }
            _ => None,
        }
    }
}
