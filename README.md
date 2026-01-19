# Video Clip Extractor

A command-line tool that recursively scans directories for video files and automatically extracts short thematic clips (10-15 seconds) from each video. Perfect for creating preview thumbnails or theme videos for media libraries.

## Features

- **Recursive Directory Scanning** - Automatically discovers video files in nested directories
- **Intelligent Clip Selection** - Choose between random or audio-intensity-based extraction strategies
- **Configurable Exclusion Zones** - Control intro/outro exclusion as percentages of video duration
- **Smart Resolution Handling** - Scales videos down to target resolution without upscaling
- **Organized Output** - Creates `backdrops/backdrop.mp4` subdirectories next to source videos
- **Skip Existing** - Automatically skips directories that already have extracted clips
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
                                         [possible values: random, intense-audio]
  -r, --resolution <RESOLUTION>          Target resolution for extracted clips [default: 1080p]
                                         [possible values: 720p, 1080p]
  -a, --audio <AUDIO>                    Include audio in extracted clips [default: true]
                                         [possible values: true, false]
      --intro-exclusion <PERCENT>        Intro exclusion zone as percentage of video duration (0-100) [default: 1.0]
      --outro-exclusion <PERCENT>        Outro exclusion zone as percentage of video duration (0-100) [default: 40.0]
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

## Selection Strategies

### Random Strategy (Default)
Selects a random 10-15 second segment from the video. By default, excludes the first 1% (intro) and last 40% (outro) of the video to skip opening credits and end credits. These exclusion zones are configurable via CLI parameters.

### Intense Audio Strategy
Analyzes audio levels throughout the video and selects the segment with the highest audio intensity, ideal for action scenes or dramatic moments. Note: Exclusion zones do not apply to this strategy as it selects based on audio analysis.

## Output Structure

For each video file, the tool creates a subdirectory with the extracted clip:

```
/path/to/videos/
├── movie1.mp4
├── backdrops/
│   └── backdrop.mp4          # Extracted clip from movie1.mp4
├── subfolder/
│   ├── movie2.mkv
│   └── backdrops/
│       └── backdrop.mp4      # Extracted clip from movie2.mkv
```

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

1. **Scanner** - Discovers video files recursively
2. **Selector** - Chooses clip segments using configured strategy
3. **FFmpeg Executor** - Extracts clips with proper scaling and encoding
4. **Processor** - Orchestrates the extraction workflow
5. **Progress Reporter** - Provides real-time feedback

## Dependencies

- `clap` - CLI argument parsing with derive macros
- `thiserror` - Error type definitions
- `walkdir` / `std::fs` - Directory traversal
- `proptest` - Property-based testing (dev dependency)

## License

[Add your license here]

## Contributing

[Add contribution guidelines here]
