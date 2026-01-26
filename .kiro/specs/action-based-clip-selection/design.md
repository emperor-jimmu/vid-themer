# Design Document: Action-Based Clip Selection

## Overview

This design adds a new clip selection strategy to the Video Clip Extractor that identifies and extracts clips from high-action video segments. The strategy uses FFmpeg's built-in scene detection filter (`select` with `scene` variable) to analyze motion intensity and select the most visually dynamic segments for extraction.

The action-based selector integrates seamlessly with the existing architecture by implementing the `ClipSelector` trait and following the same patterns as `RandomSelector` and `IntenseAudioSelector`. It respects all existing constraints including exclusion zones, clip duration limits, and fallback behavior.

## Architecture

### Component Integration

The action-based selector fits into the existing pipeline architecture:

```
CLI (cli.rs)
  ↓ parses --strategy action
VideoProcessor (processor.rs)
  ↓ creates ActionSelector
ActionSelector (selector.rs)
  ↓ calls FFmpegExecutor
FFmpegExecutor (ffmpeg.rs)
  ↓ executes scene detection
ActionSelector
  ↓ returns TimeRange
VideoProcessor
  ↓ extracts clip
```

### Strategy Selection

The CLI enum `SelectionStrategy` will be extended with a new variant:

```rust
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum SelectionStrategy {
    Random,
    IntenseAudio,
    #[value(name = "action")]
    Action,
}
```

