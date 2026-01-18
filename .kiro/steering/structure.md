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
└── progress.rs          # Progress reporting and user feedback
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
- Scale filter calculation (no upscaling)
- Audio analysis for intensity-based selection

### Processing Pipeline (`processor.rs`)
- `VideoProcessor` - Orchestrates extraction workflow
- `ProcessResult` - Processing outcome tracking
- Output directory management

### Progress Reporting (`progress.rs`)
- `ProgressReporter` - Real-time progress updates
- Success/failure tracking and summary

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
tests/
├── unit/               # Unit tests for individual components
├── property/           # Property-based tests (proptest)
└── integration/        # End-to-end pipeline tests
```

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
