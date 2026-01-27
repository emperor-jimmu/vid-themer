# Implementation Plan: Multiple Clips Per Video

## Overview

This implementation plan breaks down the multiple-clips-per-video feature into discrete coding tasks. The approach follows an incremental strategy: first updating core data structures and interfaces, then modifying each selector implementation, updating the processor, and finally adding comprehensive tests. Each task builds on previous work to ensure no orphaned code.

## Tasks

- [x] 1. Update CLI interface to accept clip count parameter
  - Add `clip_count: u8` field to `CliArgs` struct in `src/cli.rs`
  - Add clap attribute with short form `-c`, long form `--clip-count`, default value `1`
  - Implement `validate_clip_count` function to ensure value is between 1-4
  - Add value_parser attribute to use validation function
  - _Requirements: 1.1, 1.2, 1.3, 1.4_

- [x] 1.1 Write unit tests for CLI validation

  - Test valid inputs (1, 2, 3, 4)
  - Test invalid inputs (0, 5, negative values)
  - Test default value behavior
  - _Requirements: 1.2, 1.3, 1.4_

- [ ]* 1.2 Write property test for CLI input validation
  - **Property 1: CLI Input Validation**
  - **Validates: Requirements 1.2, 1.4**

- [x] 2. Extend TimeRange with overlap detection and validation
  - Add `overlaps(&self, other: &TimeRange) -> bool` method to `TimeRange` in `src/selector.rs`
  - Add `duration(&self) -> f64` method to calculate range duration
  - Add `is_valid_duration(&self) -> bool` method to check MIN/MAX bounds
  - Add constants `MIN_CLIP_DURATION` and `MAX_CLIP_DURATION` if not already present
  - _Requirements: 3.1, 5.1_

- [x] 2.1 Write unit tests for TimeRange methods

  - Test overlapping ranges (various overlap scenarios)
  - Test non-overlapping ranges
  - Test adjacent ranges (touching but not overlapping)
  - Test duration calculation
  - Test valid/invalid duration checks
  - _Requirements: 3.1, 5.1_

- [x] 2.2 Write property test for non-overlapping detection

  - **Property 4: Non-Overlapping Segments**
  - **Validates: Requirements 3.1**

- [x] 3. Update ClipSelector trait interface
  - Change `select_clip` method signature to `select_clips` in `src/selector.rs`
  - Update return type from `Result<TimeRange>` to `Result<Vec<TimeRange>>`
  - Add `clip_count: u8` parameter to method signature
  - Update trait documentation to describe multi-clip behavior
  - _Requirements: 2.1, 3.1_

- [x] 4. Implement RandomSelector for multiple clips
  - [x] 4.1 Update RandomSelector to implement new trait signature
    - Modify `select_clips` method to accept `clip_count` parameter
    - Calculate valid selection zone (intro/outro exclusion)
    - Check if video can accommodate requested clips
    - Implement loop to generate N random non-overlapping clips
    - Add attempt limit (MAX_ATTEMPTS = 1000) to prevent infinite loops
    - Sort clips by start time before returning
    - _Requirements: 2.1, 3.1, 4.1, 5.1, 7.1, 7.4_

  - [x] 4.2 Write unit tests for RandomSelector

    - Test single clip generation (backward compatibility)
    - Test multiple clip generation (2, 3, 4 clips)
    - Test graceful degradation for short videos
    - Test exclusion zone compliance
    - _Requirements: 2.1, 2.3, 3.1, 4.1_

  - [x] 4.3 Write property test for exact clip count generation

    - **Property 2: Exact Clip Count Generation**
    - **Validates: Requirements 2.1**

  - [x] 4.4 Write property test for exclusion zone compliance

    - **Property 5: Exclusion Zone Compliance**
    - **Validates: Requirements 4.1**

  - [ ]* 4.5 Write property test for duration constraints
    - **Property 6: Duration Constraint Compliance**
    - **Validates: Requirements 5.1**

- [x] 5. Implement IntenseAudioSelector for multiple clips
  - [x] 5.1 Update IntenseAudioSelector to implement new trait signature
    - Modify `select_clips` method to accept `clip_count` parameter
    - Calculate valid selection zone
    - Analyze audio intensity across video (reuse existing logic)
    - Find intensity peaks within valid zone
    - Sort peaks by intensity (descending)
    - Select top N non-overlapping peaks
    - Create TimeRange for each peak, checking for overlaps
    - Sort selected clips by start time before returning
    - _Requirements: 2.1, 3.1, 4.1, 5.1, 7.2, 7.4_

  - [x] 5.2 Write unit tests for IntenseAudioSelector

    - Test single clip selection
    - Test multiple clip selection with mock audio data
    - Test peak selection with overlapping candidates
    - Test graceful degradation
    - _Requirements: 2.1, 2.3, 3.1, 7.2_

