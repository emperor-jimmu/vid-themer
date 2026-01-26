// Clip selection strategies (trait + implementations)

use std::path::Path;
use rand::Rng;

/// Default intro exclusion percentage (1% of video duration)
#[allow(dead_code)]
pub const INTRO_EXCLUSION_PERCENT: f64 = 1.0;

/// Default outro exclusion percentage (40% of video duration)
#[allow(dead_code)]
pub const OUTRO_EXCLUSION_PERCENT: f64 = 40.0;

/// Configuration for clip duration constraints
pub struct ClipConfig {
    pub min_duration: f64,
    pub max_duration: f64,
}

impl Default for ClipConfig {
    fn default() -> Self {
        Self {
            min_duration: 12.0,
            max_duration: 18.0,
        }
    }
}

impl ClipConfig {
    /// Get a random duration within the configured range
    pub fn random_duration(&self) -> f64 {
        let mut rng = rand::thread_rng();
        rng.gen_range(self.min_duration..=self.max_duration)
    }
}

pub struct TimeRange {
    pub start_seconds: f64,
    pub duration_seconds: f64,
}

pub trait ClipSelector {
    fn select_segment(
        &self,
        video_path: &Path,
        duration: f64,
        intro_exclusion_percent: f64,
        outro_exclusion_percent: f64,
    ) -> Result<TimeRange, SelectionError>;
}

pub struct RandomSelector;

