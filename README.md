# Video Clip Extractor

A command-line tool that recursively scans directories for video files and automatically extracts short thematic clips from each video. Perfect for creating preview thumbnails or theme videos for media libraries.

## Features

- **Recursive Directory Scanning** - Automatically discovers video files in nested directories (sorted alphabetically)
- **Intelligent Clip Selection** - Choose between random or audio-intensity-based extraction strategies
- **Configurable Clip Duration** - Set minimum and maximum clip duration (default: 20-30 seconds)
- **Multiple Clips Per Video** - Generate 1-4 clips from each video with incremental generation support
- **Incremental Clip Generation** - Add more clips without regenerating existing ones
- **Configurable Exclusion Zones** - Control intro/outro exclusion as percentages of video duration
- **Smart Resolution Handling** - Scales videos down to target resolution without upscaling
- **Organized Output** - Creates `backdrops/` subdirectories with sequentially numbered clips (backdrop1.mp4, backdrop2.mp4, etc.)
- **Skip Existing** - Automatically skips directories that already have enough extracted clips
- **0-Byte File Recovery** - Automatically re-processes videos with 0-byte backdrop files
- **Failure Logging** - Detailed error logs with FFmpeg output for debugging
- **Progress Tracking** - Real-time feedback on processing status

## Requirements

- **Rust** (Edition 2024 or later)
- **FFmpeg** - Must be installed and available in your system PATH
  - Includes `ffmpeg` and `ffprobe` commands

## Installation

### From Source

```bash
git clone <repository-url>
cd video-clip-extractor
cargo build --release
```

The compiled binary will be available at `target/release/video-clip-extractor`.

## Usage

### Basic Usage

```bash
video-clip-extractor /path/to/videos
```

### Options

```bash
video-clip-extractor [OPTIONS] <DIRECTORY>

Arguments:
  <DIRECTORY>  Root directory to scan for video files

Options:
  -s, --strategy <STRATEGY>              Clip selection strategy [default: random]
                                         [possible values: random, intense-audio, action]
  -r, --resolution <RESOLUTION>          Target resolution for extracted clips [default: 1080p]
                                         [possible values: 720p, 1080p]
  -a, --audio <AUDIO>                    Include audio in extracted clips [default: true]
                                         [possible values: true, false]
  -c, --clip-count <COUNT>               Number of clips to generate per video (1-4) [default: 1]
      --intro-exclusion <PERCENT>        Intro exclusion zone as percentage of video duration (0-100) [default: 2.0]
      --outro-exclusion <PERCENT>        Outro exclusion zone as percentage of video duration (0-100) [default: 40.0]
      --min-duration <SECONDS>           Minimum clip duration in seconds [default: 20.0]
      --max-duration <SECONDS>           Maximum clip duration in seconds [default: 30.0]
  -h, --help                             Print help
  -V, --version                          Print version
```

### Examples

Extract clips using random selection at 1080p with audio:

```bash
video-clip-extractor ~/Videos
```

Extract clips using audio intensity analysis at 720p:

```bash
video-clip-extractor ~/Videos --strategy intense-audio --resolution 720p
```

Extract silent clips:

```bash
video-clip-extractor ~/Videos --audio false
```

Customize exclusion zones (skip first 5% and last 30%):

```bash
video-clip-extractor ~/Videos --intro-exclusion 5 --outro-exclusion 30
```

No exclusion zones (select from entire video):

```bash
video-clip-extractor ~/Videos --intro-exclusion 0 --outro-exclusion 0
```

Generate exactly 10-second clips:

```bash
video-clip-extractor ~/Videos --min-duration 10 --max-duration 10
```

Generate clips between 5 and 20 seconds:

```bash
video-clip-extractor ~/Videos --min-duration 5 --max-duration 20
```

Generate multiple clips per video:

```bash
# Generate 2 clips per video
video-clip-extractor ~/Videos --clip-count 2

# Later, add a third clip without regenerating the first two
video-clip-extractor ~/Videos --clip-count 3
```

## Selection Strategies

### Random Strategy (Default)

