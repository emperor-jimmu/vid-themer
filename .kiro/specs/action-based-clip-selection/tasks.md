# Implementation Plan: Action-Based Clip Selection

## Overview

This implementation plan adds a new action-based clip selection strategy to the Video Clip Extractor. The strategy uses FFmpeg's scene detection to identify high-motion video segments and extract clips from the most visually dynamic portions. The implementation follows the existing architecture patterns and integrates seamlessly with the current CLI, selector trait, and FFmpeg executor.

## Tasks

- [x] 1. Extend CLI to support action strategy
  - Add `Action` variant to `SelectionStrategy` enum in cli.rs
  - Configure clap to accept "action" and "intense-action" as valid values
  - Update CLI help text to document the new strategy
  - _Requirements: 5.1, 5.2, 5.5_

- [x] 1.1 Write unit tests for CLI action strategy parsing
  - Test `--strategy action` flag parsing
  - Test `-s action` short flag parsing
  - Test `--strategy intense-action` alias parsing
  - Verify help text includes action strategy documentation
  - _Requirements: 5.1, 5.2, 5.5_

- [ ] 2. Add motion analysis to FFmpegExecutor
  - [ ] 2.1 Define MotionSegment struct in ffmpeg.rs
    - Add fields: start_time, duration, motion_score
    - Derive Debug, Clone traits
    - _Requirements: 2.1, 2.2_
  - [ ] 2.2 Implement analyze_motion_intensity method
    - Build FFmpeg command with scene detection filter
    - Limit analysis to 5 minutes for long videos
    - Execute FFmpeg and capture stderr output
    - Parse showinfo output to extract scene scores and timestamps
    - Group frame-level scores into 12.5-second segments
    - Calculate segment motion scores by summing scene change scores
    - Sort segments by motion score (highest first)
    - Return Vec<MotionSegment> or FFmpegError
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 10.3_
  - [ ] 2.3 Write unit tests for motion analysis parsing
    - Test parsing of FFmpeg showinfo output
    - Test extraction of pts_time and scene scores
    - Test segment grouping into 12.5-second windows
    - Test score aggregation within segments
    - Test handling of empty or malformed output
    - _Requirements: 2.3, 9.3_
  - [ ] 2.4 Write property test for analysis duration limit
    - **Property 3: Analysis Duration Limit**
    - **Validates: Requirements 2.2**
    - For videos > 300 seconds, verify FFmpeg command includes -t 300
    - _Requirements: 2.2_