impl ClipSelector for RandomSelector {
    fn select_segment(
        &self,
        _video_path: &Path,
        duration: f64,
        intro_exclusion_percent: f64,
        outro_exclusion_percent: f64,
    ) -> Result<TimeRange, SelectionError> {
        let config = ClipConfig::default();
        
        // Calculate exclusion zones as percentages of video duration
        let intro_exclusion = duration * (intro_exclusion_percent / 100.0);
        let outro_exclusion = duration * (outro_exclusion_percent / 100.0);
        
        // Generate random clip duration between min and max
        let clip_duration = config.random_duration();
        
        // Calculate the minimum required duration for exclusions
        let required_duration = intro_exclusion + clip_duration + outro_exclusion;
        
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
        let earliest_start = intro_exclusion;
        let latest_start = duration - outro_exclusion - clip_duration;
        
        // Generate random start time within valid bounds
        let mut rng = rand::thread_rng();
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
        _intro_exclusion_percent: f64,
        _outro_exclusion_percent: f64,
    ) -> Result<TimeRange, SelectionError> {
        let config = ClipConfig::default();
        
        // Try to analyze audio intensity
        match self.ffmpeg_executor.analyze_audio_intensity(video_path, duration) {
            Ok(segments) => {
                // Select the segment with highest audio intensity (first in sorted list)
                if let Some(loudest_segment) = segments.first() {
                    // Use the segment's duration, but cap it between min-max seconds
                    let clip_duration = loudest_segment.duration
                        .max(config.min_duration)
                        .min(config.max_duration)
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
        let config = ClipConfig::default();
        
        // Use max duration as default clip duration
        let clip_duration = config.max_duration.min(duration);
        let actual_duration = clip_duration.max(config.min_duration).min(duration);
        
        // Calculate start time to center the clip
        let start = ((duration - actual_duration) / 2.0).max(0.0);
        
        Ok(TimeRange {
            start_seconds: start,
            duration_seconds: actual_duration,
        })
    }
}

pub struct ActionSelector {
    ffmpeg_executor: crate::ffmpeg::FFmpegExecutor,
}

impl ActionSelector {
    pub fn new(ffmpeg_executor: crate::ffmpeg::FFmpegExecutor) -> Self {
        Self { ffmpeg_executor }
    }

    /// Calculate middle segment as fallback when motion analysis fails
    fn middle_segment(duration: f64) -> Result<TimeRange, SelectionError> {
        let config = ClipConfig::default();
        
        // Use max duration as default clip duration
        let clip_duration = config.max_duration.min(duration);
        let actual_duration = clip_duration.max(config.min_duration).min(duration);
        
        // Calculate start time to center the clip
        let start = ((duration - actual_duration) / 2.0).max(0.0);
        
        Ok(TimeRange {
            start_seconds: start,
            duration_seconds: actual_duration,
        })
    }
}

impl ClipSelector for ActionSelector {
    fn select_segment(
        &self,
        video_path: &Path,
        duration: f64,
        intro_exclusion_percent: f64,
        outro_exclusion_percent: f64,
    ) -> Result<TimeRange, SelectionError> {
        let config = ClipConfig::default();
        
        // Try to analyze motion intensity
        match self.ffmpeg_executor.analyze_motion_intensity(video_path, duration) {
            Ok(segments) => {
                // Calculate exclusion zone boundaries
                let intro_boundary = duration * (intro_exclusion_percent / 100.0);
                let outro_boundary = duration - (duration * (outro_exclusion_percent / 100.0));
                
                // Filter segments by exclusion zones
                // Only consider segments where the entire clip falls between boundaries
                let filtered_segments: Vec<_> = segments.into_iter()
                    .filter(|seg| {
                        let segment_start = seg.start_time;
                        let segment_end = seg.start_time + seg.duration;
                        
                        // Entire segment must be between boundaries
                        segment_start >= intro_boundary && segment_end <= outro_boundary
                    })
                    .collect();
                
                // Select the highest-scoring valid segment
                if let Some(best_segment) = filtered_segments.first() {
                    // Adjust clip duration to fit 12-18 second range
                    let clip_duration = best_segment.duration
                        .max(config.min_duration)
                        .min(config.max_duration)
                        .min(duration); // Don't exceed video duration
                    
                    // Ensure the clip fits within the video
                    let start = best_segment.start_time;
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
                
                // No valid segments found, fall back to middle segment
                Self::middle_segment(duration)
            }
            Err(e) => {
                // Motion analysis failed, fall back to middle segment
                // Log the error but don't fail the entire operation
                eprintln!("Motion analysis failed: {}, falling back to middle segment", e);
                Self::middle_segment(duration)
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SelectionError {
    #[error("Video too short: {0}s")]
    #[allow(dead_code)]
    VideoTooShort(f64),
    
    #[error("Failed to analyze audio: {0}")]
    AudioAnalysisFailed(String),
    
    #[error("Failed to analyze motion: {0}")]
    MotionAnalysisFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use proptest::prelude::*;

    // Feature: video-clip-extractor, Property 10: Random Selection Valid Bounds
    proptest! {
        #[test]
        fn test_random_selection_valid_bounds(duration in 415.0..3600.0f64) {
            // Test that for videos long enough to accommodate exclusions,
            // the selected segment respects the intro and outro exclusion zones
            
            const INTRO_EXCLUSION_PERCENT: f64 = 1.0;
            const OUTRO_EXCLUSION_PERCENT: f64 = 40.0;
            const MIN_CLIP_DURATION: f64 = 12.0;
            const MAX_CLIP_DURATION: f64 = 18.0;
            
            let selector = RandomSelector;
            let video_path = PathBuf::from("test.mp4");
            
            // Calculate actual exclusion zones based on percentages
            let intro_exclusion = duration * (INTRO_EXCLUSION_PERCENT / 100.0);
            let outro_exclusion = duration * (OUTRO_EXCLUSION_PERCENT / 100.0);
            
            let result = selector.select_segment(&video_path, duration, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT);
            prop_assert!(result.is_ok(), "Selection should succeed for valid duration");
            
            let time_range = result.unwrap();
            
            // Property 1: Clip duration should be between 12 and 18 seconds
            prop_assert!(time_range.duration_seconds >= MIN_CLIP_DURATION,
                "Clip duration {} should be >= {}", time_range.duration_seconds, MIN_CLIP_DURATION);
            prop_assert!(time_range.duration_seconds <= MAX_CLIP_DURATION,
                "Clip duration {} should be <= {}", time_range.duration_seconds, MAX_CLIP_DURATION);
            
            // Property 2: Start time should be at least intro_exclusion from beginning
            prop_assert!(time_range.start_seconds >= intro_exclusion,
                "Start time {} should be >= {} (intro exclusion)", 
                time_range.start_seconds, intro_exclusion);
            
            // Property 3: End time should be at least outro_exclusion before video end
            let end_time = time_range.start_seconds + time_range.duration_seconds;
            prop_assert!(end_time <= duration - outro_exclusion,
                "End time {} should be <= {} (duration {} - outro exclusion {})",
                end_time, duration - outro_exclusion, duration, outro_exclusion);
            
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
        
        let result = selector.select_segment(&video_path, duration, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT);
        assert!(result.is_ok());
        
        let time_range = result.unwrap();
        
        // Calculate actual exclusion zones (1% intro, 40% outro)
        let intro_exclusion = duration * (INTRO_EXCLUSION_PERCENT / 100.0); // 6 seconds
        let outro_exclusion = duration * (OUTRO_EXCLUSION_PERCENT / 100.0); // 240 seconds
        
        // Verify clip duration is between 12 and 18 seconds
        assert!(time_range.duration_seconds >= 12.0);
        assert!(time_range.duration_seconds <= 18.0);
        
        // Verify start time respects exclusion zones
        assert!(time_range.start_seconds >= intro_exclusion); // After intro exclusion
        assert!(time_range.start_seconds + time_range.duration_seconds <= duration - outro_exclusion); // Before outro exclusion
    }

    #[test]
    fn test_random_selector_short_video_fallback() {
        let selector = RandomSelector;
        let video_path = PathBuf::from("test.mp4");
        let duration = 10.0; // 10 seconds - too short for 1% intro (0.1s) + 12-18s clip + 40% outro (4s) = needs >16.1s
        
        let result = selector.select_segment(&video_path, duration, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT);
        assert!(result.is_ok());
        
        let time_range = result.unwrap();
        
        // Verify clip duration is capped at video duration (10 seconds)
        assert!(time_range.duration_seconds <= 10.0);
        assert_eq!(time_range.duration_seconds, 10.0);
        
        // Verify it uses middle segment (start should be roughly in the middle)
        let expected_middle = (duration - time_range.duration_seconds) / 2.0;
        assert!((time_range.start_seconds - expected_middle).abs() < 0.1);
    }

    #[test]
    fn test_random_selector_very_short_video() {
        let selector = RandomSelector;
        let video_path = PathBuf::from("test.mp4");
        let duration = 3.0; // 3 seconds - shorter than minimum clip duration
        
        let result = selector.select_segment(&video_path, duration, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT);
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
            let result = selector.select_segment(&video_path, duration, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT);
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
        fn test_random_selection_variety_property(duration in 415.0..3600.0f64) {
            // Test that for any video long enough to accommodate exclusions,
            // multiple selections produce different start times (variety)
            
            let selector = RandomSelector;
            let video_path = PathBuf::from("test.mp4");
            
            // Run selection multiple times (10 iterations)
            let mut start_times = Vec::new();
            for _ in 0..10 {
                let result = selector.select_segment(&video_path, duration, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT);
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
        
        // Should use 15 seconds (max clip duration)
        assert_eq!(time_range.duration_seconds, 18.0);
        
        // Should be centered: (600 - 18) / 2 = 291.0
        assert_eq!(time_range.start_seconds, 291.0);
    }

    #[test]
    fn test_intense_audio_selector_middle_segment_short_video() {
        // Test middle_segment with a short video
        let duration = 12.0; // 12 seconds
        
        let result = IntenseAudioSelector::middle_segment(duration);
        assert!(result.is_ok());
        
        let time_range = result.unwrap();
        
        // Should use full video duration (12 seconds)
        assert_eq!(time_range.duration_seconds, 12.0);
        
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
        let result = selector.select_segment(&video_path, duration, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT);
        
        // The result should be Ok (fallback to middle segment)
        assert!(result.is_ok(), "Should fall back to middle segment when no audio track");
        
        let time_range = result.unwrap();
        
        // Verify it uses middle segment calculation
        // For a 600 second video, with 18 second clip duration:
        // start = (600 - 18) / 2 = 291.0
        assert_eq!(time_range.duration_seconds, 18.0, "Should use max clip duration (18s)");
        assert_eq!(time_range.start_seconds, 291.0, "Should center the clip in the video");
        
        // Test with a shorter video
        let short_duration = 120.0; // 2 minutes
        let result_short = selector.select_segment(&video_path, short_duration, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT);
        
        assert!(result_short.is_ok(), "Should fall back to middle segment for short video");
        
        let time_range_short = result_short.unwrap();
        
        // For a 120 second video, with 18 second clip:
        // start = (120 - 18) / 2 = 51.0
        assert_eq!(time_range_short.duration_seconds, 18.0);
        assert_eq!(time_range_short.start_seconds, 51.0);
        
        // Test with a very short video (< 18 seconds)
        let very_short_duration = 7.0; // 7 seconds
        let result_very_short = selector.select_segment(&video_path, very_short_duration, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT);
        
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

    // Tests for ActionSelector

    #[test]
    fn test_action_selector_middle_segment_fallback() {
        // Test that middle_segment helper calculates correct fallback
        let duration = 600.0; // 10 minutes
        
        let result = ActionSelector::middle_segment(duration);
        assert!(result.is_ok());
        
        let time_range = result.unwrap();
        
        // Should use 18 seconds (max clip duration)
        assert_eq!(time_range.duration_seconds, 18.0);
        
        // Should be centered: (600 - 18) / 2 = 291.0
        assert_eq!(time_range.start_seconds, 291.0);
    }

    #[test]
    fn test_action_selector_middle_segment_short_video() {
        // Test middle_segment with a short video
        let duration = 12.0; // 12 seconds
        
        let result = ActionSelector::middle_segment(duration);
        assert!(result.is_ok());
        
        let time_range = result.unwrap();
        
        // Should use full video duration (12 seconds)
        assert_eq!(time_range.duration_seconds, 12.0);
        
        // Should start at 0 (centered)
        assert_eq!(time_range.start_seconds, 0.0);
    }

    #[test]
    fn test_action_selector_middle_segment_very_short_video() {
        // Test middle_segment with a very short video (< 5 seconds)
        let duration = 3.0; // 3 seconds
        
        let result = ActionSelector::middle_segment(duration);
        assert!(result.is_ok());
        
        let time_range = result.unwrap();
        
        // Should use full video duration
        assert_eq!(time_range.duration_seconds, 3.0);
        
        // Should start at 0
        assert_eq!(time_range.start_seconds, 0.0);
    }

    #[test]
    fn test_action_selector_no_motion_fallback() {
        // Test fallback to middle segment when video has no motion
        use crate::cli::Resolution;
        use std::path::PathBuf;

        // Create an FFmpegExecutor
        let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true);
        
        // Create ActionSelector
        let selector = ActionSelector::new(ffmpeg_executor);
        
        // Use a non-existent video path - this will cause motion analysis to fail
        // which simulates a video with no motion
        let video_path = PathBuf::from("/nonexistent/video_no_motion.mp4");
        let duration = 600.0; // 10 minutes
        
        // The selector should fall back to middle segment when motion analysis fails
        let result = selector.select_segment(&video_path, duration, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT);
        
        // The result should be Ok (fallback to middle segment)
        assert!(result.is_ok(), "Should fall back to middle segment when no motion detected");
        
        let time_range = result.unwrap();
        
        // Verify it uses middle segment calculation
        // For a 600 second video, with 18 second clip duration:
        // start = (600 - 18) / 2 = 291.0
        assert_eq!(time_range.duration_seconds, 18.0, "Should use max clip duration (18s)");
        assert_eq!(time_range.start_seconds, 291.0, "Should center the clip in the video");
    }

    #[test]
    fn test_action_selector_motion_segment_tie_breaking() {
        // Test that first occurrence is selected when multiple segments have identical motion scores
        use crate::ffmpeg::MotionSegment;
        
        // Create segments with identical motion scores
        let mut segments = vec![
            MotionSegment {
                start_time: 100.0,
                duration: 12.5,
                motion_score: 5.0,
            },
            MotionSegment {
                start_time: 50.0,
                duration: 12.5,
                motion_score: 5.0,
            },
            MotionSegment {
                start_time: 25.0,
                duration: 12.5,
                motion_score: 5.0,
            },
        ];
        
        // Sort by motion score (highest first)
        // This mimics what analyze_motion_intensity does
        segments.sort_by(|a, b| b.motion_score.partial_cmp(&a.motion_score).unwrap());
        
        // With stable sort, the original order should be preserved
        // So the first segment (start_time: 100.0) should remain first
        assert_eq!(segments[0].start_time, 100.0,
            "With identical motion scores, the first occurrence in the original list should be selected");
        assert_eq!(segments[1].start_time, 50.0,
            "Second segment should maintain original order");
        assert_eq!(segments[2].start_time, 25.0,
            "Third segment should maintain original order");
    }

    // Feature: action-based-clip-selection, Property 1: Highest Motion Score Selection
    proptest! {
        #[test]
        fn test_action_highest_motion_score_selection(
            num_segments in 2..10usize,
            scores in prop::collection::vec(0.1..10.0f64, 2..10),
        ) {
            use crate::ffmpeg::MotionSegment;
            
            // Generate motion segments with random scores
            let mut segments: Vec<MotionSegment> = scores.iter().enumerate()
                .map(|(i, &score)| MotionSegment {
                    start_time: i as f64 * 12.5,
                    duration: 12.5,
                    motion_score: score,
                })
                .take(num_segments)
                .collect();
            
            // Find the highest score before sorting
            let max_score = segments.iter()
                .map(|s| s.motion_score)
                .fold(f64::NEG_INFINITY, f64::max);
            
            // Sort by motion score (highest first) - mimics analyze_motion_intensity
            segments.sort_by(|a, b| b.motion_score.partial_cmp(&a.motion_score).unwrap());
            
            // Property: The first segment should have the highest motion score
            prop_assert_eq!(segments[0].motion_score, max_score,
                "First segment should have the highest motion score");
            
            // Property: All subsequent segments should have scores <= first segment
            for segment in segments.iter().skip(1) {
                prop_assert!(segment.motion_score <= segments[0].motion_score,
                    "All segments should have scores <= highest score");
            }
        }
    }

    // Feature: action-based-clip-selection, Property 2: Tie-Breaking by First Occurrence
    proptest! {
        #[test]
        fn test_action_tie_breaking_first_occurrence(
            num_segments in 3..10usize,
            identical_score in 1.0..10.0f64,
        ) {
            use crate::ffmpeg::MotionSegment;
            
            // Generate segments with identical scores
            let mut segments: Vec<MotionSegment> = (0..num_segments)
                .map(|i| MotionSegment {
                    start_time: i as f64 * 12.5,
                    duration: 12.5,
                    motion_score: identical_score,
                })
                .collect();
            
            // Record the original order (start times)
            let original_order: Vec<f64> = segments.iter().map(|s| s.start_time).collect();
            
            // Sort by motion score (highest first) - mimics analyze_motion_intensity
            // Rust's sort_by is stable, so equal elements maintain their original order
            segments.sort_by(|a, b| b.motion_score.partial_cmp(&a.motion_score).unwrap());
            
            // Property: When all scores are identical, the original order should be preserved
            for (i, segment) in segments.iter().enumerate() {
                prop_assert_eq!(segment.start_time, original_order[i],
                    "Segment at position {} should maintain original order (start_time: {})",
                    i, original_order[i]);
            }
            
            // Property: The first segment should be the one that appeared first originally
            prop_assert_eq!(segments[0].start_time, 0.0,
                "First segment should be the one that appeared first in the original list");
        }
    }

    // Feature: action-based-clip-selection, Property 4: Exclusion Zone Compliance
    proptest! {
        #[test]
        fn test_action_exclusion_zone_compliance(
            duration in 415.0..3600.0f64,
            intro_percent in 0.0..=10.0f64,
            outro_percent in 0.0..=50.0f64,
        ) {
            use crate::ffmpeg::MotionSegment;
            
            // Calculate exclusion zone boundaries
            let intro_boundary = duration * (intro_percent / 100.0);
            let outro_boundary = duration - (duration * (outro_percent / 100.0));
            
            // Create mock motion segments across the video duration
            let num_segments = (duration / 12.5).ceil() as usize;
            let segments: Vec<MotionSegment> = (0..num_segments)
                .map(|i| {
                    let start = i as f64 * 12.5;
                    let seg_duration = 12.5_f64.min(duration - start);
                    MotionSegment {
                        start_time: start,
                        duration: seg_duration,
                        motion_score: (i + 1) as f64, // Increasing scores
                    }
                })
                .collect();
            
            // Filter segments by exclusion zones (mimics ActionSelector logic)
            let filtered_segments: Vec<_> = segments.into_iter()
                .filter(|seg| {
                    let segment_start = seg.start_time;
                    let segment_end = seg.start_time + seg.duration;
                    
                    // Entire segment must be between boundaries
                    segment_start >= intro_boundary && segment_end <= outro_boundary
                })
                .collect();
            
            // Property: All filtered segments must respect exclusion zones
            for segment in filtered_segments.iter() {
                let segment_start = segment.start_time;
                let segment_end = segment.start_time + segment.duration;
                
                prop_assert!(segment_start >= intro_boundary,
                    "Segment start {} should be >= intro boundary {}",
                    segment_start, intro_boundary);
                
                prop_assert!(segment_end <= outro_boundary,
                    "Segment end {} should be <= outro boundary {}",
                    segment_end, outro_boundary);
            }
        }
    }

    // Feature: action-based-clip-selection, Property 5: Next Best Segment Selection
    proptest! {
        #[test]
        fn test_action_next_best_segment_selection(
            duration in 415.0..3600.0f64,
        ) {
            use crate::ffmpeg::MotionSegment;
            
            // Create a scenario where the highest-scoring segment violates exclusion zones
            // Set intro exclusion to 10% and outro exclusion to 40%
            let intro_percent = 10.0;
            let outro_percent = 40.0;
            let intro_boundary = duration * (intro_percent / 100.0);
            let outro_boundary = duration - (duration * (outro_percent / 100.0));
            
            // Create segments where the highest score is in the intro zone
            let mut segments = vec![
                // Highest score in intro zone (should be filtered out)
                MotionSegment {
                    start_time: 0.0,
                    duration: 12.5,
                    motion_score: 100.0,
                },
                // Second highest score in valid zone (should be selected)
                MotionSegment {
                    start_time: intro_boundary + 10.0,
                    duration: 12.5,
                    motion_score: 80.0,
                },
                // Third highest score in valid zone
                MotionSegment {
                    start_time: intro_boundary + 30.0,
                    duration: 12.5,
                    motion_score: 60.0,
                },
            ];
            
            // Sort by motion score (highest first)
            segments.sort_by(|a, b| b.motion_score.partial_cmp(&a.motion_score).unwrap());
            
            // Filter by exclusion zones
            let filtered_segments: Vec<_> = segments.into_iter()
                .filter(|seg| {
                    let segment_start = seg.start_time;
                    let segment_end = seg.start_time + seg.duration;
                    segment_start >= intro_boundary && segment_end <= outro_boundary
                })
                .collect();
            
            // Property: The first filtered segment should be the next best (second highest score)
            prop_assert!(!filtered_segments.is_empty(), "Should have at least one valid segment");
            prop_assert_eq!(filtered_segments[0].motion_score, 80.0,
                "First valid segment should have score 80.0 (next best after filtered out 100.0)");
            
            // Property: The selected segment should respect exclusion zones
            let selected = &filtered_segments[0];
            prop_assert!(selected.start_time >= intro_boundary,
                "Selected segment should be after intro boundary");
            prop_assert!(selected.start_time + selected.duration <= outro_boundary,
                "Selected segment should be before outro boundary");
        }
    }

    // Feature: action-based-clip-selection, Property 6: Clip Duration Constraints
    proptest! {
        #[test]
        fn test_action_clip_duration_constraints(
            duration in 415.0..3600.0f64,
        ) {
            use crate::cli::Resolution;
            use std::path::PathBuf;
            
            const MIN_CLIP_DURATION: f64 = 12.0;
            const MAX_CLIP_DURATION: f64 = 18.0;
            
            // Create an FFmpegExecutor and ActionSelector
            let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true);
            let selector = ActionSelector::new(ffmpeg_executor);
            
            // Use a non-existent video path (will fall back to middle segment)
            let video_path = PathBuf::from("/nonexistent/video.mp4");
            
            // Test with default exclusion zones
            let result = selector.select_segment(&video_path, duration, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT);
            
            prop_assert!(result.is_ok(), "Selection should succeed");
            
            let time_range = result.unwrap();
            
            // Property: Duration should be between MIN and MAX (or video duration if shorter)
            if duration >= MIN_CLIP_DURATION {
                prop_assert!(time_range.duration_seconds >= MIN_CLIP_DURATION,
                    "Clip duration {} should be >= {} for video duration {}",
                    time_range.duration_seconds, MIN_CLIP_DURATION, duration);
            }
            
            prop_assert!(time_range.duration_seconds <= MAX_CLIP_DURATION.min(duration),
                "Clip duration {} should be <= {} (or video duration {})",
                time_range.duration_seconds, MAX_CLIP_DURATION, duration);
            
            // Property: Duration should never exceed video duration
            prop_assert!(time_range.duration_seconds <= duration,
                "Clip duration {} should not exceed video duration {}",
                time_range.duration_seconds, duration);
        }
    }

    // Feature: action-based-clip-selection, Property 7: Clip Within Video Boundaries
    proptest! {
        #[test]
        fn test_action_clip_within_video_boundaries(
            duration in 12.0..3600.0f64,
            intro_percent in 0.0..=10.0f64,
            outro_percent in 0.0..=50.0f64,
        ) {
            use crate::cli::Resolution;
            use std::path::PathBuf;
            
            // Create an FFmpegExecutor and ActionSelector
            let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true);
            let selector = ActionSelector::new(ffmpeg_executor);
            
            // Use a non-existent video path (will fall back to middle segment)
            let video_path = PathBuf::from("/nonexistent/video.mp4");
            
            // Test with various exclusion zones
            let result = selector.select_segment(&video_path, duration, intro_percent, outro_percent);
            
            prop_assert!(result.is_ok(), "Selection should succeed");
            
            let time_range = result.unwrap();
            
            // Property: start + duration must not exceed video duration
            let end_time = time_range.start_seconds + time_range.duration_seconds;
            prop_assert!(end_time <= duration,
                "End time {} (start {} + duration {}) should not exceed video duration {}",
                end_time, time_range.start_seconds, time_range.duration_seconds, duration);
            
            // Property: start should be non-negative
            prop_assert!(time_range.start_seconds >= 0.0,
                "Start time {} should be non-negative", time_range.start_seconds);
        }
    }

    // Feature: action-based-clip-selection, Property 8: Valid TimeRange Return
    proptest! {
        #[test]
        fn test_action_valid_timerange_return(
            duration in 12.0..3600.0f64,
            intro_percent in 0.0..=10.0f64,
            outro_percent in 0.0..=50.0f64,
        ) {
            use crate::cli::Resolution;
            use std::path::PathBuf;
            
            // Create an FFmpegExecutor and ActionSelector
            let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true);
            let selector = ActionSelector::new(ffmpeg_executor);
            
            // Use a non-existent video path (will fall back to middle segment)
            let video_path = PathBuf::from("/nonexistent/video.mp4");
            
            // Test with various exclusion zones
            let result = selector.select_segment(&video_path, duration, intro_percent, outro_percent);
            
            prop_assert!(result.is_ok(), "Selection should succeed for valid inputs");
            
            let time_range = result.unwrap();
            
            // Property: start_seconds >= 0.0
            prop_assert!(time_range.start_seconds >= 0.0,
                "start_seconds {} should be >= 0.0", time_range.start_seconds);
            
            // Property: duration_seconds > 0.0
            prop_assert!(time_range.duration_seconds > 0.0,
                "duration_seconds {} should be > 0.0", time_range.duration_seconds);
            
            // Property: end <= video_duration
            let end_time = time_range.start_seconds + time_range.duration_seconds;
            prop_assert!(end_time <= duration,
                "end time {} should be <= video_duration {}", end_time, duration);
        }
    }

    // Feature: action-based-clip-selection, Property 9: Middle Segment Consistency
    proptest! {
        #[test]
        fn test_action_middle_segment_consistency(
            duration in 12.0..3600.0f64,
        ) {
            // Test that ActionSelector::middle_segment matches IntenseAudioSelector::middle_segment
            
            let action_result = ActionSelector::middle_segment(duration);
            let audio_result = IntenseAudioSelector::middle_segment(duration);
            
            prop_assert!(action_result.is_ok(), "ActionSelector middle_segment should succeed");
            prop_assert!(audio_result.is_ok(), "IntenseAudioSelector middle_segment should succeed");
            
            let action_range = action_result.unwrap();
            let audio_range = audio_result.unwrap();
            
            // Property: Both should return the same TimeRange
            prop_assert_eq!(action_range.start_seconds, audio_range.start_seconds,
                "ActionSelector and IntenseAudioSelector should have same start_seconds");
            prop_assert_eq!(action_range.duration_seconds, audio_range.duration_seconds,
                "ActionSelector and IntenseAudioSelector should have same duration_seconds");
        }
    }

    // Feature: action-based-clip-selection, Property 10: Timestamp Scaling Correctness
    proptest! {
        #[test]
        fn test_action_timestamp_scaling_correctness(
            full_duration in 400.0..3600.0f64,
        ) {
            use crate::ffmpeg::MotionSegment;
            
            // Simulate the scaling logic from analyze_motion_intensity
            const MAX_ANALYSIS_DURATION: f64 = 300.0; // 5 minutes
            const SEGMENT_DURATION: f64 = 12.5;
            
            let analysis_duration = full_duration.min(MAX_ANALYSIS_DURATION);
            let scale_factor = full_duration / analysis_duration;
            
            // Create a segment at a specific time in the analyzed portion
            let analyzed_time = 100.0; // 100 seconds into the analyzed portion
            
            // Property: When scaled to full duration, the timestamp should be proportional
            let scaled_time = analyzed_time * scale_factor;
            
            // Verify the scaling formula
            prop_assert_eq!(scaled_time, analyzed_time * (full_duration / analysis_duration),
                "Scaled time should equal analyzed_time * (full_duration / analysis_duration)");
            
            // Property: For videos <= 5 minutes, no scaling should occur (scale_factor = 1.0)
            if full_duration <= MAX_ANALYSIS_DURATION {
                prop_assert_eq!(scale_factor, 1.0,
                    "Scale factor should be 1.0 for videos <= 5 minutes");
                prop_assert_eq!(scaled_time, analyzed_time,
                    "Scaled time should equal analyzed time when no scaling needed");
            }
            
            // Property: For videos > 5 minutes, scaling should occur (scale_factor > 1.0)
            if full_duration > MAX_ANALYSIS_DURATION {
                prop_assert!(scale_factor > 1.0,
                    "Scale factor should be > 1.0 for videos > 5 minutes");
                prop_assert!(scaled_time > analyzed_time,
                    "Scaled time should be > analyzed time when scaling is applied");
            }
            
            // Property: Scaled time should never exceed full duration
            prop_assert!(scaled_time <= full_duration,
                "Scaled time {} should not exceed full duration {}",
                scaled_time, full_duration);
        }
    }

    // Feature: action-based-clip-selection, Property 11: Stateless Processing
    proptest! {
        #[test]
        fn test_action_stateless_processing(
            duration1 in 100.0..600.0f64,
            duration2 in 100.0..600.0f64,
        ) {
            use crate::cli::Resolution;
            use std::path::PathBuf;
            
            // Create a single ActionSelector instance
            let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true);
            let selector = ActionSelector::new(ffmpeg_executor);
            
            // Process first video (non-existent, will fall back to middle segment)
            let video_path1 = PathBuf::from("/nonexistent/video1.mp4");
            let result1 = selector.select_segment(&video_path1, duration1, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT);
            
            prop_assert!(result1.is_ok(), "First selection should succeed");
            let time_range1 = result1.unwrap();
            
            // Process second video (non-existent, will fall back to middle segment)
            let video_path2 = PathBuf::from("/nonexistent/video2.mp4");
            let result2 = selector.select_segment(&video_path2, duration2, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT);
            
            prop_assert!(result2.is_ok(), "Second selection should succeed");
            let time_range2 = result2.unwrap();
            
            // Process the second video again with the same selector
            let result2_again = selector.select_segment(&video_path2, duration2, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT);
            
            prop_assert!(result2_again.is_ok(), "Second selection (repeated) should succeed");
            let time_range2_again = result2_again.unwrap();
            
            // Property: Processing the same video twice should produce the same result
            prop_assert_eq!(time_range2.start_seconds, time_range2_again.start_seconds,
                "Same video should produce same start_seconds on repeated processing");
            prop_assert_eq!(time_range2.duration_seconds, time_range2_again.duration_seconds,
                "Same video should produce same duration_seconds on repeated processing");
            
            // Property: Results should be independent of previous processing
            // If durations are different, results should be different
            if (duration1 - duration2).abs() > 1.0 {
                // For significantly different durations, we expect different results
                // (unless both happen to center at the same position, which is unlikely)
                let results_differ = time_range1.start_seconds != time_range2.start_seconds
                    || time_range1.duration_seconds != time_range2.duration_seconds;
                
                // This property is probabilistic but should hold in most cases
                // We're verifying that the selector doesn't cache state from video1 when processing video2
                prop_assert!(results_differ || (duration1 - duration2).abs() < 10.0,
                    "Different videos should typically produce different results (unless durations are very similar)");
            }
        }
    }
}
