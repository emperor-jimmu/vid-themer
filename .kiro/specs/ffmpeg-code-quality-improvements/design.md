# Design Document: FFmpeg Module Code Quality Improvements

## Overview

This design document outlines code quality improvements to the `src/ffmpeg.rs` module. The improvements focus on eliminating potential panics, enhancing maintainability through better code organization, and ensuring compatibility with stable Rust. All changes preserve the existing public API and functionality while making the codebase more robust and idiomatic.

The improvements address eight categories of issues ranging from high-priority safety concerns (unsafe unwrap operations) to medium-priority maintainability issues (code duplication) and low-priority documentation enhancements.

## Architecture

The FFmpeg module maintains its existing architecture with no structural changes:

- **FFmpegExecutor**: Main struct for FFmpeg command execution
- **VideoMetadata**: Struct for video file metadata
- **AudioSegment / MotionSegment**: Structs for analysis results
- **FFmpegError**: Error type with thiserror integration

The improvements are internal refactorings that do not affect the module's interface or interaction with other components.

## Components and Interfaces

### 1. Safe Floating-Point Comparisons

**Current Implementation:**
```rust
segments.sort_by(|a, b| b.intensity.partial_cmp(&a.intensity).unwrap());
```

**Problem:** The `unwrap()` will panic if either intensity value is NaN.

**Improved Implementation:**
```rust
segments.sort_by(|a, b| {
    b.intensity.total_cmp(&a.intensity)
});
```

**Rationale:** The `total_cmp()` method (stable since Rust 1.62) provides a total ordering for f64 values, treating NaN as equal to itself and less than all other values. This eliminates the panic risk while maintaining the desired sorting behavior.

**Affected Locations:**
- Line ~815: `analyze_audio_intensity` segment sorting
- Line ~1009: `analyze_motion_intensity` segment sorting

### 2. Robust JSON Parsing with serde_json

**Current Implementation:**
Manual string manipulation with `extract_json_value` and `extract_json_string_value` helper methods.

**Problem:** Fragile parsing that doesn't handle edge cases, nested structures, or escaped characters properly.

**Improved Implementation:**

Add `serde_json` dependency to `Cargo.toml`:
```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

Define structured types for deserialization:
```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct FFprobeOutput {
    streams: Vec<FFprobeStream>,
    format: FFprobeFormat,
}

#[derive(Debug, Deserialize)]
struct FFprobeStream {
    codec_name: String,
    width: u32,
    height: u32,
}

#[derive(Debug, Deserialize)]
struct FFprobeFormat {
    duration: String,
}
```

Replace `parse_metadata_json` method:
```rust
fn parse_metadata_json(&self, json_str: &str) -> Result<VideoMetadata, FFmpegError> {
    let output: FFprobeOutput = serde_json::from_str(json_str)
        .map_err(|e| FFmpegError::ParseError(format!("Failed to parse JSON: {}", e)))?;
    
    let stream = output.streams.first()
        .ok_or_else(|| FFmpegError::ParseError("No video stream found in JSON".to_string()))?;
    
    if output.format.duration == "N/A" || output.format.duration.is_empty() {
        return Err(FFmpegError::CorruptedFile(
            "Unable to determine video duration - file may be corrupted or incomplete".to_string()
        ));
    }
    
    let duration = output.format.duration.parse::<f64>()
        .map_err(|e| FFmpegError::ParseError(
            format!("Failed to parse duration '{}': {}", output.format.duration, e)
        ))?;
    
    Ok(VideoMetadata {
        duration,
        codec: stream.codec_name.clone(),
        width: stream.width,
        height: stream.height,
    })
}
```

**Rationale:** Using `serde_json` provides:
- Robust parsing that handles all JSON edge cases
- Clear error messages when parsing fails
- Type safety through structured deserialization
- Industry-standard approach that other developers will recognize

**Cleanup:** Remove the manual parsing helper methods `extract_json_value` and `extract_json_string_value` as they are no longer needed.

### 3. Named Constants for Magic Numbers

**Current Issues:**
- Bitrate: "5M" (line ~400, ~420)
- CRF quality: "26" (line ~440)
- Keyframe interval: "30" (line ~450)
- Fast seek offsets: 5.0 and 2.0 seconds (lines ~350, ~380)
- Max analysis duration: 300.0 seconds (lines ~700, ~850)
- Segment duration: 12.5 seconds (lines ~750, ~900)

**Improved Implementation:**

Add a constants module at the top of the file:
```rust
/// FFmpeg encoding and processing constants
mod constants {
    // Video encoding settings
    /// Target bitrate for hardware-accelerated encoding (5 Mbps)
    pub const HW_ACCEL_BITRATE: &str = "5M";
    
