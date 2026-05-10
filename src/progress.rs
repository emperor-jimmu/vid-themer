// Progress reporting and user feedback

use crate::logger::FailureLogger;
use crate::processor::ProcessResult;
use colored::Colorize;
use std::io::Write;

pub struct ProgressReporter {
    pub total: usize,
    pub current: usize,
    pub successful: usize,
    pub failed: usize,
    logger: Option<FailureLogger>,
}

impl ProgressReporter {
    pub fn new() -> Self {
        Self {
            total: 0,
            current: 0,
            successful: 0,
            failed: 0,
            logger: None,
        }
    }

    pub fn with_logger(logger: FailureLogger) -> Self {
        Self {
            total: 0,
            current: 0,
            successful: 0,
            failed: 0,
            logger: Some(logger),
        }
    }

    pub fn start(&mut self, total: usize) {
        self.total = total;
        println!(
            "{} {} videos to process",
            "Found".bright_cyan().bold(),
            total.to_string().bright_yellow().bold()
        );
    }

    /// Report progress for a single clip extraction
    /// This is called during video processing as each clip completes
    /// Must be called while holding the reporter lock to prevent interleaving
    pub fn update_clip_progress(
        &self,
        clip_num: usize,
        total_clips: usize,
        filename: &str,
        video_path: &std::path::Path,
    ) {
        // On first clip, print the header
        if clip_num == 1 {
            println!(
                "{} Processing: {}",
                format!("[{}/{}]", self.current, self.total)
                    .bright_blue()
                    .bold(),
                video_path.display().to_string().bright_white()
            );
        }

        let bar_width = 13;
        let filled = (clip_num * bar_width) / total_clips;
        let empty = bar_width - filled;
        let bar = format!(
            "[{}{}]",
            "=".repeat(filled).bright_green(),
            " ".repeat(empty)
        );

        // Use \r to overwrite the previous line
        print!(
            "\r  {} {} {}",
            filename.bright_cyan().bold(),
            bar,
            format!("{}/{}", clip_num, total_clips).bright_yellow()
        );

        // Flush to ensure immediate display
        let _ = std::io::stdout().flush();

        // On last clip, print newline to move to next line
        if clip_num == total_clips {
            println!();
        }
    }

    pub fn update(&mut self, result: &ProcessResult) {
        if result.success {
            self.successful += 1;
        } else {
            self.failed += 1;
            if let Some(error) = &result.error_message {
                println!("  {} {}", "X".bright_red().bold(), error.bright_red());
            }

            // Log failure to file if logger is available
            if let Some(logger) = &self.logger {
                logger.log_failure(result, result.ffmpeg_stderr.as_deref());
            }
        }
    }

    pub fn finish(&self) {
        println!(
            "{} {} successful, {} failed",
            "Completed:".bright_cyan().bold(),
            self.successful.to_string().bright_green().bold(),
            self.failed.to_string().bright_red().bold()
        );

        if self.failed > 0
            && let Some(logger) = &self.logger
        {
            println!(
                "{} {}",
                "Failure details logged to:".bright_yellow(),
                logger.log_path().display().to_string().bright_white()
            );
        }
    }
}

#[cfg(test)]
#[path = "progress_tests.rs"]
mod tests;