Both `--strategy action` and `--strategy intense-action` will map to the same `Action` variant (using clap's value attribute).

## Components and Interfaces

### ActionSelector Structure

```rust
pub struct ActionSelector {
    ffmpeg_executor: crate::ffmpeg::FFmpegExecutor,
}

impl ActionSelector {
    pub fn new(ffmpeg_executor: crate::ffmpeg::FFmpegExecutor) -> Self {
        Self { ffmpeg_executor }
    }

    /// Calculate middle segment as fallback when motion analysis fails
    fn middle_segment(duration: f64) -> Result<TimeRange, SelectionError> {
        // Reuse the same logic as IntenseAudioSelector::middle_segment
        // This ensures consistent fallback behavior across strategies
    }
}
```

### ClipSelector Trait Implementation

```rust
impl ClipSelector for ActionSelector {
    fn select_segment(
        &self,
        video_path: &Path,
        duration: f64,
        intro_exclusion_percent: f64,
        outro_exclusion_percent: f64,
    ) -> Result<TimeRange, SelectionError> {
        // 1. Analyze motion using FFmpeg scene detection
        // 2. Filter segments by exclusion zones
        // 3. Select highest-scoring segment
        // 4. Fall back to middle segment on failure
    }
}
```

### FFmpegExecutor Extension

Add a new method to `FFmpegExecutor` for motion analysis:

```rust
pub struct MotionSegment {
    pub start_time: f64,
    pub duration: f64,
    pub motion_score: f64,
}

impl FFmpegExecutor {
    /// Analyze motion intensity across the video duration
    /// Returns a sorted list of motion segments by score (highest first)
    pub fn analyze_motion_intensity(
        &self,
        video_path: &Path,
        duration: f64,
    ) -> Result<Vec<MotionSegment>, FFmpegError> {
        // Implementation details below
    }
}
```

## Data Models

### MotionSegment

Represents a video segment with its motion score:

```rust
pub struct MotionSegment {
    pub start_time: f64,      // Start time in seconds
    pub duration: f64,         // Duration in seconds
    pub motion_score: f64,     // Motion intensity score (higher = more action)
}
```

This mirrors the `AudioSegment` structure used by `IntenseAudioSelector`.

### Motion Analysis Algorithm

The motion analysis uses FFmpeg's scene detection filter with the following approach:

1. **Scene Detection Filter**: Use `select=gt(scene,THRESHOLD)` with `showinfo` to detect scene changes
2. **Threshold Selection**: Use a threshold of 0.3 (moderate sensitivity) to capture significant motion without noise
3. **Frame-Level Scores**: Extract scene change scores and timestamps from FFmpeg's showinfo output
4. **Segment Aggregation**: Group frame-level scores into 12.5-second segments (matching audio analysis)
5. **Score Calculation**: Calculate segment motion score as the sum of scene change scores within that segment
6. **Sorting**: Sort segments by motion score (highest first)

### FFmpeg Command Structure

The motion analysis command will be:

```bash
ffmpeg -i <video> -t <analysis_duration> \
  -vf "select=gt(scene\,0.3),showinfo" \
  -f null -
```

This command:

- Analyzes up to 5 minutes of video (for performance)
- Uses the `select` filter to identify frames with scene changes above threshold 0.3
- Uses `showinfo` to output frame information including timestamps and scene scores
- Outputs to null (no file created, only analysis)

### Output Parsing

FFmpeg's `showinfo` filter outputs lines like:

```
[Parsed_showinfo_1 @ 0x...] n:42 pts:1260 pts_time:1.26 ... scene:0.456 ...
```

The parser will:

1. Extract `pts_time` (timestamp in seconds)
2. Extract `scene` score (motion intensity value)
3. Build a list of (timestamp, score) pairs
4. Aggregate into segments

### Segment Grouping

Following the same pattern as audio analysis:

```rust
const SEGMENT_DURATION: f64 = 12.5; // seconds

// For each segment window:
// - Collect all scene change scores within the window
// - Sum the scores to get total motion for that segment
// - Higher sum = more scene changes = more action
```

### Exclusion Zone Filtering

After identifying motion segments, filter them to respect exclusion zones:

```rust
fn filter_by_exclusion_zones(
    segments: Vec<MotionSegment>,
    duration: f64,
    intro_exclusion_percent: f64,
    outro_exclusion_percent: f64,
) -> Vec<MotionSegment> {
    let intro_boundary = duration * (intro_exclusion_percent / 100.0);
    let outro_boundary = duration - (duration * (outro_exclusion_percent / 100.0));

    segments.into_iter()
        .filter(|seg| {
            let segment_start = seg.start_time;
            let segment_end = seg.start_time + seg.duration;

            // Entire segment must be between boundaries
            segment_start >= intro_boundary && segment_end <= outro_boundary
        })
        .collect()
}
```

### Segment Selection Logic

```rust
// After filtering by exclusion zones:
if let Some(best_segment) = filtered_segments.first() {
    // Use the highest-scoring segment
    // Adjust duration to fit within 12-18 second range
    let clip_duration = best_segment.duration
        .max(config.min_duration)
        .min(config.max_duration)
        .min(duration);

    return Ok(TimeRange {
        start_seconds: best_segment.start_time,
        duration_seconds: clip_duration,
    });
} else {
    // No valid segments found, fall back to middle segment
    Self::middle_segment(duration)
}
```

## Error Handling

### Error Types

Reuse existing `SelectionError` enum, potentially adding a new variant:

```rust
#[derive(Debug, thiserror::Error)]
pub enum SelectionError {
    #[error("Video too short: {0}s")]
    VideoTooShort(f64),

    #[error("Failed to analyze audio: {0}")]
    AudioAnalysisFailed(String),

    #[error("Failed to analyze motion: {0}")]
    MotionAnalysisFailed(String),
}
```

### Fallback Strategy

The action selector follows the same fallback pattern as the audio selector:

1. **Primary**: Analyze motion and select highest-scoring segment
2. **Fallback 1**: If motion analysis fails, use middle segment
3. **Fallback 2**: If video is too short, use full video duration

### Error Scenarios

| Scenario                        | Handling                                      |
| ------------------------------- | --------------------------------------------- |
| FFmpeg not available            | Return `SelectionError::MotionAnalysisFailed` |
| Video file corrupted            | Return `SelectionError::MotionAnalysisFailed` |
| No motion detected              | Fall back to middle segment (not an error)    |
| All segments in exclusion zones | Fall back to middle segment                   |
| Parsing failure                 | Return `SelectionError::MotionAnalysisFailed` |

## Testing Strategy

### Unit Tests

Unit tests will verify specific behaviors and edge cases:

1. **Middle segment fallback**: Verify fallback calculation for various durations
2. **Exclusion zone filtering**: Test that segments violating boundaries are excluded
3. **Duration adjustment**: Test clip duration capping and extension
4. **Error handling**: Test error scenarios (missing FFmpeg, corrupted files)
5. **Segment selection**: Test selection of highest-scoring segment
6. **Tie-breaking**: Test that first occurrence is selected when scores are equal

### Property-Based Tests

Property tests will verify universal correctness properties across randomized inputs (minimum 100 iterations per test):

_Properties will be defined after prework analysis_

### Integration Tests

Integration tests will validate end-to-end behavior with real video files:

1. **Action strategy extraction**: Create test video with varying motion, verify clip extraction
2. **Exclusion zone compliance**: Verify extracted clips respect configured boundaries
3. **Fallback behavior**: Test with videos that have no motion or analysis failures
4. **CLI integration**: Test `--strategy action` flag end-to-end

### Test Organization

Following the project's testing conventions:

- **Unit tests**: Co-located in `#[cfg(test)]` module in `selector.rs`
- **Property tests**: Using `proptest`, included in unit test module
- **Integration tests**: In `tests/` directory if needed for complex scenarios
- **Test tags**: All property tests tagged with feature name and property number

Example property test structure:

```rust
// Feature: action-based-clip-selection, Property 1: Exclusion Zone Compliance
#[proptest]
fn test_action_selection_respects_exclusion_zones(
    duration in 415.0..3600.0f64,
    intro_percent in 0.0..=10.0f64,
    outro_percent in 0.0..=50.0f64,
) {
    // Test implementation
}
```

## Correctness Properties

_A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees._

The following properties define the correctness criteria for the action-based clip selection strategy. Each property is universally quantified and references the requirements it validates.

### Property 1: Highest Motion Score Selection

_For any_ video with multiple motion segments identified, the Action_Selector should select the segment with the highest motion score (excluding segments that violate exclusion zones).

**Validates: Requirements 1.3**

### Property 2: Tie-Breaking by First Occurrence

_For any_ set of motion segments where multiple segments have identical or very similar motion scores (within 0.01 difference), the Action_Selector should select the segment that appears first in the original analysis order.

**Validates: Requirements 1.4**

### Property 3: Analysis Duration Limit

_For any_ video with duration greater than 300 seconds (5 minutes), the FFmpeg motion analysis command should include a duration limit parameter (-t 300) to restrict analysis to the first 5 minutes.

**Validates: Requirements 2.2**

### Property 4: Exclusion Zone Compliance

_For any_ video, exclusion zone configuration, and selected clip, the entire clip (from start_seconds to start_seconds + duration_seconds) must fall between the intro exclusion boundary and the outro exclusion boundary. Specifically:

- start_seconds >= (video_duration \* intro_exclusion_percent / 100.0)
- (start_seconds + duration_seconds) <= (video_duration - video_duration \* outro_exclusion_percent / 100.0)

Note: Tests should use the constants defined in selector.rs: `INTRO_EXCLUSION_PERCENT` (1.0) and `OUTRO_EXCLUSION_PERCENT` (40.0) for default values.

**Validates: Requirements 3.1, 3.2, 3.3**

### Property 5: Next Best Segment Selection

_For any_ video where the highest-scoring motion segment violates exclusion zones, the Action_Selector should select the next highest-scoring segment that fully respects exclusion zone boundaries (or fall back to middle segment if none exist).

**Validates: Requirements 3.4**

### Property 6: Clip Duration Constraints

_For any_ video processed by the Action_Selector, the returned TimeRange duration should be:

- At least MIN_CLIP_DURATION (12.0 seconds) or the full video duration if video is shorter
- At most MAX_CLIP_DURATION (18.0 seconds) or the full video duration if video is shorter
- Never exceeding the video duration

This property combines the requirements for minimum duration, maximum duration, and boundary compliance.

Note: Tests should use the constants defined in selector.rs: `MIN_CLIP_DURATION` (12.0) and `MAX_CLIP_DURATION` (18.0).

**Validates: Requirements 4.1, 4.2, 4.3, 4.4**

### Property 7: Clip Within Video Boundaries

_For any_ video and selected TimeRange, the end time of the clip (start_seconds + duration_seconds) must not exceed the video duration. This ensures clips never attempt to extract beyond the video's end.

**Validates: Requirements 4.4**

### Property 8: Valid TimeRange Return

_For any_ valid video path, duration, and exclusion zone configuration, the Action_Selector's select_segment method should return a TimeRange where:

- start_seconds >= 0.0
- duration_seconds > 0.0
- start_seconds + duration_seconds <= video_duration

**Validates: Requirements 6.2**

### Property 9: Middle Segment Consistency

_For any_ video duration, when the Action_Selector falls back to middle segment selection, the calculated TimeRange should match the result of IntenseAudioSelector::middle_segment for the same duration. This ensures consistent fallback behavior across strategies.

**Validates: Requirements 7.5**

### Property 10: Timestamp Scaling Correctness

_For any_ video where motion analysis is limited to a shorter duration than the full video, segment timestamps should be scaled proportionally to the full video duration. If analysis covers duration A of a video with total duration V, then a segment at time T in the analysis should map to time (T \* V / A) in the full video.

**Validates: Requirements 8.2**

### Property 11: Stateless Processing

_For any_ sequence of videos processed by the same Action_Selector instance, the selection for each video should be independent of previous videos. Processing video A then video B should produce the same result for video B as processing video B alone. This verifies no state is cached between videos.

**Validates: Requirements 8.4**
