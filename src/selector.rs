// Clip selection strategies (trait + implementations)

use std::path::Path;

pub struct TimeRange {
    pub start_seconds: f64,
    pub duration_seconds: f64,
}

pub trait ClipSelector {
    fn select_segment(
        &self,
        video_path: &Path,
        duration: f64,
    ) -> Result<TimeRange, SelectionError>;
}

pub struct RandomSelector;

pub struct IntenseAudioSelector;

#[derive(Debug, thiserror::Error)]
pub enum SelectionError {
    #[error("Video too short: {0}s")]
    VideoTooShort(f64),
    
    #[error("Failed to analyze audio: {0}")]
    AudioAnalysisFailed(String),
}