    /// Constant Rate Factor for software encoding (26 = good quality, smaller files)
    /// Range: 0-51, where lower = better quality, 18-28 is typical
    pub const SOFTWARE_CRF: &str = "26";
    
    /// Keyframe interval in frames (30 frames ≈ 1 second at 30fps)
    /// Ensures good seeking and streaming compatibility
    pub const KEYFRAME_INTERVAL: &str = "30";
    
    // Seeking optimization settings
    /// Fast seek offset for H.264 videos (seconds before target)
    /// Larger offset = faster seeking but more decoding needed
    pub const H264_FAST_SEEK_OFFSET: f64 = 5.0;
    
    /// Fast seek offset for HEVC videos (seconds before target)
    /// Smaller offset for HEVC due to more complex decoding
    pub const HEVC_FAST_SEEK_OFFSET: f64 = 2.0;
    
    // Analysis settings
    /// Maximum duration to analyze for long videos (5 minutes)
    /// Limits processing time while providing representative samples
    pub const MAX_ANALYSIS_DURATION: f64 = 300.0;
    
    /// Duration of each analysis segment (12.5 seconds)
    /// Balances granularity with statistical significance
    pub const SEGMENT_DURATION: f64 = 12.5;
    
    /// HEVC buffer size for analyzeduration and probesize (100 MB)
    /// Larger buffers help with HEVC's more complex structure
    pub const HEVC_BUFFER_SIZE: &str = "100M";
}
```

**Usage Examples:**
```rust
// Replace: "-crf".to_string(), "26".to_string()
"-crf".to_string(), constants::SOFTWARE_CRF.to_string()

// Replace: let fast_seek_offset = 5.0;
let fast_seek_offset = constants::H264_FAST_SEEK_OFFSET;

// Replace: const MAX_ANALYSIS_DURATION: f64 = 300.0;
let analysis_duration = duration.min(constants::MAX_ANALYSIS_DURATION);
```

**Rationale:** Named constants with documentation:
- Make the code self-documenting
- Centralize values that might need tuning
- Explain the reasoning behind specific values
- Follow Rust conventions (SCREAMING_SNAKE_CASE for constants)

### 4. Shared Segment Analysis Helper

**Current Issue:** Nearly identical code in `analyze_audio_intensity` and `analyze_motion_intensity` for grouping measurements into 12.5-second segments.

**Improved Implementation:**

Add a generic helper function:
```rust
/// Groups time-series measurements into fixed-duration segments and calculates aggregate scores
/// 
/// # Parameters
/// - `measurements`: Vector of (timestamp, value) pairs
/// - `video_duration`: Total duration of the video in seconds
/// - `analysis_duration`: Duration that was actually analyzed (may be less than video_duration)
/// - `segment_duration`: Duration of each segment in seconds
/// - `aggregate_fn`: Function to aggregate values within a segment (e.g., sum or average)
/// 
/// # Returns
/// Vector of (start_time, duration, score) tuples for each segment
fn group_measurements_into_segments<F>(
    measurements: &[(f64, f64)],
    video_duration: f64,
    analysis_duration: f64,
    segment_duration: f64,
    aggregate_fn: F,
) -> Vec<(f64, f64, f64)>
where
    F: Fn(&[f64]) -> f64,
{
    let scale_factor = video_duration / analysis_duration;
    let num_segments = (video_duration / segment_duration).ceil() as usize;
    
    let mut segments = Vec::new();
    
    for i in 0..num_segments {
        let segment_start = i as f64 * segment_duration;
        let segment_end = ((i + 1) as f64 * segment_duration).min(video_duration);
        let segment_duration_val = segment_end - segment_start;
        
        // Map to analyzed portion
        let analyzed_start = segment_start / scale_factor;
        let analyzed_end = segment_end / scale_factor;
        
        // Find all measurements within this segment
        let segment_measurements: Vec<f64> = measurements
            .iter()
            .filter(|(time, _)| *time >= analyzed_start && *time < analyzed_end)
            .map(|(_, value)| *value)
            .collect();
        
        if !segment_measurements.is_empty() {
            let score = aggregate_fn(&segment_measurements);
            segments.push((segment_start, segment_duration_val, score));
        }
    }
    
    segments
}
```

**Usage in audio analysis:**
```rust
let segments_data = group_measurements_into_segments(
    &measurements,
    duration,
    analysis_duration,
    constants::SEGMENT_DURATION,
    |values| values.iter().sum::<f64>() / values.len() as f64, // Average
);

