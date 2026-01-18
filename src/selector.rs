// Clip selection strategies (trait + implementations)

use std::path::Path;
use rand::Rng;

pub struct TimeRange {
    pub start_seconds: f64,
    pub duration_seconds: f64,
}

pub trait ClipSelector {
    fn select_segment(
        &self,
        video_path: &Path,
        duration: f64,
    ) -> Result<TimeRange, SelectionError>;
}

pub struct RandomSelector;

impl ClipSelector for RandomSelector {
    fn select_segment(
        &self,
        _video_path: &Path,
        duration: f64,
    ) -> Result<TimeRange, SelectionError> {
        const INTRO_EXCLUSION: f64 = 60.0;  // Exclude first 60 seconds
        const OUTRO_EXCLUSION: f64 = 240.0; // Exclude last 240 seconds
        const MIN_CLIP_DURATION: f64 = 8.0;
        const MAX_CLIP_DURATION: f64 = 12.0;
        
        // Generate random clip duration between 8 and 12 seconds
        let mut rng = rand::thread_rng();
        let clip_duration = rng.gen_range(MIN_CLIP_DURATION..=MAX_CLIP_DURATION);
        
        // Calculate the minimum required duration for exclusions
        let required_duration = INTRO_EXCLUSION + clip_duration + OUTRO_EXCLUSION;
        
        // Check if video is too short for exclusions
        if duration < required_duration {
            // Fall back to middle segment
            let start = (duration - clip_duration).max(0.0) / 2.0;
            let actual_duration = clip_duration.min(duration);
            
            return Ok(TimeRange {
                start_seconds: start,
                duration_seconds: actual_duration,
            });
        }
        
        // Calculate valid range for random selection
        let earliest_start = INTRO_EXCLUSION;
        let latest_start = duration - OUTRO_EXCLUSION - clip_duration;
        
        // Generate random start time within valid bounds
        let start = rng.gen_range(earliest_start..=latest_start);
        
        Ok(TimeRange {
            start_seconds: start,
            duration_seconds: clip_duration,
        })
    }
}

pub struct IntenseAudioSelector {
    ffmpeg_executor: crate::ffmpeg::FFmpegExecutor,
}

impl IntenseAudioSelector {
    pub fn new(ffmpeg_executor: crate::ffmpeg::FFmpegExecutor) -> Self {
        Self { ffmpeg_executor }
    }
}

impl ClipSelector for IntenseAudioSelector {
    fn select_segment(
        &self,
        video_path: &Path,
        duration: f64,
    ) -> Result<TimeRange, SelectionError> {
        const MIN_CLIP_DURATION: f64 = 8.0;
        const MAX_CLIP_DURATION: f64 = 12.0;
        
        // Try to analyze audio intensity
        match self.ffmpeg_executor.analyze_audio_intensity(video_path, duration) {
            Ok(segments) => {
                // Select the segment with highest audio intensity (first in sorted list)
                if let Some(loudest_segment) = segments.first() {
                    // Use the segment's duration, but cap it between 8-12 seconds
                    let clip_duration = loudest_segment.duration
                        .max(MIN_CLIP_DURATION)
                        .min(MAX_CLIP_DURATION)
                        .min(duration); // Don't exceed video duration
                    
                    // Ensure the clip fits within the video
                    let start = loudest_segment.start_time;
                    let end = start + clip_duration;
                    
                    if end <= duration {
                        return Ok(TimeRange {
                            start_seconds: start,
                            duration_seconds: clip_duration,
                        });
                    } else {
                        // Adjust start time to fit the clip within video duration
                        let adjusted_start = (duration - clip_duration).max(0.0);
                        return Ok(TimeRange {
                            start_seconds: adjusted_start,
                            duration_seconds: clip_duration,
                        });
                    }
                }
                
                // No segments found, fall back to middle segment
                Self::middle_segment(duration)
            }
            Err(crate::ffmpeg::FFmpegError::NoAudioTrack) => {
                // No audio track, fall back to middle segment
                Self::middle_segment(duration)
            }
            Err(e) => {
                // Other errors, return as SelectionError
                Err(SelectionError::AudioAnalysisFailed(e.to_string()))
            }
        }
    }
}

