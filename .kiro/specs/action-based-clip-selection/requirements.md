# Requirements Document

## Introduction

This document specifies the requirements for adding an action-based clip selection strategy to the Video Clip Extractor tool. The new strategy will identify and extract clips from video segments containing high motion or action, providing users with dynamic, visually engaging preview clips. This complements the existing random and audio-based selection strategies by focusing on visual activity rather than temporal randomness or audio intensity.

## Glossary

- **Video_Clip_Extractor**: The command-line tool that recursively scans directories for video files and extracts short thematic clips
- **Action_Selector**: The new clip selection strategy implementation that identifies high-motion segments using FFmpeg's motion analysis capabilities
- **ClipSelector**: The trait interface that all selection strategies must implement
- **FFmpeg**: External video processing tool used for analysis and extraction
- **Motion_Score**: A numeric value representing the amount of motion or scene changes in a video segment, derived from FFmpeg's scene detection or motion vector analysis
- **Exclusion_Zone**: A configurable percentage of video duration at the beginning (intro) or end (outro) that is excluded from clip selection
- **Time_Range**: A data structure representing a start time and duration for a video clip segment
- **Scene_Change**: A significant visual transition in video content detected by analyzing frame differences

## Requirements

### Requirement 1: Action-Based Selection Strategy

**User Story:** As a user, I want to extract clips from high-action segments of my videos, so that the preview clips showcase the most visually dynamic and engaging moments.

#### Acceptance Criteria

1. WHEN the user specifies the "action" or "intense-action" strategy, THE Action_Selector SHALL analyze the video for motion intensity
2. WHEN analyzing motion intensity, THE Action_Selector SHALL use FFmpeg's scene detection or motion analysis filters to identify high-action segments
3. WHEN multiple high-action segments are identified, THE Action_Selector SHALL select the segment with the highest motion score
4. WHEN multiple segments have similar motion scores (within a threshold), THE Action_Selector SHALL select the first occurrence
5. WHEN no high-action segments are found or analysis fails, THE Action_Selector SHALL fall back to selecting a middle segment

### Requirement 2: FFmpeg Motion Analysis Integration

**User Story:** As a developer, I want to leverage FFmpeg's built-in motion analysis capabilities, so that I can identify action-rich segments without implementing complex video analysis from scratch.

#### Acceptance Criteria

1. WHEN analyzing video for motion, THE Action_Selector SHALL use FFmpeg's scene detection filter (scdet) or motion vector analysis
2. WHEN executing FFmpeg for motion analysis, THE Action_Selector SHALL limit analysis duration to 5 minutes for videos longer than 5 minutes
3. WHEN parsing FFmpeg output, THE Action_Selector SHALL extract motion scores and timestamps for each analyzed segment
4. WHEN FFmpeg analysis fails, THE Action_Selector SHALL return an appropriate error that allows fallback behavior
5. WHEN a video has no detectable motion, THE Action_Selector SHALL treat this as a valid result and fall back to middle segment selection

### Requirement 3: Exclusion Zone Compliance

**User Story:** As a user, I want the action-based strategy to respect my configured intro and outro exclusion zones, so that clips avoid opening credits and end credits.

#### Acceptance Criteria

1. WHEN selecting an action-based clip, THE Action_Selector SHALL only consider segments where the entire clip (start to end) falls between the intro exclusion boundary and outro exclusion boundary
2. WHEN a high-action segment starts before the intro exclusion boundary, THE Action_Selector SHALL exclude that segment from consideration
3. WHEN a high-action segment ends after the outro exclusion boundary, THE Action_Selector SHALL exclude that segment from consideration
4. WHEN the highest-action segment violates exclusion zones, THE Action_Selector SHALL select the next highest-action segment that fully respects exclusion zones
5. WHEN all high-action segments violate exclusion zones, THE Action_Selector SHALL fall back to middle segment selection
6. WHEN exclusion zones are set to 0%, THE Action_Selector SHALL consider the entire video duration for selection

### Requirement 4: Clip Duration Constraints

**User Story:** As a user, I want action-based clips to maintain the standard duration constraints, so that all clips are consistently sized regardless of selection strategy.

#### Acceptance Criteria

1. WHEN generating an action-based clip, THE Action_Selector SHALL produce clips between 12 and 18 seconds in duration
2. WHEN a high-action segment is shorter than 12 seconds, THE Action_Selector SHALL extend the clip duration to meet the minimum
3. WHEN a high-action segment is longer than 18 seconds, THE Action_Selector SHALL cap the clip duration at 18 seconds
4. WHEN extending or capping duration, THE Action_Selector SHALL ensure the clip remains within video boundaries
5. WHEN the video is shorter than the minimum clip duration, THE Action_Selector SHALL use the full video duration

