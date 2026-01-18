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
        const MIN_CLIP_DURATION: f64 = 5.0;
        const MAX_CLIP_DURATION: f64 = 10.0;
        
        // Generate random clip duration between 5 and 10 seconds
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

pub struct IntenseAudioSelector;

#[derive(Debug, thiserror::Error)]
pub enum SelectionError {
    #[error("Video too short: {0}s")]
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
        fn test_random_selection_valid_bounds(duration in 315.0..3600.0f64) {
            // Test that for videos long enough to accommodate exclusions,
            // the selected segment respects the intro and outro exclusion zones
            
            const INTRO_EXCLUSION: f64 = 60.0;
            const OUTRO_EXCLUSION: f64 = 240.0;
            const MIN_CLIP_DURATION: f64 = 5.0;
            const MAX_CLIP_DURATION: f64 = 10.0;
            
            let selector = RandomSelector;
            let video_path = PathBuf::from("test.mp4");
            
            // Only test videos that are long enough for exclusions
            // Required: INTRO_EXCLUSION + MIN_CLIP_DURATION + OUTRO_EXCLUSION = 305 seconds
            // We test from 315 seconds to give some margin
            
            let result = selector.select_segment(&video_path, duration);
            prop_assert!(result.is_ok(), "Selection should succeed for valid duration");
            
            let time_range = result.unwrap();
            
            // Property 1: Clip duration should be between 5 and 10 seconds
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
        
        // Verify clip duration is between 5 and 10 seconds
        assert!(time_range.duration_seconds >= 5.0);
        assert!(time_range.duration_seconds <= 10.0);
        
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
        
        // Verify clip duration is between 5 and 10 seconds
        assert!(time_range.duration_seconds >= 5.0);
        assert!(time_range.duration_seconds <= 10.0);
        
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
}