let mut segments: Vec<AudioSegment> = segments_data
    .into_iter()
    .map(|(start, dur, intensity)| AudioSegment {
        start_time: start,
        duration: dur,
        intensity,
    })
    .collect();
```

**Usage in motion analysis:**
```rust
let segments_data = group_measurements_into_segments(
    &measurements,
    duration,
    analysis_duration,
    constants::SEGMENT_DURATION,
    |values| values.iter().sum::<f64>(), // Sum
);

let mut segments: Vec<MotionSegment> = segments_data
    .into_iter()
    .map(|(start, dur, score)| MotionSegment {
        start_time: start,
        duration: dur,
        motion_score: score,
    })
    .collect();
```

**Rationale:** This eliminates ~40 lines of duplicated code while making the aggregation strategy explicit (average for audio, sum for motion).

### 5. Enhanced Error Context

**Current Issues:**
- ffprobe errors don't include the video path
- FFmpeg extraction errors lack context about what was being processed

**Improved Implementation:**

Update error creation to include context:
```rust
// In get_video_metadata:
let output = Command::new("ffprobe")
    // ... args ...
    .output()
    .map_err(|e| FFmpegError::ExecutionFailed(
        format!("Failed to execute ffprobe on '{}': {}", 
            video_path.display(), e)
    ))?;

if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    if stderr.contains("EBML header parsing failed") 
        || stderr.contains("Invalid data found when processing input")
        || stderr.contains("moov atom not found")
        || stderr.contains("End of file") {
        return Err(FFmpegError::CorruptedFile(
            format!("Video file '{}' appears to be corrupted or incomplete: {}", 
                video_path.display(), stderr)
        ));
    }
    
    return Err(FFmpegError::ExecutionFailed(
        format!("ffprobe failed on '{}': {}", video_path.display(), stderr)
    ));
}

