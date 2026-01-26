# Implementation Plan: FFmpeg Module Code Quality Improvements

## Overview

This plan implements code quality improvements to `src/ffmpeg.rs` in four incremental phases: Safety (high priority), Robustness (high priority), Maintainability (medium priority), and Polish (low priority). Each phase can be implemented and tested independently while maintaining backward compatibility.

## Tasks

- [x] 1. Phase 1: Safety Improvements - Eliminate Unsafe Floating-Point Comparisons
  - [x] 1.1 Replace unsafe partial_cmp().unwrap() with total_cmp() in audio segment sorting
    - Locate the segment sorting in `analyze_audio_intensity` method (around line 815)
    - Replace `b.intensity.partial_cmp(&a.intensity).unwrap()` with `b.intensity.total_cmp(&a.intensity)`
    - _Requirements: 1.1, 1.4_
  
  - [x] 1.2 Replace unsafe partial_cmp().unwrap() with total_cmp() in motion segment sorting
    - Locate the segment sorting in `analyze_motion_intensity` method (around line 1009)
    - Replace `b.motion_score.partial_cmp(&a.motion_score).unwrap()` with `b.motion_score.total_cmp(&a.motion_score)`
    - _Requirements: 1.2, 1.4_
  
  - [x] 1.3 Write property test for NaN-safe segment sorting
    - **Property 1: NaN-Safe Segment Sorting**
    - **Validates: Requirements 1.1, 1.2**
    - Generate random AudioSegment and MotionSegment collections with NaN values
    - Verify sorting completes without panic
    - Verify NaN values are consistently placed
  
  - [x] 1.4 Write unit tests for NaN handling edge cases
    - Test sorting with single NaN value
    - Test sorting with all NaN values
    - Test sorting with NaN and infinity values
    - _Requirements: 1.1, 1.2_

- [x] 2. Phase 2: Robustness - Replace Manual JSON Parsing with serde_json
  - [x] 2.1 Add serde_json dependency to Cargo.toml
    - Add `serde = { version = "1.0", features = ["derive"] }`
    - Add `serde_json = "1.0"`
    - Run `cargo check` to verify dependencies resolve
    - _Requirements: 2.1_
  
  - [x] 2.2 Define structured types for JSON deserialization
    - Create `FFprobeOutput`, `FFprobeStream`, and `FFprobeFormat` structs
    - Add `#[derive(Debug, Deserialize)]` to each struct
    - Add appropriate field names matching ffprobe JSON output
    - _Requirements: 2.1, 2.3_
  
  - [x] 2.3 Replace parse_metadata_json implementation with serde_json
    - Rewrite `parse_metadata_json` to use `serde_json::from_str`
    - Add proper error handling with field-specific context
    - Maintain existing validation for "N/A" duration
    - _Requirements: 2.1, 2.2_
  
  - [x] 2.4 Remove obsolete manual JSON parsing helper methods
    - Delete `extract_json_value` method
    - Delete `extract_json_string_value` method
    - Verify no other code references these methods
    - _Requirements: 2.4_
  
  - [x] 2.5 Write unit tests for JSON parsing with serde_json
    - Test valid JSON parsing
    - Test missing codec_name field
    - Test invalid width (non-numeric)
    - Test "N/A" duration handling
    - Test empty duration handling
    - Verify error messages include field names
    - _Requirements: 2.2, 5.3_

