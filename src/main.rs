// CLI entry point and application orchestration

mod cli;
mod scanner;
mod selector;
mod ffmpeg;
mod processor;
mod progress;
mod error;

use clap::Parser;
use cli::{CliArgs, SelectionStrategy};
use scanner::VideoScanner;
use selector::{ClipSelector, RandomSelector, IntenseAudioSelector};
use ffmpeg::FFmpegExecutor;
use processor::VideoProcessor;
use progress::ProgressReporter;
use error::AppError;
use std::process;
use std::path::Path;

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

fn main() {
    // Parse CLI arguments
    let args = CliArgs::parse();
    
    // Validate directory exists (exit with error code 1 if not)
    if let Err(e) = validate_directory(&args.directory) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
    
    // Check FFmpeg availability (exit with error if not found)
    if let Err(_) = FFmpegExecutor::check_availability() {
        eprintln!("Error: FFmpeg not found in PATH");
        eprintln!("Please install FFmpeg to use this tool.");
        eprintln!("Visit https://ffmpeg.org/download.html for installation instructions.");
        process::exit(1);
    }
    
    // Create VideoScanner and scan for videos
    let scanner = VideoScanner::new(args.directory.clone());
    let scan_result = match scanner.scan() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error scanning directory: {}", e);
            process::exit(1);
        }
    };
    
    // Exit early if no videos found
    if scan_result.videos.is_empty() {
        if !scan_result.skipped_dirs.is_empty() {
            println!("All videos in {} already have backdrop clips.", args.directory.display());
            println!("Skipped {} director{} with existing backdrops.", 
                scan_result.skipped_dirs.len(),
                if scan_result.skipped_dirs.len() == 1 { "y" } else { "ies" }
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
    };
    
    // Create VideoProcessor with selector and executor
    let processor = VideoProcessor::new(selector, ffmpeg_executor);
    
    // Create ProgressReporter
    let mut reporter = ProgressReporter::new();
    
    // Start progress reporting
    reporter.start(videos.len());
    
    // Process each video with progress updates
    for video in &videos {
        let result = processor.process_video(video);
        reporter.update(&result);
    }
    
    // Display final summary
    reporter.finish();
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
