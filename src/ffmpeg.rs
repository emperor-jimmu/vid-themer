// FFmpeg command execution and video processing

use crate::cli::Resolution;
use std::path::Path;

pub struct FFmpegExecutor {
    pub resolution: Resolution,
    pub include_audio: bool,
}

pub struct AudioSegment {
    pub start_time: f64,
    pub duration: f64,
    pub intensity: f64,
}

impl FFmpegExecutor {
    pub fn new(resolution: Resolution, include_audio: bool) -> Self {
        Self {
            resolution,
            include_audio,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FFmpegError {
    #[error("Failed to execute FFmpeg: {0}")]
    ExecutionFailed(String),
    
    #[error("Failed to parse FFmpeg output: {0}")]
    ParseError(String),
    
    #[error("Video has no audio track")]
    NoAudioTrack,
}
