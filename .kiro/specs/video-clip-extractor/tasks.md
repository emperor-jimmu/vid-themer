# Implementation Plan: Video Clip Extractor

## Overview

This implementation plan breaks down the video clip extractor into discrete coding tasks. The approach follows a bottom-up strategy: build core utilities first, then selection strategies, then the processing pipeline, and finally wire everything together with the CLI interface. Each task builds incrementally on previous work, with property-based tests integrated throughout to validate correctness early.

## Tasks

- [x] 1. Set up project structure and dependencies
  - Create new Rust project with Cargo (edition 2024)
  - Add dependencies: `clap` (with derive feature), `thiserror`, `proptest` (dev dependency)
  - Create module structure: `cli`, `scanner`, `selector`, `ffmpeg`, `processor`, `progress`
  - Set up basic error types using `thiserror`
  - _Requirements: All_

- [ ] 2. Implement FFmpeg executor core functionality
  - [x] 2.1 Implement FFmpeg availability check
    - Create `FFmpegExecutor` struct with configuration fields
    - Implement method to check if FFmpeg is in PATH
    - Return appropriate error if FFmpeg not found
    - _Requirements: 2.11_

  - [x] 2.2 Implement video duration detection
    - Use `ffprobe` to query video duration
    - Parse duration output to f64 (handle fractional seconds)
    - Handle errors for corrupted or invalid videos
    - _Requirements: 9.1, 9.2, 9.3, 9.4_

  - [x] 2.3 Write property test for duration parsing

    - **Property 19: Duration Parsing Correctness**
    - **Validates: Requirements 9.2, 9.4**

  - [x] 2.4 Implement video resolution detection
    - Use `ffprobe` to query video width and height
    - Parse resolution output to (u32, u32)
    - Handle errors for invalid videos
    - _Requirements: 2.5, 2.6, 2.7_

  - [x] 2.5 Implement scale filter calculation
    - Determine target resolution based on configuration
    - Return None if source resolution is smaller (no upscaling)
    - Generate FFmpeg scale filter string with letterboxing
    - _Requirements: 2.5, 2.6, 2.7, 2.8_

  - [x] 2.6 Write property test for no upscaling

    - **Property 7: No Upscaling**
    - **Validates: Requirements 2.5, 2.6, 2.7**

  - [x] 2.7 Implement clip extraction command builder
    - Build FFmpeg command with start time, duration, scaling, audio options
    - Handle audio inclusion/exclusion based on configuration
    - Use appropriate codec settings (libx264, preset fast)
    - _Requirements: 2.1, 2.2, 2.9, 2.10_

  - [x] 2.8 Implement clip extraction execution
    - Execute FFmpeg command using `std::process::Command`
    - Capture stderr for error messages
    - Return appropriate errors on failure
    - _Requirements: 2.1, 2.2, 2.3_

  - [x] 2.9 Write property test for extracted clip duration

    - **Property 5: Extracted Clip Duration**
    - **Validates: Requirements 2.1**

  - [x] 2.10 Write property test for audio inclusion control

    - **Property 9: Audio Inclusion Control**
    - **Validates: Requirements 2.9, 2.10**

  - [x] 2.11 Write unit test for FFmpeg not found error

    - Test error handling when FFmpeg is not in PATH
    - _Requirements: 2.11_

  - [x] 2.12 Write unit test for short video edge case

    - Test that videos < 5 seconds are extracted in full
    - _Requirements: 2.4_

