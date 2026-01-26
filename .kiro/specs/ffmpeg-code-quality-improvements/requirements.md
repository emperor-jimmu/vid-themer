# Requirements Document

## Introduction

This document specifies requirements for code quality improvements to the `src/ffmpeg.rs` module in the video clip extractor project. The improvements focus on enhancing code robustness, maintainability, and adherence to Rust best practices without changing the module's public API or functionality.

## Glossary

- **FFmpeg_Module**: The `src/ffmpeg.rs` module responsible for FFmpeg command execution and video processing
- **Panic**: A Rust runtime error that causes the program to terminate abnormally
- **NaN**: Not-a-Number, a special floating-point value that can cause comparison operations to fail
- **Magic_Number**: A hardcoded numeric value in source code without clear semantic meaning
- **Named_Constant**: A constant with a descriptive identifier that documents its purpose
- **JSON_Parser**: A library component that parses JSON text into structured data
- **Stable_Rust**: The stable release channel of Rust that does not require experimental features
- **Property_Test**: A test that uses the proptest library to verify properties across generated inputs

## Requirements

### Requirement 1: Eliminate Unsafe Floating-Point Comparisons

**User Story:** As a developer, I want floating-point comparisons to handle NaN values safely, so that the program does not panic on unexpected input.

#### Acceptance Criteria

1. WHEN sorting audio segments by intensity, THE FFmpeg_Module SHALL use safe comparison methods that handle NaN values
2. WHEN sorting motion segments by score, THE FFmpeg_Module SHALL use safe comparison methods that handle NaN values
3. WHEN comparing any floating-point values, THE FFmpeg_Module SHALL NOT use `partial_cmp().unwrap()` which panics on NaN
4. WHEN encountering NaN values in comparisons, THE FFmpeg_Module SHALL handle them gracefully using `total_cmp()` or explicit None handling

### Requirement 2: Replace Manual JSON Parsing with Robust Library

**User Story:** As a developer, I want JSON parsing to use a standard library, so that the code is more maintainable and handles edge cases correctly.

#### Acceptance Criteria

1. WHEN parsing FFmpeg metadata JSON output, THE FFmpeg_Module SHALL use the `serde_json` crate
2. WHEN JSON parsing fails, THE FFmpeg_Module SHALL return descriptive error messages indicating what field failed to parse
3. THE FFmpeg_Module SHALL define structured types for JSON deserialization using serde derive macros
4. THE FFmpeg_Module SHALL NOT use manual string manipulation for JSON parsing

### Requirement 3: Extract Magic Numbers to Named Constants

**User Story:** As a developer, I want hardcoded values to be named constants with documentation, so that their purpose is clear and they are easy to modify.

#### Acceptance Criteria

1. THE FFmpeg_Module SHALL define named constants for all hardcoded numeric values
2. WHEN defining constants, THE FFmpeg_Module SHALL include documentation comments explaining their purpose
3. THE FFmpeg_Module SHALL group related constants together logically
4. THE FFmpeg_Module SHALL use constants for: bitrate values, CRF quality settings, keyframe intervals, seek offsets, analysis duration limits, and segment durations

### Requirement 4: Eliminate Duplicated Segment Analysis Logic

**User Story:** As a developer, I want segment grouping logic to be shared between audio and motion analysis, so that the code is DRY and easier to maintain.

#### Acceptance Criteria

1. WHEN grouping measurements into time segments, THE FFmpeg_Module SHALL use a shared helper function
2. THE shared helper function SHALL accept measurements, video duration, and segment duration as parameters
3. THE shared helper function SHALL return grouped segments with calculated scores
4. WHEN audio analysis groups measurements, THE FFmpeg_Module SHALL use the shared helper function
5. WHEN motion analysis groups measurements, THE FFmpeg_Module SHALL use the shared helper function

### Requirement 5: Enhance Error Context

**User Story:** As a developer, I want error messages to include relevant context, so that debugging is easier when failures occur.

#### Acceptance Criteria

1. WHEN ffprobe execution fails, THE FFmpeg_Module SHALL include the video file path in the error message
2. WHEN FFmpeg extraction fails, THE FFmpeg_Module SHALL include the video file path and time range in the error message
3. WHEN JSON parsing fails, THE FFmpeg_Module SHALL include the field name and raw value in the error message
4. WHEN validation fails, THE FFmpeg_Module SHALL include the specific validation failure reason in the error message

### Requirement 6: Use Stable Rust Features in Tests

**User Story:** As a developer, I want property tests to use stable Rust features, so that the code compiles without nightly compiler flags.

#### Acceptance Criteria

1. WHEN property tests generate multiple related values, THE FFmpeg_Module SHALL use stable Rust syntax
2. THE FFmpeg_Module SHALL NOT use `let...if` chains in property test generators
3. WHEN multiple values need conditional generation, THE FFmpeg_Module SHALL use tuples or nested if-let expressions
4. THE FFmpeg_Module SHALL compile successfully on stable Rust without feature flags

### Requirement 7: Clarify Error Extraction Logic

**User Story:** As a developer, I want the stderr extraction method to have clear and predictable behavior, so that error handling is consistent.

#### Acceptance Criteria

1. WHEN extracting stderr from FFmpegError, THE FFmpeg_Module SHALL have well-documented fallback behavior
2. THE `stderr()` method SHALL clearly document which error variants return stderr content
3. THE `stderr()` method SHALL have consistent string prefix handling across all error variants
4. WHEN an error variant does not contain stderr, THE `stderr()` method SHALL return None

### Requirement 8: Document Platform-Specific Code

**User Story:** As a developer, I want platform-specific hardware acceleration code to be well-documented, so that the conditional compilation logic is clear.

#### Acceptance Criteria

1. WHEN hardware acceleration code uses platform-specific features, THE FFmpeg_Module SHALL include documentation explaining the platform differences
2. THE FFmpeg_Module SHALL document why macOS uses VideoToolbox while other platforms use NVENC
3. THE FFmpeg_Module SHALL document the fallback behavior when hardware acceleration is unavailable
4. THE FFmpeg_Module SHALL include comments explaining the purpose of each `#[cfg]` directive

## Non-Functional Requirements

### Maintainability

1. THE FFmpeg_Module SHALL maintain backward compatibility with existing public APIs
2. THE FFmpeg_Module SHALL not change the behavior of any public methods
3. THE FFmpeg_Module SHALL preserve all existing test coverage

### Code Quality

1. THE FFmpeg_Module SHALL pass all existing unit tests and property tests after improvements
2. THE FFmpeg_Module SHALL compile without warnings on stable Rust
3. THE FFmpeg_Module SHALL follow Rust naming conventions for constants (SCREAMING_SNAKE_CASE)
