# Video Clip Extractor

[![Build and Publish](https://github.com/emperor-jimmu/vid-themer/actions/workflows/main.yml/badge.svg)](https://github.com/emperor-jimmu/vid-themer/actions/workflows/main.yml)

A command-line tool that recursively scans directories for video files and automatically extracts short thematic clips from each video. Perfect for creating preview thumbnails or theme videos for media libraries.

## Features

- **Recursive Directory Scanning** - Automatically discovers video files in nested directories (sorted alphabetically)
- **Movie Folder Support** - Processes videos in directories matching `"Movie Name (Year)"` format
- **Intelligent Clip Selection** - Choose between random, audio-intensity-based, or action-based extraction strategies
- **Configurable Clip Duration** - Set minimum and maximum clip duration (default: 20-30 seconds)
- **Multiple Clips Per Video** - Generate 1-4 clips from each video with incremental generation support
- **Incremental Clip Generation** - Add more clips without regenerating existing ones
- **Configurable Exclusion Zones** - Control intro/outro exclusion as percentages of video duration
- **Smart Resolution Handling** - Scales videos down to target resolution without upscaling
- **Organized Output** - Creates `backdrops/` subdirectories with sequentially numbered clips (backdrop1.mp4, backdrop2.mp4, etc.)
- **Skip with done.ext** - Automatically skips directories with `backdrops/done.ext` marker
- **Force Regeneration** - Override existing clips with `--force` flag
- **0-Byte File Recovery** - Automatically re-processes videos with 0-byte backdrop files
- **Failure Logging** - Detailed error logs with FFmpeg output for debugging
- **Progress Tracking** - Real-time feedback on processing status
- **Hardware Acceleration** - Optional GPU encoding support (h264_videotoolbox on macOS, h264_nvenc elsewhere)

## Supported Platforms

- Windows 10/11
- Linux
- macOS (Apple Silicon)

## Requirements

- **Rust** (Edition 2024 - requires nightly or recent stable)
- **FFmpeg** - Must be installed and available in your system PATH
  - Includes `ffmpeg` and `ffprobe` commands

## Installation

### From Source

```bash
git clone <repository-url>
cd vid-themer
cargo build --release
```

The compiled binary will be available at:
- Windows: `target/release/video-clip-extractor.exe`
- Unix: `target/release/video-clip-extractor`

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
                                           [possible values: random, intense-audio, action, intense-action]
  -r, --resolution <RESOLUTION>          Target resolution for extracted clips [default: 1080p]
                                           [possible values: 720p, 1080p]
  -a, --audio <AUDIO>                    Include audio in extracted clips [default: true]
                                           [possible values: true, false]
  -c, --clip-count <COUNT>               Number of clips to generate per video (1-4) [default: 1]
      --intro-exclusion <PERCENT>        Intro exclusion zone as percentage of video duration (0-100) [default: 2.0]
      --outro-exclusion <PERCENT>        Outro exclusion zone as percentage of video duration (0-100) [default: 40.0]
      --min-duration <SECONDS>           Minimum clip duration in seconds [default: 20.0]
      --max-duration <SECONDS>           Maximum clip duration in seconds [default: 30.0]
  -f, --force                            Force regeneration of all clips, ignoring existing clips
      --hw-accel                         Use hardware acceleration for encoding (h264_videotoolbox on macOS, h264_nvenc elsewhere)
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

Force regeneration of all clips:

```bash
video-clip-extractor ~/Videos --force
```

Use hardware acceleration:

```bash
video-clip-extractor ~/Videos --hw-accel
```

## Selection Strategies

### Random Strategy (Default)

Selects random segments from the video with configurable duration (default: 20-30 seconds). By default, excludes the first 2% (intro) and last 40% (outro) of the video to skip opening credits and end credits. These exclusion zones and clip durations are configurable via CLI parameters.

### Intense Audio Strategy

Analyzes audio levels throughout the video and selects segments with the highest audio intensity, ideal for action scenes or dramatic moments. Falls back to middle-segment selection if audio analysis fails. Respects exclusion zones and duration constraints.

### Action Strategy

Analyzes video frames for motion/intensity and selects segments with the most visual activity. Also available as `intense-action` alias. Great for capturing high-energy scenes. Falls back to middle-segment selection if motion analysis fails. Respects exclusion zones and duration constraints.

## Output Structure

For each video file in a movie folder, the tool creates a subdirectory with the extracted clips:

```
/path/to/videos/
├── Movie1 (2020)/
│   ├── movie1.mp4
│   └── backdrops/
│       ├── backdrop1.mp4         # First clip from movie1.mp4
│       ├── backdrop2.mp4         # Second clip (if -c 2 or higher)
│       ├── backdrop3.mp4         # Third clip (if -c 3 or higher)
│       └── done.ext              # Marker file - directory is complete
├── Movie2 (2021)/
│   ├── movie2.mkv
│   └── backdrops/
│       └── backdrop1.mp4         # First clip from movie2.mkv
│       └── done.ext              # Processed and marked as complete
└── 2026-04-17-12-30-00.log      # Timestamped failure log (only if failures occur)
```

### Movie Folder Format

The tool expects directories in the format `"Movie Name (Year)"` (e.g., "The Matrix (1999)"). Videos must be directly inside these movie folders. Non-movie subdirectories are skipped.

### Skip Mechanism

Directories are automatically skipped when they contain a `backdrops/done.ext` marker file. This allows the tool to resume processing from where it left off without re-processing already-completed videos.

**How it works:**

1. After all requested clips are successfully generated, the tool creates `backdrops/done.ext`
2. The `done.ext` file contains a JSON timestamp indicating when processing completed:
   ```json
   {
     "completed_at": "2026-04-17T14:30:00+02:00"
   }
   ```
3. On subsequent runs, the scanner checks for this marker and skips the directory entirely
4. Use `--force` to override this behavior and re-process all videos

**Why use done.ext:**

- **Incremental runs**: Re-running the tool won't re-process already-completed videos
- **Interruption recovery**: If the tool is interrupted, it can resume cleanly
- **Manual control**: You can delete `done.ext` to force re-processing of a specific movie
- **Docker/cron**: Scheduled runs skip completed directories automatically

**To re-process a directory:**
```bash
# Option 1: Use --force flag to reprocess all videos
video-clip-extractor ~/Videos --force

# Option 2: Delete just the done.ext marker for specific movie
rm "~/Videos/Movie Name (2020)/backdrops/done.ext"
video-clip-extractor ~/Videos
```

### Incremental Clip Generation

The tool supports incremental clip generation, allowing you to add more clips without regenerating existing ones:

1. **Initial Run**: `video-clip-extractor ~/Videos -c 2`
   - Creates `backdrop1.mp4` and `backdrop2.mp4` for each video

2. **Add More Clips**: `video-clip-extractor ~/Videos -c 3`
   - Only creates `backdrop3.mp4` (preserves existing clips)

3. **Skip When Enough Exist**: `video-clip-extractor ~/Videos -c 2`
   - Skips videos that already have 2 or more valid clips

This feature is useful when you want to:
- Start with fewer clips and add more later
- Experiment with different clip counts without wasting processing time
- Incrementally build up your clip library

## Error Handling and Debugging

### Failure Logging

When processing failures occur, the tool creates a detailed log file in the current working directory with a timestamped filename:

- **Log Format**: `YYYY-MM-DD-HH-MM-SS.log` (e.g., `2026-04-17-12-30-00.log`)
- **Contents**:
  - Video file path that failed
  - Error message
  - Number of clips generated
  - Output path
  - FFmpeg stderr output (when applicable)

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

## Docker

You can run the tool as a Docker container with scheduled execution via cron.

### Quick Start

1. Copy the sample compose file:
   ```bash
   cp docker-compose.sample.yml docker-compose.yml
   ```

2. Edit `docker-compose.yml` and update the volume path to point to your movies directory:
   ```yaml
   volumes:
     - /path/to/your/movies:/videos:ro
   ```

3. Start the container:
   ```bash
   docker compose up -d
   ```

### Running Without Docker Compose

Pull and run directly from Docker Hub:

```bash
docker pull emperorjimmu/vid-themer:latest
docker run -d \
  --name vid-themer \
  -v /path/to/movies:/videos:ro \
  -e VID_THEMER_VIDEO_DIR=/videos \
  -e VID_THEMER_STRATEGY=intense-audio \
  -e VID_THEMER_CLIP_COUNT=2 \
  emperorjimmu/vid-themer:latest
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `VID_THEMER_VIDEO_DIR` | *(required)* | Directory to scan for videos |
| `VID_THEMER_CRON_SCHEDULE` | `0 2 * * *` | Cron schedule (daily at 2am) |
| `VID_THEMER_STRATEGY` | `random` | Clip selection: `random`, `intense-audio`, `action` |
| `VID_THEMER_RESOLUTION` | `1080p` | Output resolution: `720p`, `1080p` |
| `VID_THEMER_AUDIO` | `true` | Include audio in clips |
| `VID_THEMER_CLIP_COUNT` | `1` | Number of clips per video (1-4) |
| `VID_THEMER_INTRO_EXCLUSION` | `2.0` | Intro exclusion percentage |
| `VID_THEMER_OUTRO_EXCLUSION` | `40.0` | Outro exclusion percentage |
| `VID_THEMER_MIN_DURATION` | `20.0` | Minimum clip duration (seconds) |
| `VID_THEMER_MAX_DURATION` | `30.0` | Maximum clip duration (seconds) |
| `VID_THEMER_FORCE` | `false` | Force regeneration |
| `VID_THEMER_HW_ACCEL` | `false` | Hardware acceleration |

### Example docker-compose.yml

See [`docker-compose.sample.yml`](docker-compose.sample.yml) for a complete example.

### Viewing Logs

```bash
# View container logs
docker compose logs -f

# View application logs inside container
docker compose exec vid-themer cat /var/log/video-clip-extractor.log
```

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

```
src/
├── main.rs       # CLI entry point, orchestrates components
├── cli.rs        # Argument parsing (clap derive)
├── processor.rs  # Video processing orchestration
├── scanner.rs    # Recursive directory scanning
├── selector.rs   # Clip selection strategies (random, intense-audio, action)
├── ffmpeg/       # FFmpeg wrapper (executor, command_builder, analysis, metadata)
├── progress.rs   # Progress reporting
└── logger.rs    # Failure logging
```

### Components

1. **Scanner** - Discovers video files recursively, skips directories with `done.ext` markers
2. **Selector** - Chooses clip segments using configured strategy (random, intense-audio, or action)
3. **FFmpeg Executor** - Extracts clips with proper scaling and encoding
4. **Processor** - Orchestrates the extraction workflow
5. **Progress Reporter** - Provides real-time feedback
6. **Failure Logger** - Records detailed error information for debugging

## Dependencies

- `clap` (4.6) - CLI argument parsing with derive macros
- `thiserror` (2.0) - Error type definitions
- `rand` (0.10) - Random number generation for random strategy
- `walkdir` (2.5) - Directory traversal
- `serde` (1.0) - Serialization for metadata and done markers
- `serde_json` (1.0) - JSON parsing
- `colored` (3.1) - Terminal colors for progress output
- `rayon` (1.12) - Data-level parallelism
- `tokio` (1.52) - Async runtime
- `async-trait` (0.1) - Async trait support
- `chrono` (0.4) - Date/time handling for logging
- `proptest` (1.11) - Property-based testing (dev dependency)

## License

This project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
