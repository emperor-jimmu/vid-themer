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
- Skip logic for directories with existing clips

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
    └── backdrop.mp4    # Extracted clip
```

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
- `--strategy` / `-s`: random | intense-audio (default: random)
- `--resolution` / `-r`: 720p | 1080p (default: 1080p)
- `--audio` / `-a`: true | false (default: true)
- `--intro-exclusion`: percentage of video duration to exclude from start (0-100, default: 1.0)
- `--outro-exclusion`: percentage of video duration to exclude from end (0-100, default: 40.0)
