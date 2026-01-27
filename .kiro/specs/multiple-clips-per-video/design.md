# Design Document: Multiple Clips Per Video

## Overview

This feature extends the Video Clip Extractor to support generating 1-4 clips per video file instead of just one. The design maintains backward compatibility while introducing a new CLI parameter (`--clip-count`) and modifying the selection and processing pipeline to handle multiple non-overlapping clips.

The key architectural changes involve:
1. Modifying the `ClipSelector` trait to return multiple time ranges
2. Updating all selector implementations to generate non-overlapping segments
3. Changing the processor to iterate over multiple clips and use new naming conventions
4. Ensuring all existing constraints (exclusion zones, duration limits) apply to each clip

## Architecture

### High-Level Flow

```
User Input (--clip-count N)
    ↓
CLI Validation (1-4 range check)
    ↓
Video Scanner (unchanged)
    ↓
Clip Selector (returns Vec<TimeRange>)
    ↓
FFmpeg Executor (extracts N clips)
    ↓
Output: backdrops/vid1.mp4, vid2.mp4, ..., vidN.mp4
```

### Modified Components

**CLI Layer (`cli.rs`)**
- Add `clip_count: u8` field to `CliArgs` struct
- Add validation to ensure 1 ≤ clip_count ≤ 4
- Default value: 1

**Selector Trait (`selector.rs`)**
- Change return type from `Result<TimeRange>` to `Result<Vec<TimeRange>>`
- Add `clip_count` parameter to `select_clip` method
- All implementations must ensure non-overlapping segments

**Selector Implementations**
- `RandomSelector`: Generate N random non-overlapping segments
- `IntenseAudioSelector`: Select top N audio intensity peaks (non-overlapping)
- `ActionSelector`: Select top N motion intensity peaks (non-overlapping)

**Processor (`processor.rs`)**
- Iterate over returned time ranges
- Generate sequential filenames: vid1.mp4, vid2.mp4, etc.
- Handle cases where fewer clips are returned than requested

## Components and Interfaces

### CLI Interface

```rust
#[derive(Parser)]
pub struct CliArgs {
    // ... existing fields ...
    
    /// Number of clips to generate per video (1-4)
    #[arg(short = 'c', long = "clip-count", default_value = "1", value_parser = validate_clip_count)]
    pub clip_count: u8,
}

fn validate_clip_count(s: &str) -> Result<u8, String> {
    let count = s.parse::<u8>()
        .map_err(|_| "Clip count must be a number")?;
    
    if count < 1 || count > 4 {
        return Err("Clip count must be between 1 and 4".to_string());
    }
    
    Ok(count)
}
```

### ClipSelector Trait

```rust
pub trait ClipSelector {
    /// Select multiple non-overlapping clip segments from a video
    /// 
    /// # Arguments
    /// * `video_path` - Path to the video file
    /// * `duration` - Total video duration in seconds
    /// * `clip_count` - Number of clips to generate (1-4)
    /// 
    /// # Returns
    /// Vector of TimeRange objects representing non-overlapping clip segments,
    /// sorted by start time. May return fewer than clip_count if video is too short.
    fn select_clips(
        &self,
        video_path: &Path,
        duration: f64,
        clip_count: u8,
    ) -> Result<Vec<TimeRange>, SelectionError>;
}
```

### TimeRange Structure

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeRange {
    pub start: f64,
    pub end: f64,
}

impl TimeRange {
    /// Check if this range overlaps with another
    pub fn overlaps(&self, other: &TimeRange) -> bool {
        !(self.end <= other.start || self.start >= other.end)
    }
    
    /// Get duration of this time range
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
    
