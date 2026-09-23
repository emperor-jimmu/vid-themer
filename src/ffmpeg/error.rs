// FFmpeg error types

#[derive(Debug, thiserror::Error)]
pub enum FFmpegError {
    #[error("FFmpeg not found in PATH")]
    NotFound,

    #[error("{message}")]
    ExecutionFailed {
        message: String,
        stderr: Option<String>,
    },

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

    #[error("{message}")]
    CorruptedFile {
        message: String,
        stderr: Option<String>,
    },
}

impl FFmpegError {
    pub fn failed(message: impl Into<String>, stderr: Option<String>) -> Self {
        Self::ExecutionFailed {
            message: message.into(),
            stderr,
        }
    }

    pub fn corrupted(message: impl Into<String>, stderr: Option<String>) -> Self {
        Self::CorruptedFile {
            message: message.into(),
            stderr,
        }
    }

    pub fn stderr(&self) -> Option<&str> {
        match self {
            FFmpegError::ExecutionFailed { stderr, .. }
            | FFmpegError::CorruptedFile { stderr, .. } => stderr.as_deref(),
            _ => None,
        }
    }
}
