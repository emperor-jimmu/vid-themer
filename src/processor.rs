// Video processing pipeline coordination

use crate::ffmpeg::{FFmpegExecutor, probe};
use crate::scanner::VideoFile;
use crate::selector::ClipSelector;
use std::path::PathBuf;

const BACKDROPS_DIR: &str = "backdrops";

pub struct VideoProcessor {
    selector: Box<dyn ClipSelector>,
    ffmpeg: FFmpegExecutor,
    clip_count: u8,
    force: bool,
}

impl VideoProcessor {
    pub fn new(
        selector: Box<dyn ClipSelector>,
        ffmpeg: FFmpegExecutor,
        clip_count: u8,
        force: bool,
    ) -> Self {
        Self {
            selector,
            ffmpeg,
            clip_count,
            force,
        }
    }

    pub fn process_video<F>(&self, video: &VideoFile, mut progress_callback: F) -> ProcessResult
    where
        F: FnMut(usize, usize, &str),
    {
        let video_path = video.path.clone();
        let backdrops_dir = video.parent_dir.join(BACKDROPS_DIR);
        let existing_clip_count = if !self.force && backdrops_dir.exists() {
            self.count_existing_clips(&backdrops_dir)
        } else {
            0
        };

        if existing_clip_count >= self.clip_count {
            if backdrops_dir.exists()
                && let Err(e) = crate::scanner::write_done_marker(&backdrops_dir)
            {
                eprintln!(
                    "Warning: Failed to write done marker for {}: {}",
                    video.path.display(),
                    e
                );
            }
            return ProcessResult {
                video_path,
                output_path: PathBuf::new(),
                success: true,
                error_message: None,
                ffmpeg_stderr: None,
                clips_generated: 0,
            };
        }

        let clips_to_generate = self.clip_count - existing_clip_count;

        let metadata = match probe(&video.path) {
            Ok(metadata) => metadata,
            Err(e) => {
                let error_message = match &e {
                    crate::ffmpeg::FFmpegError::CorruptedFile { .. } => {
                        format!("Skipping corrupted or incomplete video file: {}", e)
                    }
                    _ => format!("Failed to get video duration: {}", e),
                };
                return ProcessResult {
                    video_path,
                    output_path: PathBuf::new(),
                    success: false,
                    error_message: Some(error_message),
                    ffmpeg_stderr: e.stderr().map(|s| s.to_string()),
                    clips_generated: 0,
                };
            }
        };

        let mut time_ranges = self.selector.select(&video.path, metadata.duration);
        if time_ranges.is_empty() {
            return ProcessResult {
                video_path,
                output_path: PathBuf::new(),
                success: false,
                error_message: Some(format!(
                    "No valid clips could be selected (requested: {} clips)",
                    clips_to_generate
                )),
                ffmpeg_stderr: None,
                clips_generated: 0,
            };
        }

        if time_ranges.len() < clips_to_generate as usize {
            eprintln!(
                "Warning: Only generated {} of {} requested clips for {}",
                time_ranges.len(),
                clips_to_generate,
                video.path.display()
            );
        }
        time_ranges.truncate(clips_to_generate as usize);

        let backdrops_dir = match self.create_backdrops_directory(video) {
            Ok(dir) => dir,
            Err(e) => {
                return ProcessResult {
                    video_path,
                    output_path: PathBuf::new(),
                    success: false,
                    error_message: Some(format!("Failed to create output directory: {}", e)),
                    ffmpeg_stderr: None,
                    clips_generated: 0,
                };
            }
        };

        let mut last_output_path = PathBuf::new();
        let total_clips = time_ranges.len();

        for (index, time_range) in time_ranges.iter().enumerate() {
            let clip_num = existing_clip_count as usize + index + 1;
            let output_filename = format!("backdrop{}.mp4", clip_num);
            let output_path = backdrops_dir.join(&output_filename);
            last_output_path = output_path.clone();

            if let Err(e) = self
                .ffmpeg
                .extract_clip(&video.path, time_range, &output_path, &metadata)
            {
                return ProcessResult {
                    video_path,
                    output_path,
                    success: false,
                    error_message: Some(format!(
                        "Failed to extract clip {} of {} (backdrop{}.mp4): {}",
                        index + 1,
                        time_ranges.len(),
                        clip_num,
                        e
                    )),
                    ffmpeg_stderr: e.stderr().map(|s| s.to_string()),
                    clips_generated: index,
                };
            }

            progress_callback(index + 1, total_clips, &output_filename);
        }

        if let Err(e) = crate::scanner::write_done_marker(&backdrops_dir) {
            eprintln!(
                "Warning: Failed to write done marker for {}: {}",
                video.path.display(),
                e
            );
        }

        ProcessResult {
            video_path,
            output_path: last_output_path,
            success: true,
            error_message: None,
            ffmpeg_stderr: None,
            clips_generated: time_ranges.len(),
        }
    }

    fn count_existing_clips(&self, backdrops_dir: &std::path::Path) -> u8 {
        let mut count = 0u8;
        for i in 1..=4 {
            let backdrop_path = backdrops_dir.join(format!("backdrop{}.mp4", i));
            if let Ok(metadata) = std::fs::metadata(&backdrop_path) {
                if metadata.is_file() && metadata.len() > 0 {
                    count += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        count
    }

    fn create_backdrops_directory(&self, video: &VideoFile) -> Result<PathBuf, ProcessError> {
        let backdrops_dir = video.parent_dir.join(BACKDROPS_DIR);
        std::fs::create_dir_all(&backdrops_dir).map_err(|e| {
            ProcessError::OutputDirectoryCreationFailed(format!(
                "Failed to create directory {:?}: {}",
                backdrops_dir, e
            ))
        })?;
        Ok(backdrops_dir)
    }
}

pub struct ProcessResult {
    pub video_path: PathBuf,
    pub output_path: PathBuf,
    pub success: bool,
    pub error_message: Option<String>,
    pub ffmpeg_stderr: Option<String>,
    pub clips_generated: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("Failed to create output directory: {0}")]
    OutputDirectoryCreationFailed(String),
}