// In extract_clip:
if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    return Err(FFmpegError::ExecutionFailed(
        format!("FFmpeg clip extraction failed for '{}' at {:.2}s-{:.2}s: {}",
            video_path.display(),
            time_range.start_seconds,
            time_range.start_seconds + time_range.duration_seconds,
            stderr)
    ));
}
```

**Rationale:** Including file paths and time ranges in error messages makes debugging much easier, especially when processing many files in batch.

### 6. Stable Rust Syntax in Property Tests

**Current Issue:** Property tests use `let...if` chains which require unstable Rust features.

**Example from line ~1700:**
```rust
if let Some(time) = Self::extract_value_after(line, "t:")
    && let Some(peak) = Self::extract_value_after(line, "FTPK:")
{
    measurements.push((time, peak));
}
```

**Improved Implementation:**

Use nested if-let or match:
```rust
if let Some(time) = Self::extract_value_after(line, "t:") {
    if let Some(peak) = Self::extract_value_after(line, "FTPK:") {
        measurements.push((time, peak));
    }
}
```

Or use a tuple with pattern matching:
```rust
match (
    Self::extract_value_after(line, "t:"),
    Self::extract_value_after(line, "FTPK:")
) {
    (Some(time), Some(peak)) => {
        measurements.push((time, peak));
    }
    _ => {}
}
```

**Rationale:** Ensures the code compiles on stable Rust without requiring nightly features.

### 7. Clarified Error Extraction Logic

**Current Implementation:**
```rust
pub fn stderr(&self) -> Option<&str> {
    match self {
        FFmpegError::ExecutionFailed(msg) => {
            if msg.contains("FFmpeg clip extraction failed:") {
                msg.strip_prefix("FFmpeg clip extraction failed: ")
            } else if msg.contains("ffprobe failed:") {
                msg.strip_prefix("ffprobe failed: ")
            } else {
                Some(msg.as_str())
            }
        }
        _ => None,
    }
}
```

**Improved Implementation:**
```rust
/// Extracts stderr output from execution errors
/// 
/// Returns the stderr content if this is an ExecutionFailed error.
/// For other error types, returns None.
/// 
/// # Behavior
/// - For ExecutionFailed errors, attempts to strip known prefixes to extract raw stderr
/// - If no known prefix is found, returns the entire error message
/// - For all other error variants, returns None
pub fn stderr(&self) -> Option<&str> {
    match self {
        FFmpegError::ExecutionFailed(msg) => {
            // Try to extract stderr by stripping known prefixes
            msg.strip_prefix("FFmpeg clip extraction failed for ")
                .and_then(|s| s.split_once(": ").map(|(_, stderr)| stderr))
                .or_else(|| msg.strip_prefix("ffprobe failed on ")
                    .and_then(|s| s.split_once(": ").map(|(_, stderr)| stderr)))
                .or_else(|| msg.strip_prefix("Failed to execute ffprobe on ")
                    .and_then(|s| s.split_once(": ").map(|(_, stderr)| stderr)))
                .or(Some(msg.as_str())) // Fallback: return entire message
        }
        _ => None,
    }
}
```

**Rationale:** The improved version:
- Has clear documentation explaining behavior
- Uses more robust parsing that handles the enhanced error messages
- Makes the fallback behavior explicit
- Is easier to extend when new error message formats are added

### 8. Platform-Specific Code Documentation

**Current Implementation:**
```rust
if self.use_hw_accel {
    #[cfg(target_os = "macos")]
    {
        args.extend(vec![
            "-c:v".to_string(),
            "h264_videotoolbox".to_string(),
            "-b:v".to_string(),
            "5M".to_string(),
        ]);
    }
    #[cfg(not(target_os = "macos"))]
    {
        args.extend(vec![
            "-c:v".to_string(),
            "h264_nvenc".to_string(),
            "-preset".to_string(),
            "p4".to_string(),
            "-b:v".to_string(),
            "5M".to_string(),
        ]);
    }
}
```

**Improved Implementation:**
```rust
if self.use_hw_accel {
    // Hardware acceleration uses platform-specific encoders:
    // - macOS: VideoToolbox (h264_videotoolbox) - Apple's native hardware encoder
    //   Available on all Macs with hardware encoding support
    // - Other platforms: NVENC (h264_nvenc) - NVIDIA GPU encoder
    //   Requires NVIDIA GPU with NVENC support
    //   Falls back to software encoding if NVENC is unavailable
    #[cfg(target_os = "macos")]
    {
        args.extend(vec![
            "-c:v".to_string(),
            "h264_videotoolbox".to_string(),
            "-b:v".to_string(),
            constants::HW_ACCEL_BITRATE.to_string(),
        ]);
    }
    #[cfg(not(target_os = "macos"))]
    {
        args.extend(vec![
            "-c:v".to_string(),
            "h264_nvenc".to_string(),
            "-preset".to_string(),
            "p4".to_string(), // NVENC preset p4 = balanced quality/speed
            "-b:v".to_string(),
            constants::HW_ACCEL_BITRATE.to_string(),
        ]);
    }
}
```

**Rationale:** Clear comments explain:
- Why different platforms use different encoders
- What hardware is required
- What happens when hardware is unavailable
- The meaning of platform-specific settings

## Data Models

No changes to data models. All existing structs remain unchanged:
- `FFmpegExecutor`
- `VideoMetadata`
- `AudioSegment`
- `MotionSegment`
- `FFmpegError`

New internal types for JSON parsing:
- `FFprobeOutput`
- `FFprobeStream`
- `FFprobeFormat`

These are private implementation details not exposed in the public API.


## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system—essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: NaN-Safe Segment Sorting

*For any* collection of segments (audio or motion) that includes NaN values in their score fields, sorting the segments should complete without panicking and should produce a stable ordering where NaN values are consistently placed.

**Validates: Requirements 1.1, 1.2**

**Rationale:** Using `total_cmp()` instead of `partial_cmp().unwrap()` ensures that NaN values are handled gracefully. The `total_cmp()` method treats NaN as equal to itself and less than all other values, providing a total ordering that prevents panics.

**Test Strategy:** Generate random collections of segments with some containing NaN intensity/score values. Sort them and verify: (1) no panic occurs, (2) all NaN values are grouped together, (3) non-NaN values maintain their relative ordering.

### Property 2: JSON Parse Error Messages Include Field Context

*For any* invalid JSON input with a missing or malformed field, the parsing error message should contain the name of the field that failed to parse.

**Validates: Requirements 2.2**

**Rationale:** Using `serde_json` provides structured error messages that identify which field caused the parsing failure, making debugging much easier than generic "parse failed" messages.

**Test Strategy:** Create example JSON inputs with various missing or malformed fields (missing codec_name, invalid width, malformed duration). Parse each and verify the error message contains the specific field name.

### Property 3: Segment Grouping Correctness

*For any* set of time-series measurements, video duration, and segment duration, the segment grouping helper function should correctly partition measurements into non-overlapping time windows and aggregate values within each window.

**Validates: Requirements 4.3**

**Rationale:** The shared helper function must correctly handle the mapping between analyzed duration and full video duration, properly partition measurements into segments, and apply the aggregation function consistently.

**Test Strategy:** Generate random measurements with timestamps, video durations, and segment durations. Group them and verify: (1) all measurements are assigned to exactly one segment, (2) segment boundaries are correct, (3) no measurements are lost or duplicated, (4) aggregation is applied correctly.

### Property 4: Error Messages Include File Path Context

*For any* ffprobe or FFmpeg operation that fails, the error message should include the path to the video file being processed.

**Validates: Requirements 5.1, 5.2**

**Rationale:** Including file paths in error messages is essential for debugging, especially when processing multiple files in batch. The path provides immediate context about which file caused the failure.

**Test Strategy:** Trigger failures with nonexistent files or invalid files and verify error messages contain the file path. For extraction failures, also verify the time range is included.

### Property 5: Stderr Extraction Consistency

*For any* ExecutionFailed error with various message prefixes, the stderr() method should consistently extract the stderr content by stripping known prefixes, and should return the full message if no known prefix is found.

**Validates: Requirements 7.3**

**Rationale:** The stderr() method needs to handle multiple error message formats consistently. By using a chain of prefix-stripping attempts with a fallback, we ensure consistent behavior regardless of which code path created the error.

**Test Strategy:** Create ExecutionFailed errors with various prefixes (ffprobe, FFmpeg extraction, etc.) and verify stderr() correctly extracts the stderr portion. Also test with unknown prefixes to verify fallback behavior.

### Property 6: Non-Execution Errors Return None for Stderr

*For any* FFmpegError variant other than ExecutionFailed, the stderr() method should return None.

**Validates: Requirements 7.4**

**Rationale:** Only ExecutionFailed errors contain stderr output from FFmpeg/ffprobe. Other error types (NotFound, ParseError, NoAudioTrack, CorruptedFile) are generated by our code and don't have associated stderr.

**Test Strategy:** Create instances of each non-ExecutionFailed error variant and verify stderr() returns None for all of them.

### Property 7: Stable Rust Compilation

*For any* valid stable Rust toolchain (version 1.62+), the FFmpeg module should compile successfully without requiring nightly features or feature flags.

**Validates: Requirements 6.4**

**Rationale:** Using stable Rust features ensures the code is portable and doesn't break when Rust updates. The `total_cmp()` method and nested if-let are both stable features.

**Test Strategy:** Compile the module with a stable Rust toolchain and verify successful compilation with no warnings about unstable features.

## Error Handling

Error handling remains unchanged in terms of the error types and their meanings. The improvements enhance error messages with additional context but do not change the error handling strategy:

- **FFmpegError::NotFound**: FFmpeg/ffprobe not in PATH
- **FFmpegError::ExecutionFailed**: Command execution failed (now with file path and operation context)
- **FFmpegError::ParseError**: Failed to parse output (now with field-specific context from serde_json)
- **FFmpegError::NoAudioTrack**: Video has no audio stream
- **FFmpegError::CorruptedFile**: Video file is corrupted or incomplete (now with file path)

The `stderr()` method is improved to handle the enhanced error message formats consistently.

## Testing Strategy

### Unit Tests

Unit tests will verify specific examples and edge cases:

1. **NaN Handling Examples**:
   - Sort a small collection with one NaN value
   - Sort a collection with all NaN values
   - Sort a collection with NaN and infinity values

2. **JSON Parsing Examples**:
   - Parse valid JSON successfully
   - Parse JSON with missing codec_name field
   - Parse JSON with invalid width (non-numeric)
   - Parse JSON with "N/A" duration
   - Parse JSON with empty duration

3. **Error Context Examples**:
   - Trigger ffprobe failure with nonexistent file
   - Trigger extraction failure with invalid time range
   - Verify error messages contain expected context

4. **Stderr Extraction Examples**:
   - Extract stderr from ffprobe error
   - Extract stderr from extraction error
   - Verify None for ParseError
   - Verify None for NotFound

5. **Stable Rust Compilation**:
   - Verify compilation succeeds on stable Rust
   - Verify no unstable feature warnings

### Property-Based Tests

Property tests will verify universal properties across randomized inputs (minimum 100 iterations each):

1. **Property 1: NaN-Safe Segment Sorting**
   ```rust
   // Feature: ffmpeg-code-quality-improvements, Property 1: NaN-Safe Segment Sorting
   #[proptest]
   fn test_nan_safe_sorting(
       segments: Vec<AudioSegment>, // Some with NaN intensity
   ) {
       // Should not panic
       let mut sorted = segments.clone();
       sorted.sort_by(|a, b| b.intensity.total_cmp(&a.intensity));
       
       // Verify stable ordering
       // All NaN values should be grouped together
       // Non-NaN values should maintain relative order
   }
   ```

2. **Property 3: Segment Grouping Correctness**
   ```rust
   // Feature: ffmpeg-code-quality-improvements, Property 3: Segment Grouping Correctness
   #[proptest]
   fn test_segment_grouping(
       measurements: Vec<(f64, f64)>, // (time, value) pairs
       video_duration: f64,
       segment_duration: f64,
   ) {
       let segments = group_measurements_into_segments(
           &measurements,
           video_duration,
           video_duration, // analysis_duration = video_duration
           segment_duration,
           |values| values.iter().sum::<f64>(),
       );
       
       // Verify all measurements are accounted for
       // Verify no overlapping segments
       // Verify correct aggregation
   }
   ```

3. **Property 5: Stderr Extraction Consistency**
   ```rust
   // Feature: ffmpeg-code-quality-improvements, Property 5: Stderr Extraction Consistency
   #[proptest]
   fn test_stderr_extraction_consistency(
       prefix: String,
       stderr_content: String,
   ) {
       let msg = format!("{}: {}", prefix, stderr_content);
       let error = FFmpegError::ExecutionFailed(msg);
       
       let extracted = error.stderr();
       // Should extract stderr or return full message
       assert!(extracted.is_some());
   }
   ```

### Integration with Existing Tests

All existing tests must continue to pass after the improvements:
- Existing unit tests in `#[cfg(test)]` modules
- Existing property tests
- Integration tests in `tests/` directory