    /// Check if duration is within valid clip bounds
    pub fn is_valid_duration(&self) -> bool {
        let duration = self.duration();
        duration >= MIN_CLIP_DURATION && duration <= MAX_CLIP_DURATION
    }
}
```

### RandomSelector Implementation

```rust
impl ClipSelector for RandomSelector {
    fn select_clips(
        &self,
        _video_path: &Path,
        duration: f64,
        clip_count: u8,
    ) -> Result<Vec<TimeRange>, SelectionError> {
        // Calculate valid selection zone
        let intro_cutoff = duration * (INTRO_EXCLUSION_PERCENT / 100.0);
        let outro_cutoff = duration * (1.0 - OUTRO_EXCLUSION_PERCENT / 100.0);
        let valid_duration = outro_cutoff - intro_cutoff;
        
        // Check if we can fit the requested clips
        let min_required = (clip_count as f64) * MIN_CLIP_DURATION;
        if valid_duration < min_required {
            // Generate as many as possible
            let possible_count = (valid_duration / MIN_CLIP_DURATION).floor() as u8;
            return self.select_clips(_video_path, duration, possible_count);
        }
        
        let mut clips = Vec::new();
        let mut rng = thread_rng();
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 1000;
        
        while clips.len() < clip_count as usize && attempts < MAX_ATTEMPTS {
            attempts += 1;
            
            // Generate random clip
            let clip_duration = rng.gen_range(MIN_CLIP_DURATION..=MAX_CLIP_DURATION);
            let max_start = outro_cutoff - clip_duration;
            let start = rng.gen_range(intro_cutoff..max_start);
            let end = start + clip_duration;
            
            let candidate = TimeRange { start, end };
            
            // Check for overlaps with existing clips
            if !clips.iter().any(|existing| candidate.overlaps(existing)) {
                clips.push(candidate);
            }
        }
        
        // Sort by start time
        clips.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
        
        Ok(clips)
    }
}
```

### IntenseAudioSelector Implementation

```rust
impl ClipSelector for IntenseAudioSelector {
    fn select_clips(
        &self,
        video_path: &Path,
        duration: f64,
        clip_count: u8,
    ) -> Result<Vec<TimeRange>, SelectionError> {
        // Calculate valid selection zone
        let intro_cutoff = duration * (INTRO_EXCLUSION_PERCENT / 100.0);
        let outro_cutoff = duration * (1.0 - OUTRO_EXCLUSION_PERCENT / 100.0);
        
        // Analyze audio intensity across the video
        let intensity_data = self.analyze_audio_intensity(video_path, duration)?;
        
        // Find peaks in the valid zone
        let mut peaks = self.find_intensity_peaks(&intensity_data, intro_cutoff, outro_cutoff);
        
        // Sort peaks by intensity (descending)
        peaks.sort_by(|a, b| b.intensity.partial_cmp(&a.intensity).unwrap());
        
        // Select top N non-overlapping peaks
        let mut selected_clips = Vec::new();
        
        for peak in peaks {
            if selected_clips.len() >= clip_count as usize {
                break;
            }
            
            // Create time range around peak
            let clip_duration = MIN_CLIP_DURATION + 
                (MAX_CLIP_DURATION - MIN_CLIP_DURATION) * 0.5;
            let start = (peak.timestamp - clip_duration / 2.0)
                .max(intro_cutoff);
            let end = (start + clip_duration)
                .min(outro_cutoff);
            
            let candidate = TimeRange { start, end };
            
            // Check for overlaps
            if !selected_clips.iter().any(|existing| candidate.overlaps(existing)) 
                && candidate.is_valid_duration() {
                selected_clips.push(candidate);
            }
        }
        
        // Sort by start time
        selected_clips.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
        
        Ok(selected_clips)
    }
}
```

### Processor Changes

```rust
impl VideoProcessor {
    pub fn process_video(&self, video: &VideoFile) -> Result<ProcessResult, ProcessError> {
        // ... existing setup code ...
        
        // Select multiple clips
        let time_ranges = self.selector.select_clips(
            &video.path,
            duration,
            self.clip_count,
        )?;
        
        if time_ranges.is_empty() {
            return Err(ProcessError::NoValidClips);
        }
        
        // Warn if fewer clips than requested
        if time_ranges.len() < self.clip_count as usize {
            eprintln!(
                "Warning: Only generated {} of {} requested clips for {}",
                time_ranges.len(),
                self.clip_count,
                video.path.display()
            );
        }
        
        // Create backdrops directory
        let backdrop_dir = video.path.parent()
            .ok_or(ProcessError::InvalidPath)?
            .join("backdrops");
        fs::create_dir_all(&backdrop_dir)?;
        
        // Extract each clip
        for (index, time_range) in time_ranges.iter().enumerate() {
            let clip_num = index + 1;
            let output_path = backdrop_dir.join(format!("vid{}.mp4", clip_num));
            
            self.ffmpeg.extract_clip(
                &video.path,
                &output_path,
                time_range.start,
                time_range.end,
                self.resolution,
                self.include_audio,
            )?;
        }
        
        Ok(ProcessResult::Success {
            clips_generated: time_ranges.len(),
        })
    }
}
```

## Data Models

### TimeRange

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeRange {
    pub start: f64,  // Start time in seconds
    pub end: f64,    // End time in seconds
}
```

**Invariants:**
- `start < end` (enforced by constructor)
- `end - start >= MIN_CLIP_DURATION` (12 seconds)
- `end - start <= MAX_CLIP_DURATION` (18 seconds)

