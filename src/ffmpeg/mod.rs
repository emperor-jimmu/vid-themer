// FFmpeg integration module

pub mod analysis;
pub mod command_builder;
pub mod constants;
pub mod error;
pub mod executor;
pub mod metadata;

// Re-export commonly used types
pub use analysis::{
    AudioSegment, MotionSegment, analyze_audio_intensity, analyze_motion_intensity,
};
pub use error::FFmpegError;
pub use executor::FFmpegExecutor;
