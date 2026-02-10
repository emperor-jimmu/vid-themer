# Project Structure

## Module Organization

```
src/
├── main.rs              # CLI entry point and application orchestration
├── cli.rs               # CLI argument parsing (clap derive macros)
├── scanner.rs           # Video file discovery and directory traversal
├── selector.rs          # Clip selection strategies (trait + implementations)
├── ffmpeg.rs            # FFmpeg command execution and video processing
├── processor.rs         # Video processing pipeline coordination
├── progress.rs          # Progress reporting and user feedback
├── logger.rs            # Failure logging for debugging
└── error.rs             # Error type definitions
```

## Key Components

### CLI Layer (`cli.rs`)

- `CliArgs` struct with clap derive macros
- Argument validation and parsing
- Strategy and resolution enums

### Scanner (`scanner.rs`)

- `VideoScanner` - Recursive directory traversal
- `VideoFile` - Video file representation
- Skip logic for directories with existing clips based on requested clip count
- Counts existing clips (backdrop1.mp4, backdrop2.mp4, etc.) and skips directories that already have enough clips

### Selection Strategies (`selector.rs`)

- `ClipSelector` trait - Strategy interface
- `RandomSelector` - Random segment selection with exclusion zones
- `IntenseAudioSelector` - Audio intensity-based selection
- `TimeRange` - Time segment representation

### FFmpeg Integration (`ffmpeg.rs`)

- `FFmpegExecutor` - Command construction and execution
- Duration and resolution detection
- Codec detection for adaptive seeking
- Scale filter calculation (no upscaling)
- Audio analysis for intensity-based selection
- Codec-aware seeking strategy:
  - H.264/AVC: Aggressive hybrid seeking (5-second fast seek + accurate seek) for best performance
  - HEVC/H.265: Moderate hybrid seeking (2-second fast seek + accurate seek) with increased buffer sizes for reliability
- H.264 encoding with CRF 26 compression

### Processing Pipeline (`processor.rs`)

- `VideoProcessor` - Orchestrates extraction workflow
- `ProcessResult` - Processing outcome tracking
- Output directory management

### Progress Reporting (`progress.rs`)

- `ProgressReporter` - Real-time progress updates
- Success/failure tracking and summary
- Integration with failure logger

### Failure Logging (`logger.rs`)

- `FailureLogger` - Writes detailed failure information to log file
- Captures FFmpeg stderr output for debugging
- Creates `video_clip_extractor_failures.log` in the root directory

## Error Handling

All modules use `thiserror` for error types:

- `AppError` - Top-level application errors
- `ScanError` - Directory scanning errors
- `FFmpegError` - FFmpeg execution errors
- `SelectionError` - Clip selection errors
- `ProcessError` - Video processing errors

## Output Structure

For each processed video, output is organized as:

```
<video-directory>/
├── video.mp4
└── backdrops/
    ├── backdrop1.mp4    # First extracted clip
    ├── backdrop2.mp4    # Second extracted clip (if -c 2 or higher)
    ├── backdrop3.mp4    # Third extracted clip (if -c 3 or higher)
    └── backdrop4.mp4    # Fourth extracted clip (if -c 4)
```

**Incremental Clip Generation:**
- When running with `-c N`, the tool checks for existing clips (backdrop1.mp4, backdrop2.mp4, etc.)
- If fewer than N clips exist, only the missing clips are generated
- If N or more clips exist, the video is skipped entirely
- Example: Running with `-c 2` creates backdrop1.mp4 and backdrop2.mp4. Running again with `-c 3` only creates backdrop3.mp4

## Testing Organization

```
src/
├── scanner.rs          # Unit & property tests in #[cfg(test)] mod tests
├── selector.rs         # Unit & property tests in #[cfg(test)] mod tests
├── ffmpeg.rs           # Unit & property tests in #[cfg(test)] mod tests
├── processor.rs        # Unit & property tests in #[cfg(test)] mod tests
└── ...

tests/
├── common/
│   └── mod.rs                  # Shared test utilities (create_test_video, etc.)
├── full_pipeline.rs            # End-to-end pipeline tests
├── error_recovery.rs           # Error handling and recovery tests
└── zero_byte_regeneration.rs  # 0-byte backdrop regeneration tests
```

**Test Organization Principles:**

- **Unit tests**: Co-located with source code in `#[cfg(test)]` modules (standard Rust practice)
- **Property tests**: Using `proptest`, included in unit test modules
- **Integration tests**: Each file in `tests/` is a separate test binary (Rust convention)
- **Common utilities**: Shared helpers in `tests/common/mod.rs` to avoid duplication

Property tests must include comments linking to design properties:

```rust
// Feature: video-clip-extractor, Property 2: Video File Discovery
#[proptest]
fn test_video_file_discovery(...) { }
```

## Configuration

No configuration files - all settings via CLI arguments:

- Directory path (required positional)
- `--strategy` / `-s`: random | intense-audio | action (default: random)
- `--resolution` / `-r`: 720p | 1080p (default: 1080p)
- `--audio` / `-a`: true | false (default: true)
- `--clip-count` / `-c`: number of clips per video, 1-4 (default: 1)
- `--intro-exclusion`: percentage of video duration to exclude from start (0-100, default: 2.0)
- `--outro-exclusion`: percentage of video duration to exclude from end (0-100, default: 40.0)
- `--min-duration`: minimum clip duration in seconds (default: 10.0)
- `--max-duration`: maximum clip duration in seconds (default: 15.0)