Selects random segments from the video with configurable duration (default: 20-30 seconds). By default, excludes the first 2% (intro) and last 40% (outro) of the video to skip opening credits and end credits. These exclusion zones and clip durations are configurable via CLI parameters.

### Intense Audio Strategy

Analyzes audio levels throughout the video and selects segments with the highest audio intensity, ideal for action scenes or dramatic moments. Respects exclusion zones and duration constraints.

## Output Structure

For each video file, the tool creates a subdirectory with the extracted clips:

```
/path/to/videos/
├── movie1.mp4
├── backdrops/
│   ├── backdrop1.mp4         # First clip from movie1.mp4
│   ├── backdrop2.mp4         # Second clip (if -c 2 or higher)
│   └── backdrop3.mp4         # Third clip (if -c 3 or higher)
├── subfolder/
│   ├── movie2.mkv
│   └── backdrops/
│       └── backdrop1.mp4     # First clip from movie2.mkv
└── video_clip_extractor_failures.log  # Created only if failures occur
```

### Incremental Clip Generation

The tool supports incremental clip generation, allowing you to add more clips without regenerating existing ones:

1. **Initial Run**: `video-clip-extractor ~/Videos -c 2`
   - Creates `backdrop1.mp4` and `backdrop2.mp4` for each video

2. **Add More Clips**: `video-clip-extractor ~/Videos -c 3`
   - Only creates `backdrop3.mp4` (preserves existing clips)

3. **Skip When Enough Exist**: `video-clip-extractor ~/Videos -c 2`
   - Skips videos that already have 2 or more clips

This feature is useful when you want to:
- Start with fewer clips and add more later
- Experiment with different clip counts without wasting processing time
- Incrementally build up your clip library

## Error Handling and Debugging

### Failure Logging

When processing failures occur, the tool creates a detailed log file in the root directory:

- **Log File**: `video_clip_extractor_failures.log`
- **Contents**:
  - Video file path that failed
  - Error message
  - FFmpeg stderr output (when applicable)
  - Separated entries for each failure

This log file is invaluable for debugging issues with specific video files or FFmpeg processing errors.

### 0-Byte File Recovery

If a previous run created a 0-byte backdrop file (due to a crash or error), the tool will automatically detect this and re-process the video on subsequent runs. Only backdrop files with actual content (size > 0) are considered valid. The tool checks clips sequentially (backdrop1.mp4, backdrop2.mp4, etc.) and stops at the first missing or zero-byte file.

## Video Encoding

Extracted clips are re-encoded with the following settings:

- **Video Codec**: H.264 (libx264)
- **Compression**: CRF 26 (optimized for smaller file sizes with good quality)
- **Preset**: Fast (balanced encoding speed)
- **Audio Codec**: AAC (when audio is enabled)
- **Color Space**: BT.709 (standard HD color space)
- **Pixel Format**: YUV420P (maximum compatibility)
- **Keyframe Interval**: 30 frames (~1 second for better seeking)

The CRF 26 setting provides a good balance between file size and quality for backdrop/preview clips. Lower values (e.g., 18) produce higher quality but larger files, while higher values (e.g., 28) produce smaller files with reduced quality.

## Development

### Build

```bash
cargo build --release
```

### Run Tests

```bash
# Run all tests
cargo test

# Run with verbose output
cargo test -- --nocapture

# Run property-based tests
cargo test -- --ignored
```

### Code Quality

```bash
# Check code without building
cargo check

# Format code
cargo fmt

# Lint code
cargo clippy
```

## Architecture

The project follows a pipeline architecture:

1. **Scanner** - Discovers video files recursively, skips directories with valid backdrops
2. **Selector** - Chooses clip segments using configured strategy
3. **FFmpeg Executor** - Extracts clips with proper scaling and encoding
4. **Processor** - Orchestrates the extraction workflow
5. **Progress Reporter** - Provides real-time feedback
6. **Failure Logger** - Records detailed error information for debugging

## Dependencies

- `clap` - CLI argument parsing with derive macros
- `thiserror` - Error type definitions
- `walkdir` / `std::fs` - Directory traversal
- `proptest` - Property-based testing (dev dependency)

## License

[Add your license here]

## Contributing

[Add contribution guidelines here]
