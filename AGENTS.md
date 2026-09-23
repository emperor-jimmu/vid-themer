# Video Clip Extractor

## Build & Run

```bash
cargo build --release
./target/release/video-clip-extractor.exe <directory>  # Windows
./target/release/video-clip-extractor <directory>       # Unix
```

## Commands

```bash
cargo test                    # Run all tests
cargo test -- --nocapture     # Run tests with print output
cargo test -- --ignored       # Run property-based tests (proptest)
cargo clippy                  # Lint
cargo fmt                     # Format code
cargo check                   # Type check without building
```

## Supported Platforms

- Native binaries (release assets):
  - Windows 10/11 (`x86_64`)
  - Linux (`x86_64`)
  - macOS (Apple Silicon, `aarch64`)
- Docker image architectures:
  - `linux/amd64`
  - `linux/arm64/v8`

## Requirements

- **Rust** (Edition 2024; stable toolchain with 2024 edition support, e.g. Rust 1.85+)
- **FFmpeg** (must be in PATH): `ffmpeg` and `ffprobe` commands

## Build and Release Process

GitHub Actions workflow (`.github/workflows/main.yml`) currently:

- Runs tests on Linux, Windows, and macOS
- Builds release binaries for `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, and `aarch64-apple-darwin`
- Publishes GitHub Releases using `Cargo.toml` version as `v<version>`
- Builds and publishes multi-arch Docker images (`linux/amd64`, `linux/arm64/v8`) to GHCR and Docker Hub

## Architecture

```
src/
├── main.rs       # CLI entry point, orchestrates components
├── cli.rs        # Argument parsing (clap derive)
├── processor.rs  # Video processing orchestration
├── scanner.rs    # Recursive directory scanning
├── selector.rs   # Clip selection strategies (random, intense-audio, action)
├── ffmpeg/       # FFmpeg wrapper (executor, command_builder, analysis, metadata)
├── progress.rs  # Progress reporting
└── logger.rs    # Failure logging
```

## Key Behaviors

- **Movie folder format only**: Scans for directories matching `"Movie Name (Year)"` pattern (e.g., "The Matrix (1999)")
- Only processes videos inside movie folders, skips non-movie subdirectories
- Skips directories with `backdrops/done.ext` marker (unless `--force` is used)
- Creates `backdrops/backdrop1.mp4`, `backdrop2.mp4`, etc. (configurable via `-c` flag)
- Processes videos **sequentially** (FFmpeg itself is multi-threaded)
- Integration tests create temp videos via FFmpeg; skip if FFmpeg unavailable

## Testing

Integration tests are in `tests/` and use `tests/common/mod.rs` helpers:

- `create_test_video()` - creates test videos with FFmpeg
- `build_binary()` - compiles release binary
- `get_binary_path()` - returns platform-specific binary path

Tests that require FFmpeg: run with `cargo test` (they skip gracefully if FFmpeg is absent).

## Binary Name

- Windows: `video-clip-extractor.exe`
- Linux / macOS: `video-clip-extractor`

## Docker

```bash
docker build -t vid-themer .
docker run -d \
  -e VID_THEMER_STRATEGY=intense-audio \
  -e VID_THEMER_CLIP_COUNT=2 \
  -v /path/to/movies:/videos \
  vid-themer
```

`VID_THEMER_VIDEO_DIR` defaults to `/videos` in the image.

Or use docker-compose:
```bash
cp docker-compose.sample.yml docker-compose.yml
# Edit volume path, then:
docker compose up -d
```

### Environment Variables (all prefixed with `VID_THEMER_`)

| Variable | Default | Description |
|----------|---------|-------------|
| `VIDEO_DIR` | `/videos` | Directory to scan |
| `CRON_SCHEDULE` | `0 2 * * *` | Cron schedule |
| `STRATEGY` | `random` | `random`, `intense-audio`, `action` (`intense-action` alias supported) |
| `RESOLUTION` | `1080p` | `720p`, `1080p` |
| `CLIP_COUNT` | `2` | Number of clips (1-4) |
| `AUDIO` | `true` | Include audio |
| `FORCE` | `false` | Force regeneration |
| `HW_ACCEL` | `false` | Hardware acceleration |

### Docker Files

- `Dockerfile` - Multi-stage build (Rust builder + Alpine runtime with FFmpeg + cron)
- `docker-compose.sample.yml` - Sample compose configuration
- `entrypoint.sh` - Entry point script that builds CLI from env vars
- `crontab` - Cron configuration template