- [ ] 3. Checkpoint - Verify FFmpeg integration compiles
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 4. Implement ActionSelector in selector.rs
  - [ ] 4.1 Define ActionSelector struct
    - Add ffmpeg_executor field
    - Implement new() constructor
    - Implement middle_segment() helper (reuse IntenseAudioSelector logic)
    - _Requirements: 6.1, 7.5_
  - [ ] 4.2 Implement ClipSelector trait for ActionSelector
    - Implement select_segment method
    - Call analyze_motion_intensity on FFmpeg executor
    - Filter segments by exclusion zones
    - Select highest-scoring valid segment
    - Adjust clip duration to fit 12-18 second range
    - Fall back to middle segment on failure
    - Handle all error cases appropriately
    - _Requirements: 1.1, 1.3, 1.4, 1.5, 3.1, 3.4, 4.1, 4.2, 4.3, 4.4, 6.2, 6.3, 7.1, 7.2, 7.3_
  - [ ] 4.3 Write unit tests for ActionSelector
    - Test middle segment fallback calculation
    - Test exclusion zone filtering logic
    - Test duration adjustment (extend short, cap long)
    - Test segment selection with multiple candidates
    - Test fallback when all segments violate exclusion zones
    - Test error handling for FFmpeg failures
    - _Requirements: 1.5, 3.4, 3.5, 4.2, 4.3, 7.1, 7.2, 7.3, 9.1, 9.2, 9.4_
  - [ ] 4.4 Write property test for highest motion score selection
    - **Property 1: Highest Motion Score Selection**
    - **Validates: Requirements 1.3**
    - Generate random motion segments, verify highest score is selected
    - _Requirements: 1.3_
  - [ ] 4.5 Write property test for tie-breaking
    - **Property 2: Tie-Breaking by First Occurrence**
    - **Validates: Requirements 1.4**
    - Generate segments with identical scores, verify first is selected
    - _Requirements: 1.4_
  - [ ] 4.6 Write property test for exclusion zone compliance
    - **Property 4: Exclusion Zone Compliance**
    - **Validates: Requirements 3.1, 3.2, 3.3**
    - For any video and exclusion config, verify clip falls between boundaries
    - Use INTRO_EXCLUSION_PERCENT and OUTRO_EXCLUSION_PERCENT constants
    - _Requirements: 3.1, 3.2, 3.3_
  - [ ] 4.7 Write property test for next best segment selection
    - **Property 5: Next Best Segment Selection**
    - **Validates: Requirements 3.4**
    - When top segment violates zones, verify next valid segment is selected
    - _Requirements: 3.4_
  - [ ] 4.8 Write property test for clip duration constraints
    - **Property 6: Clip Duration Constraints**
    - **Validates: Requirements 4.1, 4.2, 4.3, 4.4**
    - Verify duration is between MIN_CLIP_DURATION and MAX_CLIP_DURATION
    - Verify duration never exceeds video duration
    - Use MIN_CLIP_DURATION and MAX_CLIP_DURATION constants
    - _Requirements: 4.1, 4.2, 4.3, 4.4_
  - [ ] 4.9 Write property test for clip within video boundaries
    - **Property 7: Clip Within Video Boundaries**
    - **Validates: Requirements 4.4**
    - Verify start + duration <= video_duration for all cases
    - _Requirements: 4.4_
  - [ ] 4.10 Write property test for valid TimeRange return
    - **Property 8: Valid TimeRange Return**
    - **Validates: Requirements 6.2**
    - Verify start >= 0, duration > 0, end <= video_duration
    - _Requirements: 6.2_
  - [ ] 4.11 Write property test for middle segment consistency
    - **Property 9: Middle Segment Consistency**
    - **Validates: Requirements 7.5**
    - Verify ActionSelector fallback matches IntenseAudioSelector fallback
    - _Requirements: 7.5_
  - [ ] 4.12 Write property test for timestamp scaling
    - **Property 10: Timestamp Scaling Correctness**
    - **Validates: Requirements 8.2**
    - When analysis is limited, verify timestamps scale to full duration
    - _Requirements: 8.2_
  - [ ] 4.13 Write property test for stateless processing
    - **Property 11: Stateless Processing**
    - **Validates: Requirements 8.4**
    - Process multiple videos, verify no state is cached between them
    - _Requirements: 8.4_

- [ ] 5. Checkpoint - Verify ActionSelector implementation
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 6. Wire ActionSelector into main application
  - [ ] 6.1 Update main.rs to instantiate ActionSelector
    - Add match arm for SelectionStrategy::Action
    - Create ActionSelector with FFmpegExecutor
    - Pass to VideoProcessor
    - _Requirements: 1.1, 5.1, 5.2_
  - [ ] 6.2 Update processor.rs if needed
    - Verify VideoProcessor handles ActionSelector correctly
    - Ensure error handling propagates properly
    - _Requirements: 6.1, 6.2, 6.3_

- [ ] 7. Final checkpoint - End-to-end verification
  - Run full test suite (cargo test)
  - Verify all property tests pass with 100+ iterations
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Each task references specific requirements for traceability
- Property tests validate universal correctness properties (minimum 100 iterations)
- Unit tests validate specific examples and edge cases
- The implementation follows existing patterns from RandomSelector and IntenseAudioSelector
- FFmpeg scene detection uses the `select=gt(scene,0.3)` filter with `showinfo` for analysis
- Motion scores are calculated by summing scene change scores within 12.5-second segments
- Fallback behavior matches IntenseAudioSelector for consistency