- [ ] 3. Implement selection strategies
  - [x] 3.1 Define ClipSelector trait
    - Create trait with `select_segment` method
    - Define `TimeRange` struct for start time and duration
    - Define `SelectionError` enum
    - _Requirements: 3.1, 3.2, 4.1_

  - [x] 3.2 Implement RandomSelector
    - Calculate valid time range (exclude first 60s and last 240s)
    - Generate random start time within valid bounds
    - Handle edge case: video too short for exclusions (use middle segment)
    - Ensure clip fits within video duration
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

  - [x] 3.3 Write property test for random selection valid bounds

    - **Property 10: Random Selection Valid Bounds**
    - **Validates: Requirements 3.1, 3.2, 3.3, 3.4**

  - [x] 3.4 Write property test for random selection variety

    - **Property 11: Random Selection Variety**
    - **Validates: Requirements 3.6**

  - [ ]* 3.5 Write unit test for short video fallback
    - Test middle segment selection when video is too short for exclusions
    - _Requirements: 3.5_

  - [x] 3.6 Implement audio analysis helper in FFmpegExecutor
    - Execute FFmpeg with ebur128 filter to analyze audio levels
    - Parse output to extract time and peak/RMS values
    - Group into segments and calculate intensity
    - Return sorted list of audio segments by intensity
    - _Requirements: 4.2, 4.5_

  - [x] 3.7 Implement IntenseAudioSelector
    - Use FFmpegExecutor to analyze audio intensity
    - Select segment with highest audio intensity
    - Handle tie-breaking (select first occurrence)
    - Fall back to middle segment if no audio track
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

  - [x] 3.8 Write unit test for no audio fallback

    - Test fallback to middle segment when video has no audio
    - _Requirements: 4.4_

  - [ ]* 3.9 Write unit test for tie-breaking behavior
    - Test that first occurrence is selected when multiple segments have similar intensity
    - _Requirements: 4.3_

- [x] 4. Checkpoint - Ensure core extraction logic works
  - Ensure all tests pass, ask the user if questions arise.

- [x] 5. Implement video scanner
  - [x] 5.1 Create VideoFile struct and VideoScanner
    - Define `VideoFile` struct with path and parent_dir fields
    - Create `VideoScanner` struct with root_path field
    - Define `ScanError` enum
    - _Requirements: 1.1_

  - [x] 5.2 Implement directory skip logic
    - Check if directory contains backdrops/backdrop.mp4
    - Return true if exists (skip directory)
    - _Requirements: 1.4_

  - [ ]* 5.3 Write property test for skip directories with existing clips
    - **Property 3: Skip Directories with Existing Clips**
    - **Validates: Requirements 1.4**

  - [x] 5.3 Implement recursive directory scanning
    - Use `walkdir` crate or std::fs to traverse directory tree
    - Filter for .mp4 and .mkv extensions
    - Skip directories with existing clips
    - Skip non-video files without error
    - Handle permission errors gracefully (log warning, continue)
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6_

  - [x] 5.4 Write property test for recursive directory traversal

    - **Property 1: Recursive Directory Traversal**
    - **Validates: Requirements 1.1**

  - [ ]* 5.5 Write property test for video file discovery
    - **Property 2: Video File Discovery**
    - **Validates: Requirements 1.2, 1.3**

  - [ ]* 5.6 Write property test for non-video file filtering
    - **Property 4: Non-Video File Filtering**
    - **Validates: Requirements 1.5**

- [ ] 6. Implement video processor
  - [ ] 6.1 Create VideoProcessor struct
    - Add fields for selector (Box<dyn ClipSelector>) and FFmpegExecutor
    - Define `ProcessResult` struct
    - Define `ProcessError` enum
    - _Requirements: All processing requirements_

  - [ ] 6.2 Implement output directory creation
    - Create backdrops subdirectory in video's parent directory
    - Return full output path (backdrops/backdrop.mp4)
    - Handle directory creation errors
    - _Requirements: 5.1, 5.2, 5.4, 5.5_

  - [ ]* 6.3 Write property test for backdrops directory creation
    - **Property 13: Backdrops Directory Creation**
    - **Validates: Requirements 5.2**

  - [ ]* 6.4 Write property test for output path structure
    - **Property 12: Output Path Structure**
    - **Validates: Requirements 5.1, 5.4, 5.5**

  - [ ] 6.5 Implement process_video method
    - Get video duration using FFmpegExecutor
    - Select segment using ClipSelector strategy
    - Create output directory
    - Extract clip using FFmpegExecutor
    - Return ProcessResult with success/failure status
    - Handle errors gracefully (log and continue)
    - _Requirements: 2.1, 2.2, 2.3, 5.1, 5.2, 5.3, 7.2, 7.3_

  - [ ]* 6.6 Write property test for output file naming
    - **Property 6: Output File Naming**
    - **Validates: Requirements 2.3, 5.5**

  - [ ]* 6.7 Write property test for overwrite existing clips
    - **Property 14: Overwrite Existing Clips**
    - **Validates: Requirements 5.3**

  - [ ]* 6.8 Write property test for error recovery continuation
    - **Property 15: Error Recovery Continuation**
    - **Validates: Requirements 7.2, 7.3**

  - [ ]* 6.9 Write property test for error messages include paths
    - **Property 16: Error Messages Include Paths**
    - **Validates: Requirements 7.5**

