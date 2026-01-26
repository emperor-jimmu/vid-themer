# Technology Stack

## Language & Edition

- Rust (Edition 2024)

## Core Dependencies

- `clap` (with derive feature) - CLI argument parsing with type safety
- `thiserror` - Error type definitions
- `walkdir` or `std::fs` - Directory traversal

## Development Dependencies

- `proptest` - Property-based testing framework

## External Tools

- **FFmpeg** (required) - Video processing and analysis
- **ffprobe** (part of FFmpeg) - Video metadata extraction

## Architecture Pattern

Pipeline architecture: directory scanning → video discovery → clip selection → extraction → output organization

## Common Commands

### Build

```bash
cargo build --release
```

### Run

```bash
cargo run -- <directory> [options]
```

### Test

```bash
# Run all tests (unit + integration)
cargo test

# Run with verbose output
cargo test -- --nocapture

# Run only unit tests (in src/)
cargo test --bins

# Run specific integration test
cargo test --test full_pipeline
cargo test --test error_recovery
cargo test --test zero_byte_regeneration

# Run property tests with more iterations
cargo test -- --ignored
```

### Development

```bash
# Check code without building
cargo check

# Format code
cargo fmt

# Lint code
cargo clippy
```

## Testing Strategy

- **Unit tests**: Specific examples, edge cases, error conditions (located in `#[cfg(test)]` modules within source files)
- **Property-based tests**: Universal properties across randomized inputs using `proptest` (minimum 100 iterations)
- **Integration tests**: End-to-end pipeline validation with real video files (organized in `tests/integration/`)
- **Test organization**:
  - Unit tests: Co-located with source code in `src/*/tests` modules
  - Integration tests: Each file in `tests/` directory is a separate test binary
  - Common utilities: Shared helpers in `tests/common/`
- **Test features**:
  - `integration-ffmpeg`: Flag for expensive tests requiring FFmpeg
- Target: >80% line coverage