- [ ] 3. Checkpoint - Verify high-priority improvements
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 4. Phase 3: Maintainability - Extract Magic Numbers and Eliminate Duplication
  - [ ] 4.1 Create constants module with named constants
    - Add `mod constants` at top of file
    - Define constants for: HW_ACCEL_BITRATE, SOFTWARE_CRF, KEYFRAME_INTERVAL
    - Define constants for: H264_FAST_SEEK_OFFSET, HEVC_FAST_SEEK_OFFSET
    - Define constants for: MAX_ANALYSIS_DURATION, SEGMENT_DURATION, HEVC_BUFFER_SIZE
    - Add documentation comments explaining each constant's purpose
    - _Requirements: 3.1, 3.2, 3.4_
  
  - [ ] 4.2 Replace magic numbers with named constants throughout the file
    - Replace hardcoded "5M" with `constants::HW_ACCEL_BITRATE`
    - Replace hardcoded "26" with `constants::SOFTWARE_CRF`
    - Replace hardcoded "30" with `constants::KEYFRAME_INTERVAL`
    - Replace hardcoded 5.0 and 2.0 with seek offset constants
    - Replace hardcoded 300.0 with `constants::MAX_ANALYSIS_DURATION`
    - Replace hardcoded 12.5 with `constants::SEGMENT_DURATION`
    - Replace hardcoded "100M" with `constants::HEVC_BUFFER_SIZE`
    - _Requirements: 3.4_
  
  - [ ] 4.3 Implement shared segment grouping helper function
    - Create `group_measurements_into_segments` function with generic aggregation
    - Accept measurements, video_duration, analysis_duration, segment_duration, and aggregate_fn
    - Return vector of (start_time, duration, score) tuples
    - Add comprehensive documentation
    - _Requirements: 4.1, 4.2, 4.3_
  
  - [ ] 4.4 Refactor analyze_audio_intensity to use shared helper
    - Replace duplicated segment grouping code with call to helper
    - Use average aggregation function for audio intensity
    - Map results to AudioSegment structs
    - _Requirements: 4.4_
  
  - [ ] 4.5 Refactor analyze_motion_intensity to use shared helper
    - Replace duplicated segment grouping code with call to helper
    - Use sum aggregation function for motion scores
    - Map results to MotionSegment structs
    - _Requirements: 4.5_
  
  - [ ] 4.6 Write property test for segment grouping correctness
    - **Property 3: Segment Grouping Correctness**
    - **Validates: Requirements 4.3**
    - Generate random measurements, video durations, and segment durations
    - Verify all measurements are assigned to exactly one segment
    - Verify segment boundaries are correct
    - Verify no measurements are lost or duplicated
    - Verify aggregation is applied correctly
  
  - [ ] 4.7 Write unit tests for segment grouping edge cases
    - Test with empty measurements
    - Test with single measurement
    - Test with measurements at segment boundaries
    - Test with video_duration < segment_duration
    - _Requirements: 4.3_

- [ ] 5. Phase 4: Polish - Enhanced Error Context and Documentation
  - [ ] 5.1 Enhance error messages in get_video_metadata
    - Add video file path to ffprobe execution errors
    - Add video file path to corrupted file errors
    - Add video file path to parse errors
    - _Requirements: 5.1_
  
  - [ ] 5.2 Enhance error messages in extract_clip
    - Add video file path and time range to extraction errors
    - Format time range as "start-end" for clarity
    - _Requirements: 5.2_
  
  - [ ] 5.3 Update stderr() method for enhanced error messages
    - Rewrite to handle new error message formats with file paths
    - Use chain of strip_prefix attempts with split_once
    - Add comprehensive documentation explaining behavior
    - Maintain fallback to full message if no prefix matches
    - _Requirements: 7.1, 7.2, 7.3_
  
  - [ ] 5.4 Write unit tests for enhanced error messages
    - Test ffprobe failure includes file path
    - Test extraction failure includes file path and time range
    - Test JSON parse failure includes field context
    - _Requirements: 5.1, 5.2, 5.3_
  
  - [ ] 5.5 Write property test for stderr extraction consistency
    - **Property 5: Stderr Extraction Consistency**
    - **Validates: Requirements 7.3**
    - Generate ExecutionFailed errors with various prefixes
    - Verify stderr() correctly extracts stderr content
    - Verify fallback behavior for unknown prefixes
  
  - [ ] 5.6 Write unit test for stderr() returning None for non-execution errors
    - **Property 6: Non-Execution Errors Return None for Stderr**
    - **Validates: Requirements 7.4**
    - Create NotFound, ParseError, NoAudioTrack, CorruptedFile errors
    - Verify stderr() returns None for each
  
  - [ ] 5.7 Fix unstable Rust syntax in property tests
    - Locate `let...if` chains in test code (around line 1700 in analyze_audio_intensity_fallback)
    - Replace with nested if-let or tuple pattern matching
    - Verify tests still pass
    - _Requirements: 6.1, 6.2, 6.3_
  
  - [ ] 5.8 Add documentation to platform-specific hardware acceleration code
    - Add comment block explaining VideoToolbox vs NVENC choice
    - Document hardware requirements for each platform
    - Document fallback behavior when hardware unavailable
    - Add inline comments for each #[cfg] directive
    - _Requirements: 8.1, 8.2, 8.3, 8.4_
  
  - [ ] 5.9 Verify stable Rust compilation
    - **Property 7: Stable Rust Compilation**
    - **Validates: Requirements 6.4**
    - Run `cargo build` with stable Rust toolchain
    - Verify no warnings about unstable features
    - Verify successful compilation

- [ ] 6. Final checkpoint - Verify all improvements complete
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- All tasks are required for comprehensive code quality improvements
- Each phase can be implemented independently and merged separately
- All changes maintain backward compatibility with existing public API
- Existing tests should continue to pass throughout implementation
- Property tests should run with minimum 100 iterations
- Each property test must include a comment linking to the design document property