impl IntenseAudioSelector {
    /// Calculate middle segment as fallback when audio analysis fails or no audio track exists
    fn middle_segment(duration: f64) -> Result<TimeRange, SelectionError> {
        const MIN_CLIP_DURATION: f64 = 8.0;
        const MAX_CLIP_DURATION: f64 = 12.0;
        
        // Use 10 seconds as default clip duration (middle of 8-12 range)
        let clip_duration = MAX_CLIP_DURATION.min(duration);
        let actual_duration = clip_duration.max(MIN_CLIP_DURATION).min(duration);
        
        // Calculate start time to center the clip
        let start = ((duration - actual_duration) / 2.0).max(0.0);
        
        Ok(TimeRange {
            start_seconds: start,
            duration_seconds: actual_duration,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SelectionError {
    #[error("Video too short: {0}s")]
    #[allow(dead_code)]
    VideoTooShort(f64),
    
    #[error("Failed to analyze audio: {0}")]
    AudioAnalysisFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use proptest::prelude::*;

    // Feature: video-clip-extractor, Property 10: Random Selection Valid Bounds
    proptest! {
        #[test]
        fn test_random_selection_valid_bounds(duration in 320.0..3600.0f64) {
            // Test that for videos long enough to accommodate exclusions,
            // the selected segment respects the intro and outro exclusion zones
            
            const INTRO_EXCLUSION: f64 = 60.0;
            const OUTRO_EXCLUSION: f64 = 240.0;
            const MIN_CLIP_DURATION: f64 = 8.0;
            const MAX_CLIP_DURATION: f64 = 12.0;
            
            let selector = RandomSelector;
            let video_path = PathBuf::from("test.mp4");
            
            // Only test videos that are long enough for exclusions
            // Required: INTRO_EXCLUSION + MIN_CLIP_DURATION + OUTRO_EXCLUSION = 308 seconds
            // We test from 320 seconds to give some margin
            
            let result = selector.select_segment(&video_path, duration);
            prop_assert!(result.is_ok(), "Selection should succeed for valid duration");
            
            let time_range = result.unwrap();
            
            // Property 1: Clip duration should be between 8 and 12 seconds
            prop_assert!(time_range.duration_seconds >= MIN_CLIP_DURATION,
                "Clip duration {} should be >= {}", time_range.duration_seconds, MIN_CLIP_DURATION);
            prop_assert!(time_range.duration_seconds <= MAX_CLIP_DURATION,
                "Clip duration {} should be <= {}", time_range.duration_seconds, MAX_CLIP_DURATION);
            
            // Property 2: Start time should be at least 60 seconds from beginning
            prop_assert!(time_range.start_seconds >= INTRO_EXCLUSION,
                "Start time {} should be >= {} (intro exclusion)", 
                time_range.start_seconds, INTRO_EXCLUSION);
            
            // Property 3: End time should be at least 240 seconds before video end
            let end_time = time_range.start_seconds + time_range.duration_seconds;
            prop_assert!(end_time <= duration - OUTRO_EXCLUSION,
                "End time {} should be <= {} (duration {} - outro exclusion {})",
                end_time, duration - OUTRO_EXCLUSION, duration, OUTRO_EXCLUSION);
            
            // Property 4: The selected segment should fit within video duration
            prop_assert!(end_time <= duration,
                "End time {} should not exceed video duration {}", end_time, duration);
        }
    }

    #[test]
    fn test_random_selector_long_video() {
        let selector = RandomSelector;
        let video_path = PathBuf::from("test.mp4");
        let duration = 600.0; // 10 minutes
        
        let result = selector.select_segment(&video_path, duration);
        assert!(result.is_ok());
        
        let time_range = result.unwrap();
        
        // Verify clip duration is between 8 and 12 seconds
        assert!(time_range.duration_seconds >= 8.0);
        assert!(time_range.duration_seconds <= 12.0);
        
        // Verify start time respects exclusion zones
        assert!(time_range.start_seconds >= 60.0); // After intro exclusion
        assert!(time_range.start_seconds + time_range.duration_seconds <= duration - 240.0); // Before outro exclusion
    }

    #[test]
    fn test_random_selector_short_video_fallback() {
        let selector = RandomSelector;
        let video_path = PathBuf::from("test.mp4");
        let duration = 100.0; // 100 seconds - too short for exclusions
        
        let result = selector.select_segment(&video_path, duration);
        assert!(result.is_ok());
        
        let time_range = result.unwrap();
        
        // Verify clip duration is between 8 and 12 seconds
        assert!(time_range.duration_seconds >= 8.0);
        assert!(time_range.duration_seconds <= 12.0);
        
        // Verify it uses middle segment (start should be roughly in the middle)
        let expected_middle = (duration - time_range.duration_seconds) / 2.0;
        assert!((time_range.start_seconds - expected_middle).abs() < 0.1);
    }

    #[test]
    fn test_random_selector_very_short_video() {
        let selector = RandomSelector;
        let video_path = PathBuf::from("test.mp4");
        let duration = 3.0; // 3 seconds - shorter than minimum clip duration
        
        let result = selector.select_segment(&video_path, duration);
        assert!(result.is_ok());
        
        let time_range = result.unwrap();
        
        // Verify clip duration is capped at video duration
        assert_eq!(time_range.duration_seconds, duration);
        assert_eq!(time_range.start_seconds, 0.0);
    }

    #[test]
    fn test_random_selector_variety() {
        let selector = RandomSelector;
        let video_path = PathBuf::from("test.mp4");
        let duration = 600.0; // 10 minutes
        
        // Run multiple times and collect start times
        let mut start_times = Vec::new();
        for _ in 0..10 {
            let result = selector.select_segment(&video_path, duration);
            assert!(result.is_ok());
            start_times.push(result.unwrap().start_seconds);
        }
        
        // Verify that not all start times are identical (variety check)
        let first = start_times[0];
        let all_same = start_times.iter().all(|&x| (x - first).abs() < 0.1);
        assert!(!all_same, "Random selector should produce variety in start times");
    }

    // Feature: video-clip-extractor, Property 11: Random Selection Variety
    proptest! {
        #[test]
        fn test_random_selection_variety_property(duration in 315.0..3600.0f64) {
            // Test that for any video long enough to accommodate exclusions,
            // multiple selections produce different start times (variety)
            
            let selector = RandomSelector;
            let video_path = PathBuf::from("test.mp4");
            
            // Run selection multiple times (10 iterations)
            let mut start_times = Vec::new();
            for _ in 0..10 {
                let result = selector.select_segment(&video_path, duration);
                prop_assert!(result.is_ok(), "Selection should succeed");
                start_times.push(result.unwrap().start_seconds);
            }
            
            // Property: Not all start times should be identical
            // This verifies that the random selector uses different random seeds
            let first = start_times[0];
            let all_same = start_times.iter().all(|&x| (x - first).abs() < 0.1);
            
            prop_assert!(!all_same, 
                "Random selector should produce variety in start times across multiple runs. \
                 All {} selections produced start time {}", 
                start_times.len(), first);
        }
    }

    // Tests for IntenseAudioSelector

    #[test]
    fn test_intense_audio_selector_middle_segment_fallback() {
        // Test that middle_segment helper calculates correct fallback
        let duration = 600.0; // 10 minutes
        
        let result = IntenseAudioSelector::middle_segment(duration);
        assert!(result.is_ok());
        
        let time_range = result.unwrap();
        
        // Should use 12 seconds (max clip duration)
        assert_eq!(time_range.duration_seconds, 12.0);
        
        // Should be centered: (600 - 12) / 2 = 294
        assert_eq!(time_range.start_seconds, 294.0);
    }

    #[test]
    fn test_intense_audio_selector_middle_segment_short_video() {
        // Test middle_segment with a short video
        let duration = 7.0; // 7 seconds
        
        let result = IntenseAudioSelector::middle_segment(duration);
        assert!(result.is_ok());
        
        let time_range = result.unwrap();
        
        // Should use full video duration (7 seconds)
        assert_eq!(time_range.duration_seconds, 7.0);
        
        // Should start at 0 (centered)
        assert_eq!(time_range.start_seconds, 0.0);
    }

    #[test]
    fn test_intense_audio_selector_middle_segment_very_short_video() {
        // Test middle_segment with a very short video (< 5 seconds)
        let duration = 3.0; // 3 seconds
        
        let result = IntenseAudioSelector::middle_segment(duration);
        assert!(result.is_ok());
        
        let time_range = result.unwrap();
        
        // Should use full video duration
        assert_eq!(time_range.duration_seconds, 3.0);
        
        // Should start at 0
        assert_eq!(time_range.start_seconds, 0.0);
    }

    #[test]
    fn test_intense_audio_selector_no_audio_fallback() {
        // Test fallback to middle segment when video has no audio
        // Validates Requirement 4.4
        use crate::cli::Resolution;
        use std::path::PathBuf;

        // Create an FFmpegExecutor
        let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true);
        
        // Create IntenseAudioSelector
        let selector = IntenseAudioSelector::new(ffmpeg_executor);
        
        // Use a non-existent video path - this will cause audio analysis to fail
        // which simulates a video with no audio track
        let video_path = PathBuf::from("/nonexistent/video_no_audio.mp4");
        let duration = 600.0; // 10 minutes
        
        // The selector should fall back to middle segment when audio analysis fails
        let result = selector.select_segment(&video_path, duration);
        
        // The result should be Ok (fallback to middle segment)
        assert!(result.is_ok(), "Should fall back to middle segment when no audio track");
        
        let time_range = result.unwrap();
        
        // Verify it uses middle segment calculation
        // For a 600 second video, with 12 second clip duration:
        // start = (600 - 12) / 2 = 294
        assert_eq!(time_range.duration_seconds, 12.0, "Should use max clip duration (12s)");
        assert_eq!(time_range.start_seconds, 294.0, "Should center the clip in the video");
        
        // Test with a shorter video
        let short_duration = 120.0; // 2 minutes
        let result_short = selector.select_segment(&video_path, short_duration);
        
        assert!(result_short.is_ok(), "Should fall back to middle segment for short video");
        
        let time_range_short = result_short.unwrap();
        
        // For a 120 second video, with 12 second clip:
        // start = (120 - 12) / 2 = 54
        assert_eq!(time_range_short.duration_seconds, 12.0);
        assert_eq!(time_range_short.start_seconds, 54.0);
        
        // Test with a very short video (< 12 seconds)
        let very_short_duration = 7.0; // 7 seconds
        let result_very_short = selector.select_segment(&video_path, very_short_duration);
        
        assert!(result_very_short.is_ok(), "Should fall back to middle segment for very short video");
        
        let time_range_very_short = result_very_short.unwrap();
        
        // For a 7 second video, clip duration should be capped at 7 seconds
        // start = (7 - 7) / 2 = 0
        assert_eq!(time_range_very_short.duration_seconds, 7.0, "Should use full video duration");
        assert_eq!(time_range_very_short.start_seconds, 0.0, "Should start at beginning");
    }

    #[test]
    fn test_intense_audio_selector_tie_breaking() {
        // Test that first occurrence is selected when multiple segments have similar intensity
        // Validates Requirement 4.3
        use crate::ffmpeg::AudioSegment;
        
        // Create a mock scenario where we have multiple segments with similar intensity
        // We'll test the sorting behavior directly since we can't easily mock FFmpeg output
        
        // Create segments with similar intensities (within 0.1 dBFS of each other)
        // In dBFS, higher (less negative) values are louder
        let mut segments = vec![
            AudioSegment {
                start_time: 30.0,
                duration: 7.5,
                intensity: -15.2, // Second loudest (but appears second)
            },
            AudioSegment {
                start_time: 10.0,
                duration: 7.5,
                intensity: -15.1, // Loudest (appears first)
            },
            AudioSegment {
                start_time: 50.0,
                duration: 7.5,
                intensity: -15.3, // Third loudest
            },
            AudioSegment {
                start_time: 70.0,
                duration: 7.5,
                intensity: -15.1, // Tied for loudest (but appears later)
            },
        ];
        
        // Sort segments by intensity (highest/loudest first)
        // This mimics what analyze_audio_intensity does
        segments.sort_by(|a, b| b.intensity.partial_cmp(&a.intensity).unwrap());
        
        // After sorting, the segments with intensity -15.1 should be first
        // But we need to verify which one comes first when there's a tie
        
        // The first segment should be one with intensity -15.1
        assert_eq!(segments[0].intensity, -15.1, "First segment should have highest intensity");
        
        // When there's a tie in intensity, Rust's stable sort preserves the original order
        // So the segment that appeared first in the original list should remain first
        assert_eq!(segments[0].start_time, 10.0, 
            "When multiple segments have the same intensity, the first occurrence should be selected");
        
        // The second segment with -15.1 intensity should come after
        // Find the second occurrence of -15.1 intensity
        let second_loudest_index = segments.iter().position(|s| s.intensity == -15.1 && s.start_time == 70.0);
        assert!(second_loudest_index.is_some(), "Second segment with -15.1 intensity should exist");
        assert!(second_loudest_index.unwrap() > 0, "Second tied segment should come after the first");
        
        // Test with exact tie scenario (all segments have identical intensity)
        let mut tied_segments = vec![
            AudioSegment {
                start_time: 100.0,
                duration: 7.5,
                intensity: -20.0,
            },
            AudioSegment {
                start_time: 50.0,
                duration: 7.5,
                intensity: -20.0,
            },
            AudioSegment {
                start_time: 25.0,
                duration: 7.5,
                intensity: -20.0,
            },
        ];
        
        // Sort by intensity
        tied_segments.sort_by(|a, b| b.intensity.partial_cmp(&a.intensity).unwrap());
        
        // With stable sort, the original order should be preserved
        // So the first segment (start_time: 100.0) should remain first
        assert_eq!(tied_segments[0].start_time, 100.0,
            "With identical intensities, the first occurrence in the original list should be selected");
        assert_eq!(tied_segments[1].start_time, 50.0,
            "Second segment should maintain original order");
        assert_eq!(tied_segments[2].start_time, 25.0,
            "Third segment should maintain original order");
    }
}