The improvements are internal refactorings that preserve all existing behavior, so no test modifications should be needed beyond adding new tests for the improved error messages.

### Test Coverage Goals

- Maintain existing >80% line coverage
- Add specific coverage for:
  - NaN handling in sorting (new code paths)
  - JSON parsing with serde_json (replacement code)
  - Enhanced error message formatting (modified code)
  - Segment grouping helper function (new shared code)
  - Stderr extraction with new message formats (modified code)

## Implementation Notes

### Dependency Changes

Add to `Cargo.toml`:
```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

These are widely-used, stable crates with minimal impact on compile times and binary size.

### Migration Strategy

The improvements can be implemented incrementally:

1. **Phase 1: Safety** (High Priority)
   - Replace `partial_cmp().unwrap()` with `total_cmp()`
   - Add tests for NaN handling
   - Verify no panics with edge case inputs

2. **Phase 2: Robustness** (High Priority)
   - Add serde_json dependency
   - Implement structured JSON types
   - Replace manual parsing with serde_json
   - Update tests for new error messages

3. **Phase 3: Maintainability** (Medium Priority)
   - Extract magic numbers to named constants
   - Create shared segment grouping helper
   - Refactor audio and motion analysis to use helper
   - Verify existing tests still pass

4. **Phase 4: Polish** (Low Priority)
   - Enhance error messages with context
   - Update stderr() method
   - Fix unstable Rust syntax in tests
   - Add platform-specific documentation

Each phase can be implemented, tested, and merged independently.

### Backward Compatibility

All changes maintain backward compatibility:
- Public API remains unchanged
- Function signatures remain unchanged
- Error types remain unchanged (only message content improves)
- Behavior remains unchanged (only robustness improves)

Existing code using the FFmpeg module will continue to work without modifications.

## Performance Considerations

The improvements have minimal performance impact:

1. **total_cmp() vs partial_cmp()**: Negligible difference, both are O(1) operations
2. **serde_json parsing**: Likely faster than manual string manipulation, definitely more robust
3. **Shared helper function**: No performance change, just code organization
4. **Enhanced error messages**: Only affects error paths, not hot paths

The improvements prioritize correctness and maintainability over micro-optimizations, which is appropriate for error handling and setup code.

## Security Considerations

The improvements enhance security by:

1. **Eliminating panics**: Prevents denial-of-service from malformed input
2. **Robust JSON parsing**: Prevents injection attacks through malformed JSON
3. **Better error messages**: Helps identify security issues during debugging

No new security concerns are introduced by these changes.
