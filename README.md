# Video Clip Extractor

A command-line tool that recursively scans directories for video files and automatically extracts short thematic clips (5-10 seconds) from each video. Perfect for creating preview thumbnails or theme videos for media libraries.

## Features

- **Recursive Directory Scanning** - Automatically discovers video files in nested directories
- **Intelligent Clip Selection** - Choose between random or audio-intensity-based extraction strategies
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
  -s, --strategy <STRATEGY>      Clip selection strategy [default: random]
                                 [possible values: random, intense-audio]
  -r, --resolution <RESOLUTION>  Target resolution for extracted clips [default: 1080p]
                                 [possible values: 720p, 1080p]
  -a, --audio <AUDIO>           Include audio in extracted clips [default: true]
                                 [possible values: true, false]
  -h, --help                    Print help
  -V, --version                 Print version
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

## Selection Strategies

### Random Strategy (Default)
Selects a random 5-10 second segment from the video, avoiding the first and last 10% to skip intros/credits.

### Intense Audio Strategy
Analyzes audio levels throughout the video and selects the segment with the highest audio intensity, ideal for action scenes or dramatic moments.

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
