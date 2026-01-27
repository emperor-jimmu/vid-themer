# Requirements Document

## Introduction

This feature extends the Video Clip Extractor to support generating multiple clips per video file. Currently, the tool generates a single clip named "backdrop.mp4" per video. This enhancement allows users to specify how many clips (1-4) they want to generate, enabling richer preview collections while maintaining backward compatibility and respecting all existing constraints (exclusion zones, duration limits, non-overlapping segments).

## Glossary

- **Clip**: A short video segment (12-18 seconds) extracted from a source video
- **Clip_Count**: The number of clips to generate per video (1-4)
- **Time_Range**: A segment of video defined by start and end timestamps
- **Exclusion_Zone**: Portions of video (intro/outro) that should not be used for clip selection
- **Selection_Strategy**: Algorithm used to choose clip segments (random, intense-audio, action)
- **Non_Overlapping**: Property where no two clips share any portion of the source video timeline
- **Clip_Extractor**: The video processing system
- **CLI**: Command-line interface for user interaction
- **Selector**: Component responsible for choosing clip time ranges
- **Processor**: Component responsible for orchestrating clip extraction

## Requirements

### Requirement 1: CLI Parameter for Clip Count

**User Story:** As a user, I want to specify how many clips to generate per video, so that I can create richer preview collections.

#### Acceptance Criteria

1. THE CLI SHALL accept a `--clip-count` parameter with short form `-c`
2. WHEN the clip count parameter is provided, THE CLI SHALL validate it is an integer between 1 and 4 (inclusive)
3. WHEN the clip count parameter is not provided, THE CLI SHALL default to 1
4. WHEN an invalid clip count is provided, THE CLI SHALL reject the input and display an error message

### Requirement 2: Multiple Clip Generation

**User Story:** As a user, I want the tool to generate the specified number of clips per video, so that I have multiple preview options.

#### Acceptance Criteria

1. WHEN processing a video, THE Clip_Extractor SHALL generate exactly the number of clips specified by clip count
2. WHEN clip count is 1, THE Clip_Extractor SHALL behave identically to the current system
3. WHEN a video is too short to generate the requested number of clips, THE Clip_Extractor SHALL generate as many valid clips as possible and log a warning

### Requirement 3: Non-Overlapping Clip Segments

**User Story:** As a user, I want each clip to show different parts of the video, so that I get diverse preview content.

#### Acceptance Criteria

1. FOR ALL generated clips from a single video, THE Selector SHALL ensure no two clips share any overlapping time ranges
2. WHEN selecting multiple clips, THE Selector SHALL track used time ranges to prevent overlap
3. WHEN insufficient non-overlapping time is available, THE Selector SHALL generate fewer clips than requested

### Requirement 4: Exclusion Zone Compliance

**User Story:** As a user, I want all clips to respect intro and outro exclusion zones, so that clips avoid credits and opening sequences.

#### Acceptance Criteria

1. FOR ALL generated clips, THE Selector SHALL ensure each clip falls entirely within the valid selection zone (after intro exclusion, before outro exclusion)
2. WHEN calculating available time ranges, THE Selector SHALL exclude intro and outro zones before selecting clips
3. THE Selector SHALL apply the existing INTRO_EXCLUSION_PERCENT and OUTRO_EXCLUSION_PERCENT to all clips

### Requirement 5: Duration Constraint Compliance

**User Story:** As a user, I want all clips to meet duration requirements, so that clips are consistently sized.

#### Acceptance Criteria

1. FOR ALL generated clips, THE Selector SHALL ensure each clip duration is between MIN_CLIP_DURATION (12s) and MAX_CLIP_DURATION (18s)
2. WHEN selecting clip segments, THE Selector SHALL validate duration constraints before finalizing selection
3. WHEN a potential clip violates duration constraints, THE Selector SHALL reject it and select an alternative

### Requirement 6: Clip Naming Convention

**User Story:** As a user, I want clips to be named sequentially, so that I can easily identify and reference them.

#### Acceptance Criteria

1. WHEN generating clips, THE Processor SHALL name them "vid1.mp4", "vid2.mp4", "vid3.mp4", "vid4.mp4" in order
2. THE Processor SHALL place all clips in the "backdrops/" subdirectory relative to the source video
3. WHEN clip count is 1, THE Processor SHALL name the single clip "vid1.mp4" (not "backdrop.mp4")

### Requirement 7: Strategy Application to Multiple Clips

**User Story:** As a user, I want the selection strategy to apply intelligently to all clips, so that I get the best segments according to my chosen strategy.

#### Acceptance Criteria

1. WHEN using random strategy, THE Selector SHALL randomly select N non-overlapping segments from the valid selection zone
2. WHEN using intense-audio strategy, THE Selector SHALL select the N most intense audio segments that are non-overlapping and within the valid selection zone
3. WHEN using action strategy, THE Selector SHALL select the N most intense motion segments that are non-overlapping and within the valid selection zone
4. FOR ALL strategies, THE Selector SHALL return clips in chronological order by start time

### Requirement 8: Backward Compatibility

**User Story:** As an existing user, I want the tool to work exactly as before when I don't specify clip count, so that my workflows are not disrupted.

#### Acceptance Criteria

1. WHEN clip count is not specified or is 1, THE Clip_Extractor SHALL generate a single clip
2. WHEN clip count is 1, THE Processor SHALL name the output "vid1.mp4"
3. THE Clip_Extractor SHALL maintain all existing behavior for single-clip generation

### Requirement 9: Error Handling for Insufficient Video Length

**User Story:** As a user, I want clear feedback when a video is too short for multiple clips, so that I understand why fewer clips were generated.

#### Acceptance Criteria

1. WHEN a video cannot accommodate the requested number of non-overlapping clips within constraints, THE Clip_Extractor SHALL generate as many valid clips as possible
2. WHEN fewer clips are generated than requested, THE Clip_Extractor SHALL log a warning with the video filename and actual clip count
3. THE Clip_Extractor SHALL continue processing without failing when clip count cannot be met
