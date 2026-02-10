// CLI entry point and application orchestration

mod cli;
mod error;
mod ffmpeg;
mod logger;
mod processor;
mod progress;
mod scanner;
mod selector;

use clap::Parser;
use cli::{CliArgs, SelectionStrategy};
use error::AppError;
use ffmpeg::FFmpegExecutor;
use logger::FailureLogger;
use processor::VideoProcessor;
use progress::ProgressReporter;
use rayon::prelude::*;
use scanner::VideoScanner;
use selector::{ActionSelector, ClipSelector, IntenseAudioSelector, RandomSelector};
use std::path::Path;
use std::process;
use std::sync::{Arc, Mutex};

/// Validate that the provided directory path exists and is a directory
fn validate_directory(path: &Path) -> Result<(), AppError> {
    if !path.exists() {
        return Err(AppError::DirectoryNotFound(path.to_path_buf()));
    }

    if !path.is_dir() {
        return Err(AppError::DirectoryNotFound(path.to_path_buf()));
    }

    Ok(())
}

/// Helper function to handle errors and exit with appropriate error code
/// Prints error message and exits the process with code 1
fn exit_on_error<T, E: std::fmt::Display>(result: Result<T, E>, context: &str) -> T {
    match result {
        Ok(value) => value,
        Err(e) => {
            eprintln!("Error {}: {}", context, e);
            process::exit(1);
        }
    }
}

fn main() {
    // Parse CLI arguments
    let args = CliArgs::parse();

    // Validate duration range
    if let Err(e) = args.validate_duration_range() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }

    // Validate exclusion zones
    if let Err(e) = args.validate_exclusion_zones() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }

    // Validate directory exists (exit with error code 1 if not)
    exit_on_error(validate_directory(&args.directory), "validating directory");

    // Check FFmpeg availability (exit with error if not found)
    if FFmpegExecutor::check_availability().is_err() {
        eprintln!("Error: FFmpeg not found in PATH");
        eprintln!("Please install FFmpeg to use this tool.");
        eprintln!("Visit https://ffmpeg.org/download.html for installation instructions.");
        process::exit(1);
    }

    // Create VideoScanner and scan for videos
    let scanner = VideoScanner::new(args.directory.clone());
    let scan_result = exit_on_error(scanner.scan(), "scanning directory");

    // Exit early if no videos found
    if scan_result.videos.is_empty() {
        if !scan_result.skipped_dirs.is_empty() {
            println!(
                "All videos in {} already have backdrop clips.",
                args.directory.display()
            );
            println!(
                "Skipped {} director{} with existing backdrops.",
                scan_result.skipped_dirs.len(),
                if scan_result.skipped_dirs.len() == 1 {
                    "y"
                } else {
                    "ies"
                }
            );
        } else {
            println!("No videos found in {}", args.directory.display());
        }
        return;
    }

    let videos = scan_result.videos;

    // Create FFmpegExecutor with resolution and audio settings
    let ffmpeg_executor = FFmpegExecutor::new(args.resolution.clone(), args.include_audio);

    // Create appropriate ClipSelector based on strategy flag
    let selector: Box<dyn ClipSelector> = match args.strategy {
        SelectionStrategy::Random => Box::new(RandomSelector),
        SelectionStrategy::IntenseAudio => {
            Box::new(IntenseAudioSelector::new(ffmpeg_executor.clone()))
        }
        SelectionStrategy::Action => Box::new(ActionSelector::new(ffmpeg_executor.clone())),
    };

    // Create ClipConfig from CLI arguments
    let clip_config = selector::ClipConfig {
        min_duration: args.min_duration,
        max_duration: args.max_duration,
    };

    // Create VideoProcessor with selector and executor
    let processor = Arc::new(VideoProcessor::new(
        selector,
        ffmpeg_executor,
        args.intro_exclusion_percent,
        args.outro_exclusion_percent,
        args.clip_count,
        clip_config,
    ));

    // Create ProgressReporter with logger
    let logger = match FailureLogger::new(&args.directory) {
        Ok(logger) => Some(logger),
        Err(e) => {
            eprintln!("Warning: Failed to create failure log: {}", e);
            eprintln!("Continuing without failure logging...");
            None
        }
    };

    let mut reporter = if let Some(logger) = logger {
        ProgressReporter::with_logger(logger)
    } else {
        ProgressReporter::new()
    };

    // Start progress reporting
    reporter.start(videos.len());

    // Wrap reporter in Arc<Mutex<>> for thread-safe access
    let reporter = Arc::new(Mutex::new(reporter));

    // Configure rayon thread pool based on CPU cores
    // Use 75% of available cores to avoid resource exhaustion
    // Cap at 8 threads since FFmpeg operations are I/O-bound and FFmpeg spawns its own threads
    let num_cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4); // Default to 4 if detection fails
    let thread_count = (num_cpus * 3 / 4).clamp(1, 8); // At least 1, at most 8 threads

    rayon::ThreadPoolBuilder::new()
        .num_threads(thread_count)
        .build_global()
        .unwrap_or_else(|e| {
            eprintln!("Warning: Failed to configure thread pool: {}", e);
            eprintln!("Continuing with default thread pool...");
        });

    // Process videos in parallel with progress updates
    videos.par_iter().for_each(|video| {
        // Print the video header first (with lock)
        if let Ok(mut reporter) = reporter.lock() {
            reporter.start_video(&video.path);
        }

        // Create a closure that captures the reporter for per-clip progress
        let clip_progress = |clip_num: usize, total_clips: usize, filename: &str| {
            if let Ok(reporter) = reporter.lock() {
                reporter.update_clip_progress(clip_num, total_clips, filename);
            }
        };

        let result = processor.process_video(video, clip_progress);

        // Lock the reporter to update final status
        if let Ok(mut reporter) = reporter.lock() {
            reporter.update(&result);
        }
    });

    // Display final summary
    if let Ok(reporter) = reporter.lock() {
        reporter.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_directory_not_found_error() {
        // Test error handling when directory doesn't exist
        let non_existent_path = PathBuf::from("/this/path/definitely/does/not/exist/12345");

        let result = validate_directory(&non_existent_path);

        assert!(result.is_err());
        let err = result.unwrap_err();

        // Verify it's the correct error type
        match err {
            AppError::DirectoryNotFound(path) => {
                assert_eq!(path, non_existent_path);
            }
            _ => panic!("Expected DirectoryNotFound error, got: {:?}", err),
        }
    }

    #[test]
    fn test_path_is_not_directory() {
        // Test error handling when path exists but is not a directory
        // Use Cargo.toml as a file that definitely exists
        let file_path = PathBuf::from("Cargo.toml");

        let result = validate_directory(&file_path);

        assert!(result.is_err());
        let err = result.unwrap_err();

        // Verify it's the correct error type
        match err {
            AppError::DirectoryNotFound(path) => {
                assert_eq!(path, file_path);
            }
            _ => panic!("Expected DirectoryNotFound error, got: {:?}", err),
        }
    }

    #[test]
    fn test_valid_directory() {
        // Test that valid directory passes validation
        // Use src directory which should exist
        let valid_path = PathBuf::from("src");

        let result = validate_directory(&valid_path);

        assert!(result.is_ok());
    }
}