- [ ] 7. Implement progress reporter
  - [ ] 7.1 Create ProgressReporter struct
    - Add fields for total, current, successful, failed counts
    - Implement start, update, and finish methods
    - _Requirements: 8.1, 8.2, 8.3, 8.4_

  - [ ] 7.2 Implement progress output formatting
    - Display total videos found at start
    - Display current/total progress for each video
    - Display output path on success
    - Display error message on failure
    - Display summary at completion
    - _Requirements: 8.1, 8.2, 8.3, 8.4_

  - [ ]* 7.3 Write property test for progress updates per video
    - **Property 17: Progress Updates Per Video**
    - **Validates: Requirements 8.2**

  - [ ]* 7.4 Write property test for success messages include output path
    - **Property 18: Success Messages Include Output Path**
    - **Validates: Requirements 8.3**

  - [ ]* 7.5 Write unit test for initial progress message
    - Test that total count is displayed at start
    - _Requirements: 8.1_

  - [ ]* 7.6 Write unit test for summary message
    - Test that successful and failed counts are displayed at end
    - _Requirements: 8.4_

- [ ] 8. Checkpoint - Ensure processing pipeline works end-to-end
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 9. Implement CLI interface
  - [ ] 9.1 Define CLI argument structure with clap
    - Create `CliArgs` struct with derive(Parser)
    - Add directory path as required positional argument
    - Add optional strategy flag (random or intense-audio, default random)
    - Add optional resolution flag (720p or 1080p, default 1080p)
    - Add optional audio flag (default true)
    - Add --help flag support
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7, 6.8, 6.9_

  - [ ]* 9.2 Write unit tests for CLI argument parsing
    - Test default values
    - Test various flag combinations
    - Test invalid arguments produce errors
    - Test --help flag
    - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7, 6.8, 6.9_

  - [ ] 9.3 Implement main application orchestration
    - Parse CLI arguments
    - Validate directory exists (exit with error code 1 if not)
    - Check FFmpeg availability (exit with error if not found)
    - Create VideoScanner and scan for videos
    - Create appropriate ClipSelector based on strategy flag
    - Create FFmpegExecutor with resolution and audio settings
    - Create VideoProcessor with selector and executor
    - Create ProgressReporter
    - Process each video with progress updates
    - Display final summary
    - _Requirements: 7.1, All_

  - [ ]* 9.4 Write unit test for directory not found error
    - Test error handling when directory doesn't exist
    - _Requirements: 7.1_

- [ ] 10. Integration testing and final validation
  - [ ]* 10.1 Write integration test with sample videos
    - Create test directory structure with sample videos
    - Run full pipeline end-to-end
    - Verify output files created in correct locations
    - Verify clips have correct duration and properties
    - Verify progress output is displayed
    - _Requirements: All_

  - [ ]* 10.2 Write integration test for error recovery
    - Include one corrupted video in test set
    - Verify processing continues for other videos
    - Verify error is logged with file path
    - _Requirements: 7.2, 7.3, 7.5_

- [ ] 11. Final checkpoint - Complete validation
  - Ensure all tests pass, ask the user if questions arise.
  - Verify all requirements are covered by implementation and tests
  - Run full test suite with increased property test iterations (1000+)

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation at key milestones
- Property tests validate universal correctness properties across randomized inputs
- Unit tests validate specific examples, edge cases, and error conditions
- Integration tests validate the complete pipeline end-to-end
- The implementation uses Rust 2024 edition as specified in requirements