### ProcessResult

```rust
#[derive(Debug)]
pub enum ProcessResult {
    Success {
        clips_generated: usize,
    },
    Skipped,
    Failed(String),
}
```

### CliArgs Extension

```rust
pub struct CliArgs {
    // ... existing fields ...
    pub clip_count: u8,  // Range: 1-4
}
```


## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: CLI Input Validation

*For any* input value to the `--clip-count` parameter, if the value is an integer between 1 and 4 (inclusive), it should be accepted; otherwise, it should be rejected with an error message.

**Validates: Requirements 1.2, 1.4**

### Property 2: Exact Clip Count Generation

*For any* video file with sufficient duration and any valid clip count N (1-4), the system should generate exactly N clips.

**Validates: Requirements 2.1**

### Property 3: Graceful Degradation for Short Videos

*For any* video file where the valid selection zone cannot accommodate N non-overlapping clips of valid duration, the system should generate the maximum number of valid clips possible (which may be less than N) without failing.

**Validates: Requirements 2.3, 3.3**

### Property 4: Non-Overlapping Segments

*For any* set of clips generated from a single video, no two clips should have overlapping time ranges (i.e., for all pairs of clips i and j where i ≠ j, either clip_i.end ≤ clip_j.start or clip_j.end ≤ clip_i.start).

**Validates: Requirements 3.1**

### Property 5: Exclusion Zone Compliance

*For any* generated clip from any video, the clip's time range should fall entirely within the valid selection zone (clip.start ≥ intro_cutoff AND clip.end ≤ outro_cutoff).

**Validates: Requirements 4.1**

### Property 6: Duration Constraint Compliance

*For any* generated clip, the clip duration should be between MIN_CLIP_DURATION (12 seconds) and MAX_CLIP_DURATION (18 seconds) inclusive.

**Validates: Requirements 5.1**

### Property 7: Sequential Naming Convention

*For any* set of N clips generated from a video, the clips should be named "vid1.mp4", "vid2.mp4", ..., "vidN.mp4" in sequential order.

**Validates: Requirements 6.1**

### Property 8: Output Directory Consistency

*For any* generated clip from a video at path P, the clip should be located in the "backdrops/" subdirectory relative to P's parent directory.

**Validates: Requirements 6.2**

### Property 9: Chronological Ordering

*For any* set of clips returned by a selector, the clips should be ordered by start time in ascending order (i.e., for all adjacent clips i and i+1, clip_i.start < clip_{i+1}.start).

**Validates: Requirements 7.4**

### Property 10: Warning Logging for Reduced Clip Count

*For any* video where fewer clips are generated than requested, a warning should be logged containing the video filename and the actual number of clips generated.

**Validates: Requirements 9.2**

### Property 11: No-Crash Guarantee for Constrained Videos

*For any* video file, regardless of duration or clip count requested, the processing should complete without panicking or returning a fatal error (it may return fewer clips than requested, but should not crash).

**Validates: Requirements 9.3**

## Error Handling

### Error Types

**SelectionError**
- `InsufficientDuration`: Video too short for even one valid clip
- `NoValidSegments`: Cannot find non-overlapping segments meeting constraints
- `AudioAnalysisFailed`: FFmpeg audio analysis failed (intense-audio strategy)

**ProcessError**
- `NoValidClips`: Selector returned empty vector
- `FFmpegError`: Clip extraction failed
- `InvalidPath`: Video path or output directory invalid
- `IoError`: File system operation failed

### Error Handling Strategy

1. **Graceful Degradation**: When full clip count cannot be met, generate as many valid clips as possible
2. **Warning Logging**: Log warnings for reduced clip counts, but continue processing
3. **Per-Video Isolation**: Failure on one video should not stop processing of other videos
4. **Clear Error Messages**: All errors should include context (video path, requested count, etc.)

### Edge Cases

1. **Very Short Videos**: Videos shorter than MIN_CLIP_DURATION + exclusion zones
   - Return empty vector, log warning, continue processing
   
2. **Clip Count = 1**: Should behave identically to legacy system
   - Single clip named "vid1.mp4"
   - All existing logic applies
   
3. **Maximum Clip Count (4)**: Videos that can barely fit 4 clips
   - May require many attempts to find non-overlapping segments
   - Implement attempt limit (e.g., 1000 attempts) to prevent infinite loops
   
4. **Overlapping Peak Detection**: For intensity-based strategies, multiple peaks in same region
   - Select highest intensity peak, skip overlapping lower-intensity peaks
   
5. **Exclusion Zones Larger Than Video**: Intro + outro exclusion > 100%
   - Return empty vector, log warning