- [x] 6. Update VideoProcessor for multiple clips
  - [x] 6.1 Modify VideoProcessor to handle multiple clips
    - Add `clip_count` field to `VideoProcessor` struct in `src/processor.rs`
    - Update `process_video` method to call `select_clips` with clip_count
    - Handle empty vector result (NoValidClips error)
    - Add warning logging when fewer clips generated than requested
    - Implement loop to extract each clip with sequential naming
    - Update filename generation to use "vid1.mp4", "vid2.mp4", etc.
    - Update `ProcessResult` enum to include `clips_generated: usize` field
    - _Requirements: 2.1, 2.3, 6.1, 6.2, 9.2_

  - [x] 6.2 Write unit tests for processor clip naming

    - Test sequential naming for 1-4 clips
    - Test output directory creation
    - Test file path construction
    - _Requirements: 6.1, 6.2_

  - [x] 6.3 Write property test for sequential naming convention

    - **Property 7: Sequential Naming Convention**
    - **Validates: Requirements 6.1**

  - [ ]* 6.4 Write property test for output directory consistency
    - **Property 8: Output Directory Consistency**
    - **Validates: Requirements 6.2**

- [ ] 7. Update main.rs to pass clip_count through pipeline
  - Extract `clip_count` from `CliArgs` in `src/main.rs`
  - Pass `clip_count` to `VideoProcessor` constructor
  - Update any logging or progress reporting to reflect multiple clips
  - _Requirements: 2.1_

- [ ] 8. Checkpoint - Ensure all tests pass
  - Run `cargo test` to verify all unit and property tests pass
  - Run `cargo clippy` to check for warnings
  - Run `cargo fmt` to ensure code formatting
  - Ask the user if questions arise

- [ ]* 9. Add property tests for graceful degradation and error handling
  - [ ]* 9.1 Write property test for graceful degradation
    - **Property 3: Graceful Degradation for Short Videos**
    - **Validates: Requirements 2.3, 3.3**

  - [ ]* 9.2 Write property test for chronological ordering
    - **Property 9: Chronological Ordering**
    - **Validates: Requirements 7.4**

  - [ ]* 9.3 Write property test for warning logging
    - **Property 10: Warning Logging for Reduced Clip Count**
    - **Validates: Requirements 9.2**

  - [ ]* 9.4 Write property test for no-crash guarantee
    - **Property 11: No-Crash Guarantee for Constrained Videos**
    - **Validates: Requirements 9.3**

- [ ]* 10. Add integration tests for end-to-end behavior
  - [ ]* 10.1 Write integration test for full pipeline with multiple clips
    - Create test video with sufficient duration in `tests/multiple_clips_pipeline.rs`
    - Run extractor with clip_count=3
    - Verify 3 clips generated with correct names (vid1.mp4, vid2.mp4, vid3.mp4)
    - Verify clips are non-overlapping and within constraints
    - _Requirements: 2.1, 3.1, 4.1, 5.1, 6.1, 6.2_

  - [ ]* 10.2 Write integration test for backward compatibility
    - Run with clip_count=1
    - Verify single clip named "vid1.mp4"
    - Compare behavior with expected single-clip behavior
    - _Requirements: 8.1, 8.2_

  - [ ]* 10.3 Write integration test for strategy-specific behavior
    - Test random strategy with clip_count=2
    - Test intense-audio strategy with clip_count=2
    - Verify strategy-specific selection maintained
    - _Requirements: 7.1, 7.2_

- [ ] 11. Update error handling and logging
  - Add `NoValidClips` variant to `ProcessError` enum in `src/error.rs`
  - Update error messages to include clip count context
  - Ensure warning messages include video filename and actual clip count
  - Update failure logger to capture multi-clip failures
  - _Requirements: 9.2, 9.3_

- [ ] 12. Final checkpoint - Comprehensive testing and validation
  - Run full test suite: `cargo test`
  - Run property tests with verbose output: `cargo test -- --nocapture`
  - Test with real video files manually (clip_count=1, 2, 3, 4)
  - Verify backward compatibility (clip_count=1 behaves as expected)
  - Verify graceful degradation with short videos
  - Ask the user if questions arise

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Property tests validate universal correctness properties with minimum 100 iterations
- Unit tests validate specific examples and edge cases
- Integration tests verify end-to-end behavior with real video files
- The implementation maintains backward compatibility (clip_count=1 behaves like current system)
- All clips must be non-overlapping and respect exclusion zones and duration constraints
