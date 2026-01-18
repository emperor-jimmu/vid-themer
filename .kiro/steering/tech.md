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
# Run all tests
cargo test

# Run with verbose output
cargo test -- --nocapture

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
- **Unit tests**: Specific examples, edge cases, error conditions
- **Property-based tests**: Universal properties across randomized inputs (minimum 100 iterations)
- **Integration tests**: End-to-end pipeline validation with real video files
- Target: >80% line coverage
