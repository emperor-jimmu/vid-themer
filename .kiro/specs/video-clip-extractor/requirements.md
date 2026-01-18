# Requirements Document

## Introduction

A command-line tool that recursively scans directories for video files and automatically extracts short thematic clips from each video. The tool provides configurable clip selection strategies to choose the most representative segment from each video, placing the extracted clips in organized subdirectories for easy access.

## Glossary

- **CLI_Tool**: The command-line application that orchestrates video scanning and clip extraction
- **Video_Scanner**: The component responsible for recursively discovering video files in a directory tree
- **Clip_Extractor**: The component that extracts video segments using FFmpeg
- **Selection_Strategy**: The algorithm used to determine which segment of a video to extract (Random or Intense_Audio)
- **Source_Video**: An mp4 or mkv video file found during directory scanning
- **Theme_Clip**: The extracted 8-12 second video segment saved as "backdrop.mp4"
- **Backdrops_Folder**: The subdirectory named "backdrops" where theme clips are stored
- **FFmpeg**: External video processing tool used for clip extraction and audio analysis

## Requirements

### Requirement 1: Directory Scanning

**User Story:** As a user, I want to scan a directory recursively for video files, so that I can process all videos in a folder hierarchy without manual enumeration.

#### Acceptance Criteria

1. WHEN a user provides a valid directory path, THE Video_Scanner SHALL recursively traverse all subdirectories
2. WHEN the Video_Scanner encounters a file with .mp4 extension, THE Video_Scanner SHALL add it to the processing list
3. WHEN the Video_Scanner encounters a file with .mkv extension, THE Video_Scanner SHALL add it to the processing list
4. WHEN the Video_Scanner encounters a directory containing a "backdrops/backdrop.mp4" file, THE Video_Scanner SHALL skip all videos in that directory
5. WHEN the Video_Scanner encounters a non-video file, THE Video_Scanner SHALL skip it without error
6. IF a directory is not accessible due to permissions, THEN THE Video_Scanner SHALL log a warning and continue scanning other directories

### Requirement 2: Clip Extraction

**User Story:** As a user, I want to extract a short clip from each video, so that I can create preview thumbnails or theme videos for my media library.

#### Acceptance Criteria

1. WHEN a Source_Video is processed, THE Clip_Extractor SHALL extract a segment between 8 and 12 seconds in duration
2. WHEN extracting a clip, THE Clip_Extractor SHALL invoke FFmpeg with appropriate parameters
3. WHEN the extraction completes successfully, THE Clip_Extractor SHALL save the output as "backdrop.mp4"
4. WHEN the Source_Video duration is less than 8 seconds, THE Clip_Extractor SHALL extract the entire video
5. WHEN the output resolution is set to 1080p, THE Clip_Extractor SHALL scale the output to 1920x1080 only if the source resolution is higher
6. WHEN the output resolution is set to 720p, THE Clip_Extractor SHALL scale the output to 1280x720 only if the source resolution is higher
7. WHEN the Source_Video resolution is lower than the target resolution, THE Clip_Extractor SHALL keep the original resolution without upscaling
8. WHEN scaling video, THE Clip_Extractor SHALL maintain aspect ratio by adding letterboxing if necessary
9. WHEN the audio inclusion flag is set to false, THE Clip_Extractor SHALL create a video-only output without audio tracks
10. WHEN the audio inclusion flag is set to true, THE Clip_Extractor SHALL preserve the audio track from the source segment
11. IF FFmpeg is not available in the system PATH, THEN THE CLI_Tool SHALL return an error message indicating FFmpeg is required

### Requirement 3: Selection Strategy - Random

**User Story:** As a user, I want to extract a random segment from videos, so that I can quickly generate diverse preview clips without analysis overhead.

#### Acceptance Criteria

1. WHEN the Random selection strategy is active, THE Clip_Extractor SHALL select a random start time within valid bounds
2. WHEN calculating the random start time, THE Clip_Extractor SHALL ensure the selected segment fits within the video duration
3. WHEN calculating the random start time, THE Clip_Extractor SHALL exclude the first 1 minute of the video to avoid opening titles
4. WHEN calculating the random start time, THE Clip_Extractor SHALL exclude the last 4 minutes of the video to avoid ending credits
5. WHEN the video duration is less than the sum of excluded regions plus clip duration, THE Clip_Extractor SHALL select the middle segment
6. THE Clip_Extractor SHALL use a different random seed for each video to ensure variety