## Testing Strategy

### Unit Tests

Unit tests will focus on specific examples, edge cases, and error conditions:

1. **CLI Validation**
   - Test valid inputs (1, 2, 3, 4)
   - Test invalid inputs (0, 5, -1, "abc")
   - Test default value when not specified

2. **TimeRange Overlap Detection**
   - Test overlapping ranges
   - Test non-overlapping ranges
   - Test adjacent ranges (touching but not overlapping)
   - Test edge cases (zero-duration, negative duration)

3. **Selector Edge Cases**
   - Video exactly long enough for N clips
   - Video too short for N clips
   - Exclusion zones leaving minimal valid zone
   - Single clip generation (backward compatibility)

4. **Naming Convention**
   - Test sequential naming for 1-4 clips
   - Test output directory creation
   - Test file path construction

5. **Error Handling**
   - Test graceful degradation for short videos
   - Test warning logging
   - Test no-crash guarantee

### Property-Based Tests

Property-based tests will verify universal properties across randomized inputs using `proptest` with minimum 100 iterations:

1. **Property 1: CLI Input Validation**
   - Generate random integers
   - Verify 1-4 accepted, others rejected
   - Tag: `Feature: multiple-clips-per-video, Property 1: CLI Input Validation`

2. **Property 2: Exact Clip Count Generation**
   - Generate random videos (varying duration)
   - Generate random clip counts (1-4)
   - Verify output count matches request (when possible)
   - Tag: `Feature: multiple-clips-per-video, Property 2: Exact Clip Count Generation`

3. **Property 3: Graceful Degradation**
   - Generate short videos with high clip counts
   - Verify system returns partial results without crashing
   - Tag: `Feature: multiple-clips-per-video, Property 3: Graceful Degradation for Short Videos`

4. **Property 4: Non-Overlapping Segments**
   - Generate random videos and clip counts
   - Verify no two clips overlap
   - Tag: `Feature: multiple-clips-per-video, Property 4: Non-Overlapping Segments`

5. **Property 5: Exclusion Zone Compliance**
   - Generate random videos with varying exclusion percentages
   - Verify all clips within valid zone
   - Tag: `Feature: multiple-clips-per-video, Property 5: Exclusion Zone Compliance`

6. **Property 6: Duration Constraint Compliance**
   - Generate random videos and clip counts
   - Verify all clips between 12-18 seconds
   - Tag: `Feature: multiple-clips-per-video, Property 6: Duration Constraint Compliance`

7. **Property 7: Sequential Naming Convention**
   - Generate random clip counts
   - Verify naming follows vid1.mp4, vid2.mp4, etc.
   - Tag: `Feature: multiple-clips-per-video, Property 7: Sequential Naming Convention`

8. **Property 8: Output Directory Consistency**
   - Generate random video paths
   - Verify all clips in backdrops/ subdirectory
   - Tag: `Feature: multiple-clips-per-video, Property 8: Output Directory Consistency`

9. **Property 9: Chronological Ordering**
   - Generate random videos and clip counts
   - Verify clips sorted by start time
   - Tag: `Feature: multiple-clips-per-video, Property 9: Chronological Ordering`

10. **Property 10: Warning Logging**
    - Generate constrained videos
    - Verify warnings logged when clip count reduced
    - Tag: `Feature: multiple-clips-per-video, Property 10: Warning Logging for Reduced Clip Count`

11. **Property 11: No-Crash Guarantee**
    - Generate random videos (including edge cases)
    - Verify processing completes without panic
    - Tag: `Feature: multiple-clips-per-video, Property 11: No-Crash Guarantee for Constrained Videos`

### Integration Tests

Integration tests will verify end-to-end behavior with real video files:

1. **Full Pipeline with Multiple Clips**
   - Create test video with sufficient duration
   - Run extractor with clip_count=3
   - Verify 3 clips generated with correct names
   - Verify clips are non-overlapping and within constraints

2. **Backward Compatibility**
   - Run with clip_count=1
   - Verify single clip named "vid1.mp4"
   - Compare behavior with legacy system

3. **Strategy-Specific Tests**
   - Test random strategy with multiple clips
   - Test intense-audio strategy with multiple clips
   - Verify strategy-specific behavior maintained

### Test Configuration

- **Property test iterations**: Minimum 100 per test
- **Test framework**: `proptest` for property-based tests, standard Rust test framework for unit tests
- **Integration test location**: `tests/` directory
- **Coverage target**: >80% line coverage
- **Test data**: Generate synthetic videos using FFmpeg for integration tests
