// FFmpeg integration module

pub mod analysis;
pub mod command_builder;
pub mod constants;
pub mod error;
pub mod executor;
pub mod metadata;

// Re-export commonly used types
#[allow(unused_imports)]
pub use analysis::{analyze_audio_intensity, analyze_motion_intensity, AudioSegment, MotionSegment};
pub use error::FFmpegError;
pub use executor::FFmpegExecutor;