### Requirement 4: Selection Strategy - Intense Audio

**User Story:** As a user, I want to extract the segment with the most intense audio, so that I can capture the most engaging or action-packed moments from videos.

#### Acceptance Criteria

1. WHEN the Intense_Audio selection strategy is active, THE Clip_Extractor SHALL analyze the audio track of the Source_Video
2. WHEN analyzing audio, THE Clip_Extractor SHALL use FFmpeg to calculate audio volume levels across the video duration
3. WHEN multiple segments have similar audio intensity, THE Clip_Extractor SHALL select the first occurrence
4. WHEN the Source_Video has no audio track, THE Clip_Extractor SHALL fall back to selecting the middle segment
5. THE Clip_Extractor SHALL analyze audio in chunks to identify the loudest continuous segment

### Requirement 5: Output Organization

**User Story:** As a user, I want extracted clips organized in backdrops subfolders, so that I can easily locate theme clips relative to their source videos.

#### Acceptance Criteria

1. WHEN a Theme_Clip is created, THE CLI_Tool SHALL place it in a Backdrops_Folder relative to the Source_Video location
2. WHEN the Backdrops_Folder does not exist, THE CLI_Tool SHALL create it
3. WHEN a "backdrop.mp4" file already exists in the Backdrops_Folder, THE CLI_Tool SHALL overwrite it
4. THE Backdrops_Folder SHALL be named "backdrops" in lowercase
5. THE Theme_Clip SHALL be named "backdrop.mp4" in lowercase

### Requirement 6: Command-Line Interface

**User Story:** As a user, I want to configure the tool via command-line arguments, so that I can customize behavior without modifying code.

#### Acceptance Criteria

1. THE CLI_Tool SHALL accept a required positional argument for the directory path to scan
2. THE CLI_Tool SHALL accept an optional flag to specify the selection strategy (random or intense-audio)
3. WHEN no selection strategy is specified, THE CLI_Tool SHALL default to the Random strategy
4. THE CLI_Tool SHALL accept an optional flag to specify output resolution (720p or 1080p)
5. WHEN no output resolution is specified, THE CLI_Tool SHALL default to 1080p
6. THE CLI_Tool SHALL accept an optional flag to include or exclude audio in the output clip
7. WHEN no audio preference is specified, THE CLI_Tool SHALL default to including audio
8. WHEN invalid arguments are provided, THE CLI_Tool SHALL display usage information and exit with a non-zero status code
9. THE CLI_Tool SHALL provide a --help flag that displays usage information

### Requirement 7: Error Handling

**User Story:** As a user, I want clear error messages when problems occur, so that I can understand and resolve issues quickly.

#### Acceptance Criteria

1. IF the provided directory path does not exist, THEN THE CLI_Tool SHALL display an error message and exit with status code 1
2. IF a Source_Video file is corrupted or unreadable, THEN THE CLI_Tool SHALL log an error for that file and continue processing other videos
3. IF FFmpeg execution fails for a video, THEN THE CLI_Tool SHALL log the FFmpeg error output and continue processing other videos
4. IF disk space is insufficient to write a Theme_Clip, THEN THE CLI_Tool SHALL log an error and continue processing other videos
5. WHEN any error occurs, THE CLI_Tool SHALL include the file path in the error message

### Requirement 8: Progress Indication

**User Story:** As a user, I want to see progress while the tool runs, so that I know the tool is working and can estimate completion time.

#### Acceptance Criteria

1. WHEN processing begins, THE CLI_Tool SHALL display the total number of videos found
2. WHEN each video is processed, THE CLI_Tool SHALL display a progress indicator showing current and total count
3. WHEN a video is successfully processed, THE CLI_Tool SHALL display the output path of the Theme_Clip
4. WHEN processing completes, THE CLI_Tool SHALL display a summary of successful and failed extractions
5. THE CLI_Tool SHALL display progress updates in real-time without requiring buffering

### Requirement 9: Video Duration Detection

**User Story:** As a developer, I want to detect video duration accurately, so that the clip extraction logic can make informed decisions about segment selection.

#### Acceptance Criteria

1. WHEN a Source_Video is encountered, THE Clip_Extractor SHALL query its duration using FFmpeg
2. WHEN FFmpeg returns duration information, THE Clip_Extractor SHALL parse it into a numeric value in seconds
3. IF duration detection fails, THEN THE Clip_Extractor SHALL log an error and skip that video
4. THE Clip_Extractor SHALL handle videos with fractional second durations correctly
