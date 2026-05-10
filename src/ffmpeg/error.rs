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
    pub fn stderr(&self) -> Option<&str> {
        match self {
            FFmpegError::ExecutionFailed(msg) => {
                // Try to extract stderr by finding the last ": " separator after a path/context prefix
                // Common formats:
                //   "... for '<path>' at <start>s-<end>s: <stderr>"
                //   "... on '<path>': <stderr>"
                //   "... for '<path>': <stderr>"
                // Fall back to entire message if no known separator found.
                if let Some(idx) = msg.find("': ") {
                    Some(&msg[idx + 3..])
                } else {
                    Some(msg.as_str())
                }
            }
            _ => None,
        }
    }
}