### Requirement 5: CLI Integration

**User Story:** As a user, I want to specify the action-based strategy via command-line arguments, so that I can easily switch between selection strategies.

#### Acceptance Criteria

1. WHEN the user provides "--strategy action" or "-s action", THE CLI SHALL select the action-based strategy
2. WHEN the user provides "--strategy intense-action" or "-s intense-action", THE CLI SHALL select the action-based strategy
3. WHEN no strategy is specified, THE CLI SHALL default to random strategy (existing behavior)
4. WHEN an invalid strategy is specified, THE CLI SHALL return an error with available options
5. THE CLI help text SHALL include documentation for the action/intense-action strategy option

### Requirement 6: ClipSelector Trait Implementation

**User Story:** As a developer, I want the action-based selector to implement the ClipSelector trait, so that it integrates seamlessly with the existing architecture.

#### Acceptance Criteria

1. THE Action_Selector SHALL implement the ClipSelector trait
2. WHEN select_segment is called, THE Action_Selector SHALL return a TimeRange with valid start time and duration
3. WHEN select_segment encounters an error, THE Action_Selector SHALL return a SelectionError
4. THE Action_Selector SHALL accept intro_exclusion_percent and outro_exclusion_percent parameters
5. THE Action_Selector SHALL use the video_path parameter to perform motion analysis

### Requirement 7: Fallback Behavior

**User Story:** As a user, I want the tool to handle edge cases gracefully, so that clip extraction succeeds even when motion analysis is not possible.

#### Acceptance Criteria

1. WHEN motion analysis fails due to FFmpeg errors, THE Action_Selector SHALL fall back to middle segment selection
2. WHEN a video has no detectable motion or scene changes, THE Action_Selector SHALL fall back to middle segment selection
3. WHEN a video is too short for meaningful motion analysis, THE Action_Selector SHALL fall back to middle segment selection
4. WHEN falling back to middle segment, THE Action_Selector SHALL log the reason for fallback
5. THE middle segment fallback SHALL use the same logic as IntenseAudioSelector's fallback (centered clip with appropriate duration)

### Requirement 8: Performance Optimization

**User Story:** As a user, I want motion analysis to complete in reasonable time, so that processing large video libraries remains practical.

#### Acceptance Criteria

1. WHEN analyzing videos longer than 5 minutes, THE Action_Selector SHALL limit analysis to the first 5 minutes
2. WHEN analyzing limited duration, THE Action_Selector SHALL scale segment timestamps to the full video duration
3. WHEN analyzing motion, THE Action_Selector SHALL use efficient FFmpeg filters that minimize processing time
4. WHEN multiple videos are processed, THE Action_Selector SHALL not cache analysis results between videos
5. THE motion analysis SHALL complete within a reasonable time proportional to the analyzed duration

### Requirement 9: Error Handling

**User Story:** As a developer, I want clear error messages for motion analysis failures, so that I can diagnose and fix issues effectively.

#### Acceptance Criteria

1. WHEN FFmpeg is not available, THE Action_Selector SHALL return a SelectionError indicating FFmpeg is required
2. WHEN FFmpeg motion analysis fails, THE Action_Selector SHALL capture stderr output for debugging
3. WHEN parsing motion analysis output fails, THE Action_Selector SHALL return a SelectionError with details
4. WHEN a video file is corrupted or unreadable, THE Action_Selector SHALL return an appropriate error
5. ALL SelectionError messages SHALL be descriptive and actionable

### Requirement 10: Motion Scoring Algorithm

**User Story:** As a developer, I want a clear algorithm for scoring motion intensity, so that the most action-packed segments are reliably identified.

#### Acceptance Criteria

1. WHEN using scene detection, THE Action_Selector SHALL calculate motion scores based on scene change frequency and magnitude
2. WHEN using motion vectors, THE Action_Selector SHALL calculate motion scores based on average motion magnitude per frame
3. WHEN grouping frames into segments, THE Action_Selector SHALL use 12.5-second segment windows (matching audio analysis)
4. WHEN calculating segment scores, THE Action_Selector SHALL aggregate frame-level motion data into segment-level scores
5. THE motion scoring algorithm SHALL produce higher scores for segments with more visual activity
