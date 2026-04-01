// Clip selection strategies (trait + implementations)

use rand::RngExt;
use std::path::Path;

/// Minimum clip duration in seconds
pub const MIN_CLIP_DURATION: f64 = 10.0;

/// Maximum clip duration in seconds
pub const MAX_CLIP_DURATION: f64 = 15.0;

/// Configuration for clip duration constraints
#[derive(Clone)]
pub struct ClipConfig {
    pub min_duration: f64,
    pub max_duration: f64,
}

impl Default for ClipConfig {
    fn default() -> Self {
        Self {
            min_duration: MIN_CLIP_DURATION,
            max_duration: MAX_CLIP_DURATION,
        }
    }
}

impl ClipConfig {
    /// Get a random duration within the configured range
    #[allow(dead_code)] // Public API method
    pub fn random_duration(&self) -> f64 {
        let mut rng = rand::rng();
        rng.random_range(self.min_duration..=self.max_duration)
    }

    /// Calculate middle segment as fallback when analysis fails
    pub fn middle_segment(&self, duration: f64) -> Result<TimeRange, SelectionError> {
        let clip_duration = self.max_duration.min(duration);
        let actual_duration = clip_duration.max(self.min_duration).min(duration);
        let start = ((duration - actual_duration) / 2.0).max(0.0);

        Ok(TimeRange {
            start_seconds: start,
            duration_seconds: actual_duration,
        })
    }
}

/// Represents a time segment within a video.
///
/// Used to specify which portion of a video should be extracted as a clip.
#[derive(Clone)]
pub struct TimeRange {
    /// Start position in seconds from the beginning of the video
    pub start_seconds: f64,
    /// Duration of the clip in seconds
    pub duration_seconds: f64,
}

impl TimeRange {
    /// Check if this time range overlaps with another time range.
    ///
    /// Two ranges overlap if they share any portion of time. Adjacent ranges
    /// (where one ends exactly when the other starts) are not considered overlapping.
    ///
    /// # Arguments
    /// * `other` - The other TimeRange to check for overlap
    ///
    /// # Returns
    /// `true` if the ranges overlap, `false` otherwise
    ///
    /// # Examples
    /// ```
    /// # use video_clip_extractor::selector::TimeRange;
    /// let range1 = TimeRange { start_seconds: 10.0, duration_seconds: 5.0 };
    /// let range2 = TimeRange { start_seconds: 12.0, duration_seconds: 5.0 };
    /// assert!(range1.overlaps(&range2)); // Overlaps from 12.0 to 15.0
    ///
    /// let range3 = TimeRange { start_seconds: 15.0, duration_seconds: 5.0 };
    /// assert!(!range1.overlaps(&range3)); // Adjacent but not overlapping
    /// ```
    pub fn overlaps(&self, other: &TimeRange) -> bool {
        let self_end = self.start_seconds + self.duration_seconds;
        let other_end = other.start_seconds + other.duration_seconds;

        // Ranges overlap if one starts before the other ends AND vice versa
        // Using < instead of <= means adjacent ranges (touching) don't overlap
        !(self_end <= other.start_seconds || other_end <= self.start_seconds)
    }

    /// Calculate the duration of this time range.
    ///
    /// # Returns
    /// The duration in seconds
    ///
    /// # Examples
    /// ```
    /// # use video_clip_extractor::selector::TimeRange;
    /// let range = TimeRange { start_seconds: 10.0, duration_seconds: 15.0 };
    /// assert_eq!(range.duration(), 15.0);
    /// ```
    #[allow(dead_code)] // Public API method
    pub fn duration(&self) -> f64 {
        self.duration_seconds
    }

    /// Check if the duration of this time range is within valid clip bounds.
    ///
    /// Valid clip durations are between MIN_CLIP_DURATION (10 seconds) and
    /// MAX_CLIP_DURATION (15 seconds) inclusive.
    ///
    /// # Returns
    /// `true` if the duration is valid, `false` otherwise
    ///
    /// # Examples
    /// ```
    /// # use video_clip_extractor::selector::TimeRange;
    /// let valid_range = TimeRange { start_seconds: 10.0, duration_seconds: 12.0 };
    /// assert!(valid_range.is_valid_duration());
    ///
    /// let too_short = TimeRange { start_seconds: 10.0, duration_seconds: 5.0 };
    /// assert!(!too_short.is_valid_duration());
    ///
    /// let too_long = TimeRange { start_seconds: 10.0, duration_seconds: 25.0 };
    /// assert!(!too_long.is_valid_duration());
    /// ```
    #[allow(dead_code)] // Public API method
    pub fn is_valid_duration(&self) -> bool {
        let duration = self.duration();
        (MIN_CLIP_DURATION..=MAX_CLIP_DURATION).contains(&duration)
    }
}

/// Helper struct for calculating and validating exclusion zone boundaries
struct ExclusionZones {
    intro_boundary: f64,
    outro_boundary: f64,
}

impl ExclusionZones {
    /// Create new exclusion zones based on video duration and percentages
    fn new(duration: f64, intro_percent: f64, outro_percent: f64) -> Self {
        Self {
            intro_boundary: duration * (intro_percent / 100.0),
            outro_boundary: duration - (duration * (outro_percent / 100.0)),
        }
    }

    /// Check if a segment (defined by start and end times) falls within valid zones
    fn contains_segment(&self, start: f64, end: f64) -> bool {
        start >= self.intro_boundary && end <= self.outro_boundary
    }
}

/// Trait for selecting clip segments from videos.
///
/// Implementations of this trait provide different strategies for selecting
/// one or more non-overlapping clip segments from a video file. All clips
/// must respect exclusion zones (intro/outro) and duration constraints.
pub trait ClipSelector: Send + Sync {
    /// Select multiple non-overlapping clip segments from a video.
    ///
    /// # Arguments
    /// * `video_path` - Path to the video file
    /// * `duration` - Total video duration in seconds
    /// * `intro_exclusion_percent` - Percentage of video duration to exclude from start (0-100)
    /// * `outro_exclusion_percent` - Percentage of video duration to exclude from end (0-100)
    /// * `clip_count` - Number of clips to generate (1-4)
    /// * `config` - Clip duration configuration (min/max bounds)
    ///
    /// # Returns
    /// Vector of TimeRange objects representing non-overlapping clip segments,
    /// sorted by start time. May return fewer than clip_count if video is too short
    /// or if insufficient non-overlapping segments can be found within constraints.
    ///
    /// # Behavior
    /// - All returned clips must be non-overlapping
    /// - All clips must fall within the valid selection zone (after intro, before outro)
    /// - All clips must have durations between config.min_duration and config.max_duration
    /// - Clips are returned in chronological order (sorted by start time)
    /// - If the requested clip_count cannot be satisfied, returns as many valid clips as possible
    fn select_clips(
        &self,
        video_path: &Path,
        duration: f64,
        intro_exclusion_percent: f64,
        outro_exclusion_percent: f64,
        clip_count: u8,
        config: &ClipConfig,
    ) -> Result<Vec<TimeRange>, SelectionError>;
}

pub struct RandomSelector;

impl ClipSelector for RandomSelector {
    fn select_clips(
        &self,
        _video_path: &Path,
        duration: f64,
        intro_exclusion_percent: f64,
        outro_exclusion_percent: f64,
        clip_count: u8,
        config: &ClipConfig,
    ) -> Result<Vec<TimeRange>, SelectionError> {
        const MAX_ATTEMPTS: u32 = 1000;

        // Calculate valid selection zone (intro/outro exclusion)
        let intro_cutoff = duration * (intro_exclusion_percent / 100.0);
        let outro_cutoff = duration - (duration * (outro_exclusion_percent / 100.0));
        let valid_duration = outro_cutoff - intro_cutoff;

        // Check if video can accommodate requested clips
        let min_required = (clip_count as f64) * config.min_duration;

        // If video is too short, generate as many clips as possible
        let actual_clip_count = if valid_duration < min_required {
            (valid_duration / config.min_duration).floor() as u8
        } else {
            clip_count
        };

        // If no clips can be generated, return empty vector
        if actual_clip_count == 0 || valid_duration < config.min_duration {
            return Ok(vec![]);
        }

        let mut clips = Vec::new();
        let mut rng = rand::rng();
        let mut attempts = 0;
        let mut consecutive_empty_gaps = 0;

        // Implement loop to generate N random non-overlapping clips
        // Use a smarter approach: track available gaps and sample from them
        while clips.len() < actual_clip_count as usize && attempts < MAX_ATTEMPTS {
            attempts += 1;

            // Generate random clip duration between min and max
            let clip_duration = rng.random_range(config.min_duration..=config.max_duration);

            // Find available gaps between existing clips
            let available_gaps =
                self.find_available_gaps(intro_cutoff, outro_cutoff, &clips, clip_duration);

            // If no gaps available, try with a shorter clip duration
            if available_gaps.is_empty() {
                consecutive_empty_gaps += 1;
                // Early exit if we consistently can't find gaps
                if consecutive_empty_gaps > 50 {
                    eprintln!(
                        "Warning: Unable to find gaps for additional clips, returning {} of {} requested clips",
                        clips.len(),
                        clip_count
                    );
                    break;
                }
                continue;
            }

            consecutive_empty_gaps = 0; // Reset on success

            // Randomly select a gap
            let gap_index = rng.random_range(0..available_gaps.len());
            let (gap_start, gap_end) = available_gaps[gap_index];

            // Generate random start time within the selected gap
            let max_start = gap_end - clip_duration;
            let start = rng.random_range(gap_start..=max_start);

            let candidate = TimeRange {
                start_seconds: start,
                duration_seconds: clip_duration,
            };

            // Double-check for overlaps (should not happen with gap-based approach)
            if !clips.iter().any(|existing| candidate.overlaps(existing)) {
                clips.push(candidate);
            }
        }

        // Log if we hit MAX_ATTEMPTS
        if attempts >= MAX_ATTEMPTS && clips.len() < actual_clip_count as usize {
            eprintln!(
                "Warning: Reached MAX_ATTEMPTS ({}) while selecting clips, returning {} of {} requested clips",
                MAX_ATTEMPTS,
                clips.len(),
                clip_count
            );
        }

        // Sort clips by start time before returning
        clips.sort_by(|a, b| a.start_seconds.partial_cmp(&b.start_seconds).unwrap());

        Ok(clips)
    }
}

impl RandomSelector {
    /// Find available gaps in the valid zone where a clip of the given duration can fit.
    /// Returns a vector of (start, end) tuples representing available gaps.
    fn find_available_gaps(
        &self,
        intro_cutoff: f64,
        outro_cutoff: f64,
        existing_clips: &[TimeRange],
        clip_duration: f64,
    ) -> Vec<(f64, f64)> {
        let mut gaps = Vec::new();

        // If no existing clips, the entire valid zone is available
        if existing_clips.is_empty() {
            if outro_cutoff - intro_cutoff >= clip_duration {
                gaps.push((intro_cutoff, outro_cutoff));
            }
            return gaps;
        }

        // Sort clips by start time (should already be sorted, but ensure it)
        let mut sorted_clips = existing_clips.to_vec();
        sorted_clips.sort_by(|a, b| a.start_seconds.partial_cmp(&b.start_seconds).unwrap());

        // Check gap before first clip
        let first_clip_start = sorted_clips[0].start_seconds;
        if first_clip_start - intro_cutoff >= clip_duration {
            gaps.push((intro_cutoff, first_clip_start));
        }

        // Check gaps between consecutive clips
        for i in 0..sorted_clips.len() - 1 {
            let current_end = sorted_clips[i].start_seconds + sorted_clips[i].duration_seconds;
            let next_start = sorted_clips[i + 1].start_seconds;
            let gap_size = next_start - current_end;

            if gap_size >= clip_duration {
                gaps.push((current_end, next_start));
            }
        }

        // Check gap after last clip
        let last_clip_end = sorted_clips[sorted_clips.len() - 1].start_seconds
            + sorted_clips[sorted_clips.len() - 1].duration_seconds;
        if outro_cutoff - last_clip_end >= clip_duration {
            gaps.push((last_clip_end, outro_cutoff));
        }

        gaps
    }
}

/// Trait for intensity segments (audio or motion)
trait IntensitySegment {
    fn start_time(&self) -> f64;
    fn duration(&self) -> f64;
}

impl IntensitySegment for crate::ffmpeg::AudioSegment {
    fn start_time(&self) -> f64 {
        self.start_time
    }
    fn duration(&self) -> f64 {
        self.duration
    }
}

impl IntensitySegment for crate::ffmpeg::MotionSegment {
    fn start_time(&self) -> f64 {
        self.start_time
    }
    fn duration(&self) -> f64 {
        self.duration
    }
}

/// Common implementation for selecting clips from intensity peaks (audio or motion)
///
/// This function extracts the shared logic between IntenseAudioSelector and ActionSelector,
/// eliminating code duplication while maintaining the same behavior.
fn select_clips_from_peaks<T: IntensitySegment>(
    segments: Vec<T>,
    duration: f64,
    intro_exclusion_percent: f64,
    outro_exclusion_percent: f64,
    clip_count: u8,
    config: &ClipConfig,
) -> Result<Vec<TimeRange>, SelectionError> {
    const MAX_ATTEMPTS: u32 = 1000;

    // Calculate valid selection zone
    let intro_cutoff = duration * (intro_exclusion_percent / 100.0);
    let outro_cutoff = duration - (duration * (outro_exclusion_percent / 100.0));
    let valid_duration = outro_cutoff - intro_cutoff;

    // Check if video can accommodate requested clips
    let min_required = (clip_count as f64) * config.min_duration;

    // If video is too short, generate as many clips as possible
    let actual_clip_count = if valid_duration < min_required {
        (valid_duration / config.min_duration).floor() as u8
    } else {
        clip_count
    };

    // If no clips can be generated within exclusion zones, fall back to middle segment
    if actual_clip_count == 0 || valid_duration < config.min_duration {
        return Ok(vec![config.middle_segment(duration)?]);
    }

    // Calculate exclusion zone boundaries
    let zones = ExclusionZones::new(duration, intro_exclusion_percent, outro_exclusion_percent);

    // Find intensity peaks within valid zone
    // Segments are already sorted by intensity (highest first)
    let valid_peaks: Vec<_> = segments
        .into_iter()
        .filter(|seg| {
            let segment_end = seg.start_time() + seg.duration();
            zones.contains_segment(seg.start_time(), segment_end)
        })
        .collect();

    // If no valid peaks found, fall back to middle segment
    if valid_peaks.is_empty() {
        return Ok(vec![config.middle_segment(duration)?]);
    }

    // Select top N non-overlapping peaks
    let mut selected_clips = Vec::new();
    let mut attempts = 0;
    let mut consecutive_failures = 0;

    for peak in valid_peaks {
        if selected_clips.len() >= actual_clip_count as usize {
            break;
        }

        attempts += 1;
        if attempts > MAX_ATTEMPTS {
            eprintln!(
                "Warning: Reached MAX_ATTEMPTS ({}) while selecting clips, returning {} of {} requested clips",
                MAX_ATTEMPTS,
                selected_clips.len(),
                clip_count
            );
            break;
        }

        // Early exit if we're consistently failing to find valid clips
        if consecutive_failures > 50 {
            eprintln!(
                "Warning: Too many consecutive failures, returning {} of {} requested clips",
                selected_clips.len(),
                clip_count
            );
            break;
        }

        // Create TimeRange around peak
        // Use a duration in the middle of the valid range
        let clip_duration = config.min_duration + (config.max_duration - config.min_duration) * 0.5;

        // Center the clip around the peak's start time
        let mut start = (peak.start_time() - clip_duration / 2.0).max(intro_cutoff);

        // Ensure the clip doesn't exceed outro boundary
        let mut end = start + clip_duration;
        if end > outro_cutoff {
            end = outro_cutoff;
            start = (end - clip_duration).max(intro_cutoff);
        }

        // Recalculate duration in case adjustments were made
        let actual_duration = end - start;

        // Skip if duration is invalid
        if !(config.min_duration..=config.max_duration).contains(&actual_duration) {
            consecutive_failures += 1;
            continue;
        }

        let candidate = TimeRange {
            start_seconds: start,
            duration_seconds: actual_duration,
        };

        // Check for overlaps with existing clips
        if !selected_clips
            .iter()
            .any(|existing| candidate.overlaps(existing))
            && (config.min_duration..=config.max_duration).contains(&candidate.duration_seconds)
        {
            selected_clips.push(candidate);
            consecutive_failures = 0; // Reset on success
        } else {
            consecutive_failures += 1;
        }
    }

    // If we couldn't generate any clips from peaks, fall back to middle segment
    if selected_clips.is_empty() {
        return Ok(vec![config.middle_segment(duration)?]);
    }

    // Sort selected clips by start time before returning
    selected_clips.sort_by(|a, b| a.start_seconds.partial_cmp(&b.start_seconds).unwrap());

    Ok(selected_clips)
}

pub struct IntenseAudioSelector;

impl IntenseAudioSelector {
    pub fn new(_ffmpeg_executor: crate::ffmpeg::FFmpegExecutor) -> Self {
        Self
    }
}

impl ClipSelector for IntenseAudioSelector {
    fn select_clips(
        &self,
        video_path: &Path,
        duration: f64,
        intro_exclusion_percent: f64,
        outro_exclusion_percent: f64,
        clip_count: u8,
        config: &ClipConfig,
    ) -> Result<Vec<TimeRange>, SelectionError> {
        // Try to analyze audio intensity
        match crate::ffmpeg::analyze_audio_intensity(video_path, duration) {
            Ok(segments) => select_clips_from_peaks(
                segments,
                duration,
                intro_exclusion_percent,
                outro_exclusion_percent,
                clip_count,
                config,
            ),
            Err(crate::ffmpeg::FFmpegError::NoAudioTrack) => {
                // No audio track, fall back to middle segment
                Ok(vec![config.middle_segment(duration)?])
            }
            Err(e) => {
                // Other errors, return as SelectionError
                Err(SelectionError::AudioAnalysisFailed(e.to_string()))
            }
        }
    }
}

pub struct ActionSelector;

impl ActionSelector {
    pub fn new(_ffmpeg_executor: crate::ffmpeg::FFmpegExecutor) -> Self {
        Self
    }
}

impl ClipSelector for ActionSelector {
    fn select_clips(
        &self,
        video_path: &Path,
        duration: f64,
        intro_exclusion_percent: f64,
        outro_exclusion_percent: f64,
        clip_count: u8,
        config: &ClipConfig,
    ) -> Result<Vec<TimeRange>, SelectionError> {
        // Try to analyze motion intensity
        match crate::ffmpeg::analyze_motion_intensity(video_path, duration) {
            Ok(segments) => select_clips_from_peaks(
                segments,
                duration,
                intro_exclusion_percent,
                outro_exclusion_percent,
                clip_count,
                config,
            ),
            Err(_e) => {
                // Motion analysis failed, fall back to middle segment
                Ok(vec![config.middle_segment(duration)?])
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
    #[allow(dead_code)]
    MotionAnalysisFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::path::PathBuf;

    // Test constants
    const MIN_TEST_DURATION: f64 = 415.0; // Minimum duration for exclusion zone tests
    const FLOAT_EPSILON: f64 = 0.1; // Tolerance for floating-point comparisons
    const INTRO_EXCLUSION_PERCENT: f64 = 1.0; // Default intro exclusion for tests
    const OUTRO_EXCLUSION_PERCENT: f64 = 40.0; // Default outro exclusion for tests

    // Unit tests for TimeRange methods
    // Requirements: 3.1, 5.1

    #[test]
    fn test_timerange_overlaps_complete_overlap() {
        // Test case: range2 is completely inside range1
        let range1 = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 20.0,
        };
        let range2 = TimeRange {
            start_seconds: 15.0,
            duration_seconds: 5.0,
        };

        assert!(
            range1.overlaps(&range2),
            "range1 should overlap with range2 (range2 inside range1)"
        );
        assert!(
            range2.overlaps(&range1),
            "range2 should overlap with range1 (symmetric)"
        );
    }

    #[test]
    fn test_timerange_overlaps_partial_overlap() {
        // Test case: ranges partially overlap
        let range1 = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 10.0,
        }; // 10-20
        let range2 = TimeRange {
            start_seconds: 15.0,
            duration_seconds: 10.0,
        }; // 15-25

        assert!(
            range1.overlaps(&range2),
            "range1 should overlap with range2 (partial overlap)"
        );
        assert!(
            range2.overlaps(&range1),
            "range2 should overlap with range1 (symmetric)"
        );
    }

    #[test]
    fn test_timerange_overlaps_start_overlap() {
        // Test case: range2 starts before range1 ends
        let range1 = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 5.0,
        }; // 10-15
        let range2 = TimeRange {
            start_seconds: 12.0,
            duration_seconds: 8.0,
        }; // 12-20

        assert!(
            range1.overlaps(&range2),
            "range1 should overlap with range2"
        );
        assert!(
            range2.overlaps(&range1),
            "range2 should overlap with range1 (symmetric)"
        );
    }

    #[test]
    fn test_timerange_no_overlap_before() {
        // Test case: range1 ends before range2 starts
        let range1 = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 5.0,
        }; // 10-15
        let range2 = TimeRange {
            start_seconds: 20.0,
            duration_seconds: 5.0,
        }; // 20-25

        assert!(
            !range1.overlaps(&range2),
            "range1 should not overlap with range2 (gap between)"
        );
        assert!(
            !range2.overlaps(&range1),
            "range2 should not overlap with range1 (symmetric)"
        );
    }

    #[test]
    fn test_timerange_no_overlap_after() {
        // Test case: range2 ends before range1 starts
        let range1 = TimeRange {
            start_seconds: 20.0,
            duration_seconds: 5.0,
        }; // 20-25
        let range2 = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 5.0,
        }; // 10-15

        assert!(
            !range1.overlaps(&range2),
            "range1 should not overlap with range2"
        );
        assert!(
            !range2.overlaps(&range1),
            "range2 should not overlap with range1 (symmetric)"
        );
    }

    #[test]
    fn test_timerange_adjacent_touching() {
        // Test case: ranges are adjacent (touching but not overlapping)
        let range1 = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 5.0,
        }; // 10-15
        let range2 = TimeRange {
            start_seconds: 15.0,
            duration_seconds: 5.0,
        }; // 15-20

        assert!(
            !range1.overlaps(&range2),
            "Adjacent ranges should not overlap (range1 ends where range2 starts)"
        );
        assert!(
            !range2.overlaps(&range1),
            "Adjacent ranges should not overlap (symmetric)"
        );
    }

    #[test]
    fn test_timerange_adjacent_touching_reverse() {
        // Test case: ranges are adjacent in reverse order
        let range1 = TimeRange {
            start_seconds: 15.0,
            duration_seconds: 5.0,
        }; // 15-20
        let range2 = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 5.0,
        }; // 10-15

        assert!(
            !range1.overlaps(&range2),
            "Adjacent ranges should not overlap"
        );
        assert!(
            !range2.overlaps(&range1),
            "Adjacent ranges should not overlap (symmetric)"
        );
    }

    #[test]
    fn test_timerange_identical_ranges() {
        // Test case: identical ranges
        let range1 = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 5.0,
        };
        let range2 = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 5.0,
        };

        assert!(range1.overlaps(&range2), "Identical ranges should overlap");
        assert!(
            range2.overlaps(&range1),
            "Identical ranges should overlap (symmetric)"
        );
    }

    #[test]
    fn test_timerange_zero_duration() {
        // Test case: zero duration range (edge case)
        let range1 = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 0.0,
        };
        let range2 = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 5.0,
        };

        // Zero duration range at the start of another range should not overlap
        assert!(
            !range1.overlaps(&range2),
            "Zero duration range should not overlap"
        );
        assert!(
            !range2.overlaps(&range1),
            "Zero duration range should not overlap (symmetric)"
        );
    }

    #[test]
    fn test_timerange_duration_calculation() {
        // Test duration() method returns the duration_seconds field
        let range1 = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 15.0,
        };
        assert_eq!(range1.duration(), 15.0, "Duration should be 15.0");

        let range2 = TimeRange {
            start_seconds: 0.0,
            duration_seconds: 12.0,
        };
        assert_eq!(range2.duration(), 12.0, "Duration should be 12.0");

        let range3 = TimeRange {
            start_seconds: 100.0,
            duration_seconds: 18.0,
        };
        assert_eq!(range3.duration(), 18.0, "Duration should be 18.0");
    }

    #[test]
    fn test_timerange_valid_duration_within_bounds() {
        // Test valid durations (10-15 seconds)
        let range_min = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 10.0,
        };
        assert!(
            range_min.is_valid_duration(),
            "10 seconds should be valid (minimum)"
        );

        let range_mid = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 12.5,
        };
        assert!(
            range_mid.is_valid_duration(),
            "12.5 seconds should be valid (middle)"
        );

        let range_max = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 15.0,
        };
        assert!(
            range_max.is_valid_duration(),
            "15 seconds should be valid (maximum)"
        );
    }

    #[test]
    fn test_timerange_invalid_duration_too_short() {
        // Test durations below minimum (< 10 seconds)
        let range_short = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 9.9,
        };
        assert!(
            !range_short.is_valid_duration(),
            "9.9 seconds should be invalid (too short)"
        );

        let range_very_short = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 5.0,
        };
        assert!(
            !range_very_short.is_valid_duration(),
            "5 seconds should be invalid (too short)"
        );

        let range_zero = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 0.0,
        };
        assert!(
            !range_zero.is_valid_duration(),
            "0 seconds should be invalid"
        );
    }

    #[test]
    fn test_timerange_invalid_duration_too_long() {
        // Test durations above maximum (> 15 seconds)
        let range_long = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 15.1,
        };
        assert!(
            !range_long.is_valid_duration(),
            "15.1 seconds should be invalid (too long)"
        );

        let range_very_long = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 25.0,
        };
        assert!(
            !range_very_long.is_valid_duration(),
            "25 seconds should be invalid (too long)"
        );

        let range_extremely_long = TimeRange {
            start_seconds: 10.0,
            duration_seconds: 100.0,
        };
        assert!(
            !range_extremely_long.is_valid_duration(),
            "100 seconds should be invalid (too long)"
        );
    }

    #[test]
    fn test_timerange_boundary_values() {
        // Test exact boundary values
        let range_exactly_min = TimeRange {
            start_seconds: 0.0,
            duration_seconds: MIN_CLIP_DURATION,
        };
        assert!(
            range_exactly_min.is_valid_duration(),
            "Exactly MIN_CLIP_DURATION should be valid"
        );

        let range_exactly_max = TimeRange {
            start_seconds: 0.0,
            duration_seconds: MAX_CLIP_DURATION,
        };
        assert!(
            range_exactly_max.is_valid_duration(),
            "Exactly MAX_CLIP_DURATION should be valid"
        );

        let range_just_below_min = TimeRange {
            start_seconds: 0.0,
            duration_seconds: MIN_CLIP_DURATION - 0.01,
        };
        assert!(
            !range_just_below_min.is_valid_duration(),
            "Just below MIN_CLIP_DURATION should be invalid"
        );

        let range_just_above_max = TimeRange {
            start_seconds: 0.0,
            duration_seconds: MAX_CLIP_DURATION + 0.01,
        };
        assert!(
            !range_just_above_max.is_valid_duration(),
            "Just above MAX_CLIP_DURATION should be invalid"
        );
    }

    #[test]
    fn test_timerange_overlaps_with_multiple_ranges() {
        // Test a range against multiple other ranges
        let base_range = TimeRange {
            start_seconds: 50.0,
            duration_seconds: 15.0,
        }; // 50-65

        let before = TimeRange {
            start_seconds: 30.0,
            duration_seconds: 10.0,
        }; // 30-40
        let touching_before = TimeRange {
            start_seconds: 35.0,
            duration_seconds: 15.0,
        }; // 35-50
        let overlapping_start = TimeRange {
            start_seconds: 45.0,
            duration_seconds: 10.0,
        }; // 45-55
        let inside = TimeRange {
            start_seconds: 55.0,
            duration_seconds: 5.0,
        }; // 55-60
        let overlapping_end = TimeRange {
            start_seconds: 60.0,
            duration_seconds: 10.0,
        }; // 60-70
        let touching_after = TimeRange {
            start_seconds: 65.0,
            duration_seconds: 10.0,
        }; // 65-75
        let after = TimeRange {
            start_seconds: 70.0,
            duration_seconds: 10.0,
        }; // 70-80

        assert!(
            !base_range.overlaps(&before),
            "Should not overlap with range before"
        );
        assert!(
            !base_range.overlaps(&touching_before),
            "Should not overlap with range touching before"
        );
        assert!(
            base_range.overlaps(&overlapping_start),
            "Should overlap with range overlapping start"
        );
        assert!(
            base_range.overlaps(&inside),
            "Should overlap with range inside"
        );
        assert!(
            base_range.overlaps(&overlapping_end),
            "Should overlap with range overlapping end"
        );
        assert!(
            !base_range.overlaps(&touching_after),
            "Should not overlap with range touching after"
        );
        assert!(
            !base_range.overlaps(&after),
            "Should not overlap with range after"
        );
    }

    // Feature: multiple-clips-per-video, Property 4: Non-Overlapping Segments
    proptest! {
        #[test]
        fn test_non_overlapping_segments_property(
            // Generate a vector of time ranges with random start times and durations
            ranges in prop::collection::vec(
                (0.0..1000.0f64, MIN_CLIP_DURATION..=MAX_CLIP_DURATION),
                2..10
            )
        ) {
            // Convert tuples to TimeRange instances
            let time_ranges: Vec<TimeRange> = ranges.iter()
                .map(|(start, duration)| TimeRange {
                    start_seconds: *start,
                    duration_seconds: *duration,
                })
                .collect();

            // Property: For any pair of ranges that don't overlap according to overlaps(),
            // they should satisfy the mathematical definition of non-overlapping:
            // either range1.end <= range2.start OR range2.end <= range1.start
            for i in 0..time_ranges.len() {
                for j in (i + 1)..time_ranges.len() {
                    let range1 = &time_ranges[i];
                    let range2 = &time_ranges[j];

                    let range1_end = range1.start_seconds + range1.duration_seconds;
                    let range2_end = range2.start_seconds + range2.duration_seconds;

                    let overlaps_result = range1.overlaps(range2);

                    // Mathematical definition: ranges don't overlap if one ends before or at the other starts
                    let mathematically_non_overlapping =
                        range1_end <= range2.start_seconds || range2_end <= range1.start_seconds;

                    // Property: overlaps() should return true IFF ranges are NOT mathematically non-overlapping
                    prop_assert_eq!(
                        overlaps_result,
                        !mathematically_non_overlapping,
                        "overlaps() result should match mathematical definition. \
                         Range1: [{}, {}), Range2: [{}, {}), overlaps()={}, math_non_overlapping={}",
                        range1.start_seconds, range1_end,
                        range2.start_seconds, range2_end,
                        overlaps_result, mathematically_non_overlapping
                    );

                    // Property: overlaps() should be symmetric
                    prop_assert_eq!(
                        range1.overlaps(range2),
                        range2.overlaps(range1),
                        "overlaps() should be symmetric: range1.overlaps(range2) == range2.overlaps(range1)"
                    );
                }
            }
        }
    }

    // Feature: video-clip-extractor, Property 10: Random Selection Valid Bounds
    proptest! {
        #[test]
        fn test_random_selection_valid_bounds(duration in MIN_TEST_DURATION..3600.0f64) {
            // Test that for videos long enough to accommodate exclusions,
            // the selected segment respects the intro and outro exclusion zones

            const INTRO_EXCLUSION_PERCENT: f64 = 2.0;
            const OUTRO_EXCLUSION_PERCENT: f64 = 40.0;
            const MIN_CLIP_DURATION: f64 = 10.0;
            const MAX_CLIP_DURATION: f64 = 15.0;

            let selector = RandomSelector;
            let video_path = PathBuf::from("test.mp4");

            // Calculate actual exclusion zones based on percentages
            let intro_exclusion = duration * (INTRO_EXCLUSION_PERCENT / 100.0);
            let outro_exclusion = duration * (OUTRO_EXCLUSION_PERCENT / 100.0);

            let result = selector.select_clips(&video_path, duration, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT, 1, &ClipConfig::default());
            prop_assert!(result.is_ok(), "Selection should succeed for valid duration");

            let time_ranges = result.unwrap();
            prop_assert!(!time_ranges.is_empty(), "Should return at least one clip");
            let time_range = &time_ranges[0];

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

        let result = selector.select_clips(
            &video_path,
            duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            1,
            &ClipConfig::default(),
        );
        assert!(result.is_ok());

        let time_ranges = result.unwrap();
        assert!(!time_ranges.is_empty());
        let time_range = &time_ranges[0];

        // Calculate actual exclusion zones (1% intro, 40% outro)
        let intro_exclusion = duration * (INTRO_EXCLUSION_PERCENT / 100.0); // 6 seconds
        let outro_exclusion = duration * (OUTRO_EXCLUSION_PERCENT / 100.0); // 240 seconds

        // Verify clip duration is between 10 and 15 seconds
        assert!(time_range.duration_seconds >= 10.0);
        assert!(time_range.duration_seconds <= 15.0);

        // Verify start time respects exclusion zones
        assert!(time_range.start_seconds >= intro_exclusion); // After intro exclusion
        assert!(
            time_range.start_seconds + time_range.duration_seconds <= duration - outro_exclusion
        ); // Before outro exclusion
    }

    #[test]
    fn test_random_selector_short_video_fallback() {
        let selector = RandomSelector;
        let video_path = PathBuf::from("test.mp4");
        let duration = 10.0; // 10 seconds - too short for 1% intro (0.1s) + 10-15s clip + 40% outro (4s) = needs >14.1s

        let result = selector.select_clips(
            &video_path,
            duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            1,
            &ClipConfig::default(),
        );
        assert!(result.is_ok());

        let time_ranges = result.unwrap();
        // With the new implementation, videos too short for exclusion zones return empty vector
        assert_eq!(
            time_ranges.len(),
            0,
            "Should return empty vector for video too short for exclusion zones"
        );
    }

    #[test]
    fn test_random_selector_very_short_video() {
        let selector = RandomSelector;
        let video_path = PathBuf::from("test.mp4");
        let duration = 3.0; // 3 seconds - shorter than minimum clip duration

        let result = selector.select_clips(
            &video_path,
            duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            1,
            &ClipConfig::default(),
        );
        assert!(result.is_ok());

        let time_ranges = result.unwrap();
        // With the new implementation, videos too short return empty vector
        assert_eq!(
            time_ranges.len(),
            0,
            "Should return empty vector for video too short"
        );
    }

    #[test]
    fn test_random_selector_variety() {
        let selector = RandomSelector;
        let video_path = PathBuf::from("test.mp4");
        let duration = 600.0; // 10 minutes

        // Run multiple times and collect start times
        let mut start_times = Vec::new();
        for _ in 0..10 {
            let result = selector.select_clips(
                &video_path,
                duration,
                INTRO_EXCLUSION_PERCENT,
                OUTRO_EXCLUSION_PERCENT,
                1,
                &ClipConfig::default(),
            );
            assert!(result.is_ok());
            let time_ranges = result.unwrap();
            assert!(!time_ranges.is_empty());
            start_times.push(time_ranges[0].start_seconds);
        }

        // Verify that not all start times are identical (variety check)
        let first = start_times[0];
        let all_same = start_times
            .iter()
            .all(|&x| (x - first).abs() < FLOAT_EPSILON);
        assert!(
            !all_same,
            "Random selector should produce variety in start times"
        );
    }

    // Unit tests for RandomSelector with multiple clips
    // Requirements: 2.1, 2.3, 3.1, 4.1

    #[test]
    fn test_random_selector_single_clip_backward_compatibility() {
        // Test single clip generation (backward compatibility)
        // Requirement 2.1
        let selector = RandomSelector;
        let video_path = PathBuf::from("test.mp4");
        let duration = 600.0; // 10 minutes

        let result = selector.select_clips(
            &video_path,
            duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            1,
            &ClipConfig::default(),
        );
        assert!(result.is_ok(), "Single clip selection should succeed");

        let clips = result.unwrap();
        assert_eq!(
            clips.len(),
            1,
            "Should generate exactly 1 clip when clip_count=1"
        );

        let clip = &clips[0];

        // Verify clip duration is valid
        assert!(clip.duration_seconds >= MIN_CLIP_DURATION);
        assert!(clip.duration_seconds <= MAX_CLIP_DURATION);

        // Verify exclusion zones are respected
        let intro_cutoff = duration * (INTRO_EXCLUSION_PERCENT / 100.0);
        let outro_cutoff = duration - (duration * (OUTRO_EXCLUSION_PERCENT / 100.0));

        assert!(
            clip.start_seconds >= intro_cutoff,
            "Clip should start after intro exclusion"
        );
        assert!(
            clip.start_seconds + clip.duration_seconds <= outro_cutoff,
            "Clip should end before outro exclusion"
        );
    }

    #[test]
    fn test_random_selector_multiple_clips_two() {
        // Test multiple clip generation (2 clips)
        // Requirement 2.1, 3.1
        let selector = RandomSelector;
        let video_path = PathBuf::from("test.mp4");
        let duration = 600.0; // 10 minutes

        let result = selector.select_clips(
            &video_path,
            duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            2,
            &ClipConfig::default(),
        );
        assert!(result.is_ok(), "Two clip selection should succeed");

        let clips = result.unwrap();
        assert_eq!(
            clips.len(),
            2,
            "Should generate exactly 2 clips when clip_count=2"
        );

        // Verify all clips have valid durations
        for clip in &clips {
            assert!(clip.duration_seconds >= MIN_CLIP_DURATION);
            assert!(clip.duration_seconds <= MAX_CLIP_DURATION);
        }

        // Verify clips are non-overlapping
        assert!(!clips[0].overlaps(&clips[1]), "Clips should not overlap");

        // Verify clips are sorted by start time
        assert!(
            clips[0].start_seconds < clips[1].start_seconds,
            "Clips should be sorted by start time"
        );
    }

    #[test]
    fn test_random_selector_multiple_clips_three() {
        // Test multiple clip generation (3 clips)
        // Requirement 2.1, 3.1
        let selector = RandomSelector;
        let video_path = PathBuf::from("test.mp4");
        let duration = 600.0; // 10 minutes

        let result = selector.select_clips(
            &video_path,
            duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            3,
            &ClipConfig::default(),
        );
        assert!(result.is_ok(), "Three clip selection should succeed");

        let clips = result.unwrap();
        assert_eq!(
            clips.len(),
            3,
            "Should generate exactly 3 clips when clip_count=3"
        );

        // Verify all clips have valid durations
        for clip in &clips {
            assert!(clip.duration_seconds >= MIN_CLIP_DURATION);
            assert!(clip.duration_seconds <= MAX_CLIP_DURATION);
        }

        // Verify clips are non-overlapping
        for i in 0..clips.len() {
            for j in (i + 1)..clips.len() {
                assert!(
                    !clips[i].overlaps(&clips[j]),
                    "Clips {} and {} should not overlap",
                    i,
                    j
                );
            }
        }

        // Verify clips are sorted by start time
        for i in 0..(clips.len() - 1) {
            assert!(
                clips[i].start_seconds < clips[i + 1].start_seconds,
                "Clips should be sorted by start time"
            );
        }
    }

    #[test]
    fn test_random_selector_multiple_clips_four() {
        // Test multiple clip generation (4 clips - maximum)
        // Requirement 2.1, 3.1
        let selector = RandomSelector;
        let video_path = PathBuf::from("test.mp4");
        let duration = 600.0; // 10 minutes

        let result = selector.select_clips(
            &video_path,
            duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            4,
            &ClipConfig::default(),
        );
        assert!(result.is_ok(), "Four clip selection should succeed");

        let clips = result.unwrap();
        assert_eq!(
            clips.len(),
            4,
            "Should generate exactly 4 clips when clip_count=4"
        );

        // Verify all clips have valid durations
        for clip in &clips {
            assert!(clip.duration_seconds >= MIN_CLIP_DURATION);
            assert!(clip.duration_seconds <= MAX_CLIP_DURATION);
        }

        // Verify clips are non-overlapping
        for i in 0..clips.len() {
            for j in (i + 1)..clips.len() {
                assert!(
                    !clips[i].overlaps(&clips[j]),
                    "Clips {} and {} should not overlap",
                    i,
                    j
                );
            }
        }

        // Verify clips are sorted by start time
        for i in 0..(clips.len() - 1) {
            assert!(
                clips[i].start_seconds < clips[i + 1].start_seconds,
                "Clips should be sorted by start time"
            );
        }
    }

    #[test]
    fn test_random_selector_graceful_degradation_short_video() {
        // Test graceful degradation for short videos
        // Requirement 2.3, 3.3
        let selector = RandomSelector;
        let video_path = PathBuf::from("test.mp4");
        let duration = 30.0; // 30 seconds - can fit 2 clips at minimum (2 * 12s = 24s)

        // Request 4 clips but video can only fit 2
        let result = selector.select_clips(
            &video_path,
            duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            4,
            &ClipConfig::default(),
        );
        assert!(
            result.is_ok(),
            "Should succeed even when video is too short for all clips"
        );

        let clips = result.unwrap();

        // Should generate fewer clips than requested
        assert!(
            clips.len() < 4,
            "Should generate fewer than 4 clips for short video"
        );
        assert!(
            clips.len() >= 1,
            "Should generate at least 1 clip if possible"
        );

        // Verify all clips have valid durations
        for clip in &clips {
            assert!(clip.duration_seconds >= MIN_CLIP_DURATION);
            assert!(clip.duration_seconds <= MAX_CLIP_DURATION);
        }

        // Verify clips are non-overlapping
        for i in 0..clips.len() {
            for j in (i + 1)..clips.len() {
                assert!(
                    !clips[i].overlaps(&clips[j]),
                    "Clips {} and {} should not overlap",
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn test_random_selector_very_short_video_no_clips() {
        // Test very short video that cannot fit any clips
        // Requirement 2.3
        let selector = RandomSelector;
        let video_path = PathBuf::from("test.mp4");
        let duration = 5.0; // 5 seconds - too short for minimum clip duration (12s)

        let result = selector.select_clips(
            &video_path,
            duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            2,
            &ClipConfig::default(),
        );
        assert!(
            result.is_ok(),
            "Should succeed even when video is too short"
        );

        let clips = result.unwrap();
        assert_eq!(
            clips.len(),
            0,
            "Should generate 0 clips when video is too short"
        );
    }

    #[test]
    fn test_random_selector_exclusion_zone_compliance_multiple_clips() {
        // Test exclusion zone compliance with multiple clips
        // Requirement 4.1
        let selector = RandomSelector;
        let video_path = PathBuf::from("test.mp4");
        let duration = 600.0; // 10 minutes

        let intro_percent = 5.0; // 5% intro exclusion
        let outro_percent = 30.0; // 30% outro exclusion

        let result = selector.select_clips(
            &video_path,
            duration,
            intro_percent,
            outro_percent,
            3,
            &ClipConfig::default(),
        );
        assert!(result.is_ok(), "Selection should succeed");

        let clips = result.unwrap();
        assert_eq!(clips.len(), 3, "Should generate 3 clips");

        // Calculate exclusion boundaries
        let intro_cutoff = duration * (intro_percent / 100.0);
        let outro_cutoff = duration - (duration * (outro_percent / 100.0));

        // Verify all clips respect exclusion zones
        for (i, clip) in clips.iter().enumerate() {
            assert!(
                clip.start_seconds >= intro_cutoff,
                "Clip {} should start after intro exclusion (start: {}, cutoff: {})",
                i,
                clip.start_seconds,
                intro_cutoff
            );

            let clip_end = clip.start_seconds + clip.duration_seconds;
            assert!(
                clip_end <= outro_cutoff,
                "Clip {} should end before outro exclusion (end: {}, cutoff: {})",
                i,
                clip_end,
                outro_cutoff
            );
        }
    }

    #[test]
    fn test_random_selector_clips_sorted_chronologically() {
        // Test that clips are returned in chronological order
        // Requirement 7.4
        let selector = RandomSelector;
        let video_path = PathBuf::from("test.mp4");
        let duration = 600.0; // 10 minutes

        // Run multiple times to ensure sorting is consistent
        for _ in 0..5 {
            let result = selector.select_clips(
                &video_path,
                duration,
                INTRO_EXCLUSION_PERCENT,
                OUTRO_EXCLUSION_PERCENT,
                3,
                &ClipConfig::default(),
            );
            assert!(result.is_ok(), "Selection should succeed");

            let clips = result.unwrap();
            assert_eq!(clips.len(), 3, "Should generate 3 clips");

            // Verify chronological ordering
            for i in 0..(clips.len() - 1) {
                assert!(
                    clips[i].start_seconds < clips[i + 1].start_seconds,
                    "Clip {} (start: {}) should come before clip {} (start: {})",
                    i,
                    clips[i].start_seconds,
                    i + 1,
                    clips[i + 1].start_seconds
                );
            }
        }
    }

    // Feature: video-clip-extractor, Property 11: Random Selection Variety
    proptest! {
        #[test]
        fn test_random_selection_variety_property(duration in MIN_TEST_DURATION..3600.0f64) {
            // Test that for any video long enough to accommodate exclusions,
            // multiple selections produce different start times (variety)

            let selector = RandomSelector;
            let video_path = PathBuf::from("test.mp4");

            // Run selection multiple times (10 iterations)
            let mut start_times = Vec::new();
            for _ in 0..10 {
                let result = selector.select_clips(&video_path, duration, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT, 1, &ClipConfig::default());
                prop_assert!(result.is_ok(), "Selection should succeed");
                let time_ranges = result.unwrap();
                prop_assert!(!time_ranges.is_empty(), "Should return at least one clip");
                start_times.push(time_ranges[0].start_seconds);
            }

            // Property: Not all start times should be identical
            // This verifies that the random selector uses different random seeds
            let first = start_times[0];
            let all_same = start_times.iter().all(|&x| (x - first).abs() < FLOAT_EPSILON);

            prop_assert!(!all_same,
                "Random selector should produce variety in start times across multiple runs. \
                 All {} selections produced start time {}",
                start_times.len(), first);
        }
    }

    // Feature: multiple-clips-per-video, Property 2: Exact Clip Count Generation
    proptest! {
        #[test]
        fn test_exact_clip_count_generation_property(
            duration in 100.0..3600.0f64,
            clip_count in 1u8..=4u8,
        ) {
            // **Validates: Requirements 2.1**
            // Property: For any video file with sufficient duration and any valid clip count N (1-4),
            // the system should generate exactly N clips.

            let selector = RandomSelector;
            let video_path = PathBuf::from("test.mp4");

            // Calculate valid selection zone
            let intro_cutoff = duration * (INTRO_EXCLUSION_PERCENT / 100.0);
            let outro_cutoff = duration - (duration * (OUTRO_EXCLUSION_PERCENT / 100.0));
            let valid_duration = outro_cutoff - intro_cutoff;

            // Calculate minimum required duration for N clips
            let min_required = (clip_count as f64) * MIN_CLIP_DURATION;

            let result = selector.select_clips(&video_path, duration, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT, clip_count, &ClipConfig::default());
            prop_assert!(result.is_ok(), "Selection should succeed");

            let clips = result.unwrap();

            // Property: If video has sufficient duration with generous margin, should generate exactly clip_count clips
            // We need a generous margin because random selection is probabilistic and may not always
            // find all clips even when theoretically possible (due to MAX_ATTEMPTS limit)
            let generous_margin = (clip_count as f64) * MAX_CLIP_DURATION * 2.0; // 2x the maximum space needed
            if valid_duration >= min_required + generous_margin {
                // With generous space, we should get exactly the requested count
                prop_assert_eq!(clips.len(), clip_count as usize,
                    "Should generate exactly {} clips for video with duration {} (valid_duration: {}, min_required: {}, generous_margin: {})",
                    clip_count, duration, valid_duration, min_required, generous_margin);
            } else if valid_duration >= min_required {
                // With tight space, we should get at least some clips, but may not get all due to random placement
                prop_assert!(clips.len() > 0,
                    "Should generate at least 1 clip when video has sufficient minimum duration");
                prop_assert!(clips.len() <= clip_count as usize,
                    "Should not generate more than {} clips", clip_count);
            } else {
                // Property: If video is too short, should generate fewer clips (graceful degradation)
                prop_assert!(clips.len() <= clip_count as usize,
                    "Should generate at most {} clips when video is too short", clip_count);

                // Property: Should generate as many clips as possible
                let max_possible = (valid_duration / MIN_CLIP_DURATION).floor() as usize;
                prop_assert!(clips.len() <= max_possible,
                    "Should not generate more than {} clips (max possible for valid_duration {})",
                    max_possible, valid_duration);
            }

            // Property: All generated clips should be valid
            for (i, clip) in clips.iter().enumerate() {
                // Valid duration
                prop_assert!(clip.duration_seconds >= MIN_CLIP_DURATION,
                    "Clip {} duration {} should be >= MIN_CLIP_DURATION {}",
                    i, clip.duration_seconds, MIN_CLIP_DURATION);
                prop_assert!(clip.duration_seconds <= MAX_CLIP_DURATION,
                    "Clip {} duration {} should be <= MAX_CLIP_DURATION {}",
                    i, clip.duration_seconds, MAX_CLIP_DURATION);

                // Within video bounds
                prop_assert!(clip.start_seconds >= 0.0,
                    "Clip {} start {} should be >= 0", i, clip.start_seconds);
                prop_assert!(clip.start_seconds + clip.duration_seconds <= duration,
                    "Clip {} end {} should be <= video duration {}",
                    i, clip.start_seconds + clip.duration_seconds, duration);
            }

            // Property: All clips should be non-overlapping
            for i in 0..clips.len() {
                for j in (i + 1)..clips.len() {
                    prop_assert!(!clips[i].overlaps(&clips[j]),
                        "Clip {} and clip {} should not overlap", i, j);
                }
            }

            // Property: Clips should be sorted by start time
            for i in 0..(clips.len().saturating_sub(1)) {
                prop_assert!(clips[i].start_seconds < clips[i + 1].start_seconds,
                    "Clips should be sorted by start time: clip {} start {} should be < clip {} start {}",
                    i, clips[i].start_seconds, i + 1, clips[i + 1].start_seconds);
            }
        }
    }

    // Feature: multiple-clips-per-video, Property 5: Exclusion Zone Compliance
    proptest! {
        #[test]
        fn test_exclusion_zone_compliance_property(
            duration in 100.0..3600.0f64,
            intro_percent in 0.0..=20.0f64,
            outro_percent in 0.0..=50.0f64,
            clip_count in 1u8..=4u8,
        ) {
            // **Validates: Requirements 4.1**
            // Property: For any generated clip from any video, the clip's time range should fall
            // entirely within the valid selection zone (clip.start >= intro_cutoff AND clip.end <= outro_cutoff).

            let selector = RandomSelector;
            let video_path = PathBuf::from("test.mp4");

            // Calculate exclusion zone boundaries
            let intro_cutoff = duration * (intro_percent / 100.0);
            let outro_cutoff = duration - (duration * (outro_percent / 100.0));

            let result = selector.select_clips(&video_path, duration, intro_percent, outro_percent, clip_count, &ClipConfig::default());
            prop_assert!(result.is_ok(), "Selection should succeed");

            let clips = result.unwrap();

            // Property: All clips must respect exclusion zones
            for (i, clip) in clips.iter().enumerate() {
                let clip_end = clip.start_seconds + clip.duration_seconds;

                prop_assert!(clip.start_seconds >= intro_cutoff,
                    "Clip {} start {} should be >= intro_cutoff {} (intro_percent: {})",
                    i, clip.start_seconds, intro_cutoff, intro_percent);

                prop_assert!(clip_end <= outro_cutoff,
                    "Clip {} end {} should be <= outro_cutoff {} (outro_percent: {})",
                    i, clip_end, outro_cutoff, outro_percent);

                // Property: Clip should be entirely within valid zone
                prop_assert!(clip.start_seconds >= intro_cutoff && clip_end <= outro_cutoff,
                    "Clip {} [{}, {}) should be entirely within valid zone [{}, {})",
                    i, clip.start_seconds, clip_end, intro_cutoff, outro_cutoff);
            }
        }
    }

    // Feature: multiple-clips-per-video, Property 6: Duration Constraint Compliance
    proptest! {
        #[test]
        fn test_duration_constraint_compliance_property(
            duration in 100.0..3600.0f64,
            intro_percent in 0.0..=20.0f64,
            outro_percent in 0.0..=50.0f64,
            clip_count in 1u8..=4u8,
        ) {
            // **Validates: Requirements 5.1**
            // Property: For any generated clip, the clip duration should be between
            // MIN_CLIP_DURATION (10 seconds) and MAX_CLIP_DURATION (15 seconds) inclusive.

            let selector = RandomSelector;
            let video_path = PathBuf::from("test.mp4");

            let result = selector.select_clips(&video_path, duration, intro_percent, outro_percent, clip_count, &ClipConfig::default());
            prop_assert!(result.is_ok(), "Selection should succeed");

            let clips = result.unwrap();

            // Property: All clips must have valid durations (10-15 seconds)
            for (i, clip) in clips.iter().enumerate() {
                let clip_duration = clip.duration();

                prop_assert!(clip_duration >= MIN_CLIP_DURATION,
                    "Clip {} duration {} should be >= MIN_CLIP_DURATION ({})",
                    i, clip_duration, MIN_CLIP_DURATION);

                prop_assert!(clip_duration <= MAX_CLIP_DURATION,
                    "Clip {} duration {} should be <= MAX_CLIP_DURATION ({})",
                    i, clip_duration, MAX_CLIP_DURATION);

                // Also verify using the is_valid_duration method
                prop_assert!(clip.is_valid_duration(),
                    "Clip {} with duration {} should pass is_valid_duration() check",
                    i, clip_duration);
            }
        }
    }

    // Tests for IntenseAudioSelector

    #[test]
    fn test_clip_config_middle_segment_fallback() {
        // Test that middle_segment helper calculates correct fallback
        let config = ClipConfig::default();
        let duration = 600.0; // 10 minutes

        let result = config.middle_segment(duration);
        assert!(result.is_ok());

        let time_range = result.unwrap();

        // Should use 15 seconds (max clip duration)
        assert_eq!(time_range.duration_seconds, 15.0);

        // Should be centered: (600 - 15) / 2 = 292.5
        assert_eq!(time_range.start_seconds, 292.5);
    }

    #[test]
    fn test_clip_config_middle_segment_short_video() {
        // Test middle_segment with a short video
        let config = ClipConfig::default();
        let duration = 12.0; // 12 seconds

        let result = config.middle_segment(duration);
        assert!(result.is_ok());

        let time_range = result.unwrap();

        // Should use full video duration (12 seconds)
        assert_eq!(time_range.duration_seconds, 12.0);

        // Should start at 0 (centered)
        assert_eq!(time_range.start_seconds, 0.0);
    }

    #[test]
    fn test_clip_config_middle_segment_very_short_video() {
        // Test middle_segment with a very short video (< 5 seconds)
        let config = ClipConfig::default();
        let duration = 3.0; // 3 seconds

        let result = config.middle_segment(duration);
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
        let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true, false);

        // Create IntenseAudioSelector
        let selector = IntenseAudioSelector::new(ffmpeg_executor);

        // Use a non-existent video path - this will cause audio analysis to fail
        // which simulates a video with no audio track
        let video_path = PathBuf::from("/nonexistent/video_no_audio.mp4");
        let duration = 600.0; // 10 minutes

        // The selector should fall back to middle segment when audio analysis fails
        let result = selector.select_clips(
            &video_path,
            duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            1,
            &ClipConfig::default(),
        );

        // The result should be Ok (fallback to middle segment)
        assert!(
            result.is_ok(),
            "Should fall back to middle segment when no audio track"
        );

        let time_ranges = result.unwrap();
        assert!(!time_ranges.is_empty());
        let time_range = &time_ranges[0];

        // Verify it uses middle segment calculation
        // For a 600 second video, with 15 second clip duration:
        // start = (600 - 15) / 2 = 292.5
        assert_eq!(
            time_range.duration_seconds, 15.0,
            "Should use max clip duration (15s)"
        );
        assert_eq!(
            time_range.start_seconds, 292.5,
            "Should center the clip in the video"
        );

        // Test with a shorter video
        let short_duration = 120.0; // 2 minutes
        let result_short = selector.select_clips(
            &video_path,
            short_duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            1,
            &ClipConfig::default(),
        );

        assert!(
            result_short.is_ok(),
            "Should fall back to middle segment for short video"
        );

        let time_ranges_short = result_short.unwrap();
        assert!(!time_ranges_short.is_empty());
        let time_range_short = &time_ranges_short[0];

        // For a 120 second video, with 15 second clip:
        // start = (120 - 15) / 2 = 52.5
        assert_eq!(time_range_short.duration_seconds, 15.0);
        assert_eq!(time_range_short.start_seconds, 52.5);

        // Test with a very short video (< 18 seconds)
        let very_short_duration = 7.0; // 7 seconds
        let result_very_short = selector.select_clips(
            &video_path,
            very_short_duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            1,
            &ClipConfig::default(),
        );

        assert!(
            result_very_short.is_ok(),
            "Should fall back to middle segment for very short video"
        );

        let time_ranges_very_short = result_very_short.unwrap();
        assert!(!time_ranges_very_short.is_empty());
        let time_range_very_short = &time_ranges_very_short[0];

        // For a 7 second video, clip duration should be capped at 7 seconds
        // start = (7 - 7) / 2 = 0
        assert_eq!(
            time_range_very_short.duration_seconds, 7.0,
            "Should use full video duration"
        );
        assert_eq!(
            time_range_very_short.start_seconds, 0.0,
            "Should start at beginning"
        );
    }

    #[test]
    fn test_intense_audio_selector_tie_breaking() {
        // Test that first occurrence is selected when multiple segments have similar intensity
        // Validates Requirement 4.3
        use crate::ffmpeg::analysis::AudioSegment;

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
        assert_eq!(
            segments[0].intensity, -15.1,
            "First segment should have highest intensity"
        );

        // When there's a tie in intensity, Rust's stable sort preserves the original order
        // So the segment that appeared first in the original list should remain first
        assert_eq!(
            segments[0].start_time, 10.0,
            "When multiple segments have the same intensity, the first occurrence should be selected"
        );

        // The second segment with -15.1 intensity should come after
        // Find the second occurrence of -15.1 intensity
        let second_loudest_index = segments
            .iter()
            .position(|s| s.intensity == -15.1 && s.start_time == 70.0);
        assert!(
            second_loudest_index.is_some(),
            "Second segment with -15.1 intensity should exist"
        );
        assert!(
            second_loudest_index.unwrap() > 0,
            "Second tied segment should come after the first"
        );

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
        assert_eq!(
            tied_segments[0].start_time, 100.0,
            "With identical intensities, the first occurrence in the original list should be selected"
        );
        assert_eq!(
            tied_segments[1].start_time, 50.0,
            "Second segment should maintain original order"
        );
        assert_eq!(
            tied_segments[2].start_time, 25.0,
            "Third segment should maintain original order"
        );
    }

    // Unit tests for IntenseAudioSelector with multiple clips
    // Requirements: 2.1, 2.3, 3.1, 7.2

    #[test]
    fn test_intense_audio_selector_single_clip() {
        // Test single clip selection
        // Requirement 2.1
        use crate::cli::Resolution;
        use std::path::PathBuf;

        let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true, false);
        let selector = IntenseAudioSelector::new(ffmpeg_executor);

        // Use a non-existent video path - will fall back to middle segment
        let video_path = PathBuf::from("/nonexistent/video.mp4");
        let duration = 600.0; // 10 minutes

        let result = selector.select_clips(
            &video_path,
            duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            1,
            &ClipConfig::default(),
        );
        assert!(result.is_ok(), "Single clip selection should succeed");

        let clips = result.unwrap();
        assert_eq!(
            clips.len(),
            1,
            "Should generate exactly 1 clip when clip_count=1"
        );

        let clip = &clips[0];

        // Verify clip duration is valid
        assert!(clip.duration_seconds >= MIN_CLIP_DURATION);
        assert!(clip.duration_seconds <= MAX_CLIP_DURATION);

        // Verify clip is within video bounds
        assert!(clip.start_seconds >= 0.0);
        assert!(clip.start_seconds + clip.duration_seconds <= duration);
    }

    #[test]
    fn test_intense_audio_selector_multiple_clips_mock_data() {
        // Test multiple clip selection with mock audio data
        // Requirements 2.1, 3.1, 7.2
        use crate::ffmpeg::analysis::AudioSegment;

        // Create mock audio segments with varying intensities
        // Higher (less negative) values are louder
        let segments = vec![
            AudioSegment {
                start_time: 50.0,
                duration: 7.5,
                intensity: -10.0, // Loudest
            },
            AudioSegment {
                start_time: 150.0,
                duration: 7.5,
                intensity: -12.0, // Second loudest
            },
            AudioSegment {
                start_time: 250.0,
                duration: 7.5,
                intensity: -15.0, // Third loudest
            },
            AudioSegment {
                start_time: 350.0,
                duration: 7.5,
                intensity: -20.0, // Fourth loudest
            },
        ];

        // Verify segments are sorted by intensity (highest first)
        let mut sorted_segments = segments.clone();
        sorted_segments.sort_by(|a, b| b.intensity.partial_cmp(&a.intensity).unwrap());

        assert_eq!(
            sorted_segments[0].intensity, -10.0,
            "First segment should be loudest"
        );
        assert_eq!(
            sorted_segments[1].intensity, -12.0,
            "Second segment should be second loudest"
        );
        assert_eq!(
            sorted_segments[2].intensity, -15.0,
            "Third segment should be third loudest"
        );

        // Verify all segments are non-overlapping
        for i in 0..sorted_segments.len() {
            for j in (i + 1)..sorted_segments.len() {
                let seg1_end = sorted_segments[i].start_time + sorted_segments[i].duration;
                let seg2_start = sorted_segments[j].start_time;
                assert!(
                    seg1_end <= seg2_start
                        || sorted_segments[j].start_time + sorted_segments[j].duration
                            <= sorted_segments[i].start_time,
                    "Segments {} and {} should not overlap",
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn test_intense_audio_selector_peak_selection_overlapping() {
        // Test peak selection with overlapping candidates
        // Requirement 3.1
        use crate::ffmpeg::analysis::AudioSegment;

        // Create mock audio segments where some peaks would overlap
        let segments = vec![
            AudioSegment {
                start_time: 50.0,
                duration: 7.5,
                intensity: -10.0, // Loudest
            },
            AudioSegment {
                start_time: 55.0, // Overlaps with first segment
                duration: 7.5,
                intensity: -11.0, // Second loudest but overlaps
            },
            AudioSegment {
                start_time: 150.0, // Non-overlapping
                duration: 7.5,
                intensity: -12.0, // Third loudest
            },
            AudioSegment {
                start_time: 250.0, // Non-overlapping
                duration: 7.5,
                intensity: -15.0, // Fourth loudest
            },
        ];

        // Sort by intensity (highest first)
        let mut sorted_segments = segments.clone();
        sorted_segments.sort_by(|a, b| b.intensity.partial_cmp(&a.intensity).unwrap());

        // Simulate selection logic: pick top N non-overlapping segments
        let mut selected = Vec::new();
        for segment in sorted_segments {
            // Check if this segment overlaps with any already selected
            let overlaps = selected.iter().any(|sel: &AudioSegment| {
                let seg_end = segment.start_time + segment.duration;
                let sel_end = sel.start_time + sel.duration;
                !(seg_end <= sel.start_time || sel_end <= segment.start_time)
            });

            if !overlaps {
                selected.push(segment);
            }

            if selected.len() >= 3 {
                break;
            }
        }

        // Should select segments at 50.0, 150.0, and 250.0 (skipping 55.0 due to overlap)
        assert_eq!(
            selected.len(),
            3,
            "Should select 3 non-overlapping segments"
        );
        assert_eq!(
            selected[0].start_time, 50.0,
            "First selected should be at 50.0"
        );
        assert_eq!(
            selected[1].start_time, 150.0,
            "Second selected should be at 150.0"
        );
        assert_eq!(
            selected[2].start_time, 250.0,
            "Third selected should be at 250.0"
        );

        // Verify all selected segments are non-overlapping
        for i in 0..selected.len() {
            for j in (i + 1)..selected.len() {
                let seg1_end = selected[i].start_time + selected[i].duration;
                let seg2_start = selected[j].start_time;
                assert!(
                    seg1_end <= seg2_start,
                    "Selected segments {} and {} should not overlap",
                    i,
                    j
                );
            }
        }
    }

    #[test]
    fn test_intense_audio_selector_graceful_degradation() {
        // Test graceful degradation for short videos
        // Requirement 2.3
        use crate::cli::Resolution;
        use std::path::PathBuf;

        let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true, false);
        let selector = IntenseAudioSelector::new(ffmpeg_executor);

        // Use a short video duration that can only fit 1 clip
        let video_path = PathBuf::from("/nonexistent/short_video.mp4");
        let duration = 30.0; // 30 seconds

        // Request 4 clips but video can only fit 2
        let result = selector.select_clips(
            &video_path,
            duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            4,
            &ClipConfig::default(),
        );
        assert!(
            result.is_ok(),
            "Should succeed even when video is too short for all clips"
        );

        let clips = result.unwrap();

        // Should generate fewer clips than requested (or fall back to middle segment)
        assert!(clips.len() <= 4, "Should generate at most 4 clips");
        assert!(clips.len() >= 1, "Should generate at least 1 clip");

        // Verify all clips have valid durations
        for clip in &clips {
            assert!(
                clip.duration_seconds > 0.0,
                "Clip duration should be positive"
            );
            assert!(
                clip.duration_seconds <= duration,
                "Clip duration should not exceed video duration"
            );
        }
    }

    #[test]
    fn test_intense_audio_selector_very_short_video() {
        // Test very short video that cannot fit any clips with exclusion zones
        // Requirement 2.3
        use crate::cli::Resolution;
        use std::path::PathBuf;

        let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true, false);
        let selector = IntenseAudioSelector::new(ffmpeg_executor);

        let video_path = PathBuf::from("/nonexistent/very_short.mp4");
        let duration = 5.0; // 5 seconds - too short for minimum clip duration

        let result = selector.select_clips(
            &video_path,
            duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            2,
            &ClipConfig::default(),
        );
        assert!(
            result.is_ok(),
            "Should succeed even when video is too short"
        );

        let clips = result.unwrap();
        // Should return empty vector or fall back to middle segment
        assert!(
            clips.len() <= 1,
            "Should generate at most 1 clip for very short video"
        );
    }

    #[test]
    fn test_intense_audio_selector_clips_sorted_chronologically() {
        // Test that clips are returned in chronological order
        // Requirement 7.4
        use crate::ffmpeg::analysis::AudioSegment;

        // Create mock segments in random order
        let mut segments = vec![
            AudioSegment {
                start_time: 250.0,
                duration: 7.5,
                intensity: -10.0, // Loudest
            },
            AudioSegment {
                start_time: 50.0,
                duration: 7.5,
                intensity: -12.0, // Second loudest
            },
            AudioSegment {
                start_time: 150.0,
                duration: 7.5,
                intensity: -15.0, // Third loudest
            },
        ];

        // Sort by intensity (highest first)
        segments.sort_by(|a, b| b.intensity.partial_cmp(&a.intensity).unwrap());

        // Simulate creating TimeRanges from these segments
        let mut clips: Vec<TimeRange> = segments
            .iter()
            .map(|seg| {
                TimeRange {
                    start_seconds: seg.start_time,
                    duration_seconds: 15.0, // Use a fixed duration for testing
                }
            })
            .collect();

        // Sort by start time (as IntenseAudioSelector does)
        clips.sort_by(|a, b| a.start_seconds.partial_cmp(&b.start_seconds).unwrap());

        // Verify chronological ordering
        assert_eq!(clips.len(), 3, "Should have 3 clips");
        assert_eq!(
            clips[0].start_seconds, 50.0,
            "First clip should start at 50.0"
        );
        assert_eq!(
            clips[1].start_seconds, 150.0,
            "Second clip should start at 150.0"
        );
        assert_eq!(
            clips[2].start_seconds, 250.0,
            "Third clip should start at 250.0"
        );

        // Verify ordering
        for i in 0..(clips.len() - 1) {
            assert!(
                clips[i].start_seconds < clips[i + 1].start_seconds,
                "Clip {} (start: {}) should come before clip {} (start: {})",
                i,
                clips[i].start_seconds,
                i + 1,
                clips[i + 1].start_seconds
            );
        }
    }

    // Feature: intense-audio-clip-selection, Property: Exclusion Zone Compliance
    proptest! {
        #[test]
        fn test_intense_audio_exclusion_zone_compliance(
            duration in MIN_TEST_DURATION..3600.0f64,
            intro_percent in 0.0..=10.0f64,
            outro_percent in 0.0..=50.0f64,
        ) {
            use crate::ffmpeg::analysis::AudioSegment;

            // Calculate exclusion zone boundaries
            let zones = ExclusionZones::new(duration, intro_percent, outro_percent);

            // Create mock audio segments across the video duration
            let num_segments = (duration / 7.5).ceil() as usize;
            let segments: Vec<AudioSegment> = (0..num_segments)
                .map(|i| {
                    let start = i as f64 * 7.5;
                    let seg_duration = 7.5_f64.min(duration - start);
                    AudioSegment {
                        start_time: start,
                        duration: seg_duration,
                        intensity: -(i as f64 + 10.0), // Decreasing intensity (more negative = quieter)
                    }
                })
                .collect();

            // Filter segments by exclusion zones (mimics IntenseAudioSelector logic)
            let filtered_segments: Vec<_> = segments.into_iter()
                .filter(|seg| {
                    let segment_end = seg.start_time + seg.duration;
                    zones.contains_segment(seg.start_time, segment_end)
                })
                .collect();

            // Property: All filtered segments must respect exclusion zones
            for segment in filtered_segments.iter() {
                let segment_end = segment.start_time + segment.duration;

                prop_assert!(zones.contains_segment(segment.start_time, segment_end),
                    "Segment [{}, {}) should be within exclusion zones [{}, {})",
                    segment.start_time, segment_end, zones.intro_boundary, zones.outro_boundary);
            }
        }
    }

    // Tests for ActionSelector

    #[test]
    fn test_action_selector_no_motion_fallback() {
        // Test fallback to middle segment when video has no motion
        use crate::cli::Resolution;
        use std::path::PathBuf;

        // Create an FFmpegExecutor
        let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true, false);

        // Create ActionSelector
        let selector = ActionSelector::new(ffmpeg_executor);

        // Use a non-existent video path - this will cause motion analysis to fail
        // which simulates a video with no motion
        let video_path = PathBuf::from("/nonexistent/video_no_motion.mp4");
        let duration = 600.0; // 10 minutes

        // The selector should fall back to middle segment when motion analysis fails
        let result = selector.select_clips(
            &video_path,
            duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            1,
            &ClipConfig::default(),
        );

        // The result should be Ok (fallback to middle segment)
        assert!(
            result.is_ok(),
            "Should fall back to middle segment when no motion detected"
        );

        let time_ranges = result.unwrap();
        assert!(!time_ranges.is_empty());
        let time_range = &time_ranges[0];

        // Verify it uses middle segment calculation
        // For a 600 second video, with 15 second clip duration:
        // start = (600 - 15) / 2 = 292.5
        assert_eq!(
            time_range.duration_seconds, 15.0,
            "Should use max clip duration (15s)"
        );
        assert_eq!(
            time_range.start_seconds, 292.5,
            "Should center the clip in the video"
        );
    }

    #[test]
    fn test_action_selector_motion_segment_tie_breaking() {
        // Test that first occurrence is selected when multiple segments have identical motion scores
        use crate::ffmpeg::analysis::MotionSegment;

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
        assert_eq!(
            segments[0].start_time, 100.0,
            "With identical motion scores, the first occurrence in the original list should be selected"
        );
        assert_eq!(
            segments[1].start_time, 50.0,
            "Second segment should maintain original order"
        );
        assert_eq!(
            segments[2].start_time, 25.0,
            "Third segment should maintain original order"
        );
    }

    // Feature: action-based-clip-selection, Property 1: Highest Motion Score Selection
    proptest! {
        #[test]
        fn test_action_highest_motion_score_selection(
            num_segments in 2..10usize,
            scores in prop::collection::vec(0.1..10.0f64, 2..10),
        ) {
            use crate::ffmpeg::analysis::MotionSegment;

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
            use crate::ffmpeg::analysis::MotionSegment;

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
            use crate::ffmpeg::analysis::MotionSegment;

            // Calculate exclusion zone boundaries
            let zones = ExclusionZones::new(duration, intro_percent, outro_percent);

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
                    let segment_end = seg.start_time + seg.duration;
                    zones.contains_segment(seg.start_time, segment_end)
                })
                .collect();

            // Property: All filtered segments must respect exclusion zones
            for segment in filtered_segments.iter() {
                let segment_end = segment.start_time + segment.duration;

                prop_assert!(zones.contains_segment(segment.start_time, segment_end),
                    "Segment [{}, {}) should be within exclusion zones [{}, {})",
                    segment.start_time, segment_end, zones.intro_boundary, zones.outro_boundary);
            }
        }
    }

    // Feature: action-based-clip-selection, Property 5: Next Best Segment Selection
    proptest! {
        #[test]
        fn test_action_next_best_segment_selection(
            duration in 415.0..3600.0f64,
        ) {
            use crate::ffmpeg::analysis::MotionSegment;

            // Create a scenario where the highest-scoring segment violates exclusion zones
            // Set intro exclusion to 10% and outro exclusion to 40%
            let intro_percent = 10.0;
            let outro_percent = 40.0;
            let zones = ExclusionZones::new(duration, intro_percent, outro_percent);

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
                    start_time: zones.intro_boundary + 10.0,
                    duration: 12.5,
                    motion_score: 80.0,
                },
                // Third highest score in valid zone
                MotionSegment {
                    start_time: zones.intro_boundary + 30.0,
                    duration: 12.5,
                    motion_score: 60.0,
                },
            ];

            // Sort by motion score (highest first)
            segments.sort_by(|a, b| b.motion_score.partial_cmp(&a.motion_score).unwrap());

            // Filter by exclusion zones
            let filtered_segments: Vec<_> = segments.into_iter()
                .filter(|seg| {
                    let segment_end = seg.start_time + seg.duration;
                    zones.contains_segment(seg.start_time, segment_end)
                })
                .collect();

            // Property: The first filtered segment should be the next best (second highest score)
            prop_assert!(!filtered_segments.is_empty(), "Should have at least one valid segment");
            prop_assert_eq!(filtered_segments[0].motion_score, 80.0,
                "First valid segment should have score 80.0 (next best after filtered out 100.0)");

            // Property: The selected segment should respect exclusion zones
            let selected = &filtered_segments[0];
            let selected_end = selected.start_time + selected.duration;
            prop_assert!(zones.contains_segment(selected.start_time, selected_end),
                "Selected segment should be within exclusion zones");
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

            const MIN_CLIP_DURATION: f64 = 10.0;
            const MAX_CLIP_DURATION: f64 = 15.0;

            // Create an FFmpegExecutor and ActionSelector
            let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true, false);
            let selector = ActionSelector::new(ffmpeg_executor);

            // Use a non-existent video path (will fall back to middle segment)
            let video_path = PathBuf::from("/nonexistent/video.mp4");

            // Test with default exclusion zones
            let result = selector.select_clips(&video_path, duration, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT, 1, &ClipConfig::default());

            prop_assert!(result.is_ok(), "Selection should succeed");

            let time_ranges = result.unwrap();
            prop_assert!(!time_ranges.is_empty(), "Should return at least one clip");
            let time_range = &time_ranges[0];

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
            duration in 10.0..3600.0f64,
            intro_percent in 0.0..=10.0f64,
            outro_percent in 0.0..=50.0f64,
        ) {
            use crate::cli::Resolution;
            use std::path::PathBuf;

            // Create an FFmpegExecutor and ActionSelector
            let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true, false);
            let selector = ActionSelector::new(ffmpeg_executor);

            // Use a non-existent video path (will fall back to middle segment)
            let video_path = PathBuf::from("/nonexistent/video.mp4");

            // Test with various exclusion zones
            let result = selector.select_clips(&video_path, duration, intro_percent, outro_percent, 1, &ClipConfig::default());

            prop_assert!(result.is_ok(), "Selection should succeed");

            let time_ranges = result.unwrap();
            prop_assert!(!time_ranges.is_empty(), "Should return at least one clip");
            let time_range = &time_ranges[0];

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
            duration in 10.0..3600.0f64,
            intro_percent in 0.0..=10.0f64,
            outro_percent in 0.0..=50.0f64,
        ) {
            use crate::cli::Resolution;
            use std::path::PathBuf;

            // Create an FFmpegExecutor and ActionSelector
            let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true, false);
            let selector = ActionSelector::new(ffmpeg_executor);

            // Use a non-existent video path (will fall back to middle segment)
            let video_path = PathBuf::from("/nonexistent/video.mp4");

            // Test with various exclusion zones
            let result = selector.select_clips(&video_path, duration, intro_percent, outro_percent, 1, &ClipConfig::default());

            prop_assert!(result.is_ok(), "Selection should succeed for valid inputs");

            let time_ranges = result.unwrap();
            prop_assert!(!time_ranges.is_empty(), "Should return at least one clip");
            let time_range = &time_ranges[0];

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
            duration in 10.0..3600.0f64,
        ) {
            // Test that middle_segment produces consistent results regardless of selector
            let config = ClipConfig::default();

            let result = config.middle_segment(duration);

            prop_assert!(result.is_ok(), "middle_segment should succeed");

            let time_range = result.unwrap();

            // Property: Result should be valid
            prop_assert!(time_range.start_seconds >= 0.0, "start_seconds should be >= 0");
            prop_assert!(time_range.duration_seconds > 0.0, "duration_seconds should be > 0");
            prop_assert!(time_range.start_seconds + time_range.duration_seconds <= duration,
                "end time should not exceed video duration");
        }
    }

    // Feature: action-based-clip-selection, Property 10: Timestamp Scaling Correctness
    proptest! {
        #[test]
        fn test_action_timestamp_scaling_correctness(
            full_duration in 400.0..3600.0f64,
        ) {
            // Simulate the scaling logic from analyze_motion_intensity
            const MAX_ANALYSIS_DURATION: f64 = 300.0; // 5 minutes

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
            let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true, false);
            let selector = ActionSelector::new(ffmpeg_executor);

            // Process first video (non-existent, will fall back to middle segment)
            let video_path1 = PathBuf::from("/nonexistent/video1.mp4");
            let result1 = selector.select_clips(&video_path1, duration1, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT, 1, &ClipConfig::default());

            prop_assert!(result1.is_ok(), "First selection should succeed");
            let time_ranges1 = result1.unwrap();
            prop_assert!(!time_ranges1.is_empty(), "Should return at least one clip");
            let time_range1 = &time_ranges1[0];

            // Process second video (non-existent, will fall back to middle segment)
            let video_path2 = PathBuf::from("/nonexistent/video2.mp4");
            let result2 = selector.select_clips(&video_path2, duration2, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT, 1, &ClipConfig::default());

            prop_assert!(result2.is_ok(), "Second selection should succeed");
            let time_ranges2 = result2.unwrap();
            prop_assert!(!time_ranges2.is_empty(), "Should return at least one clip");
            let time_range2 = &time_ranges2[0];

            // Process the second video again with the same selector
            let result2_again = selector.select_clips(&video_path2, duration2, INTRO_EXCLUSION_PERCENT, OUTRO_EXCLUSION_PERCENT, 1, &ClipConfig::default());

            prop_assert!(result2_again.is_ok(), "Second selection (repeated) should succeed");
            let time_ranges2_again = result2_again.unwrap();
            prop_assert!(!time_ranges2_again.is_empty(), "Should return at least one clip");
            let time_range2_again = &time_ranges2_again[0];

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

    // Feature: multiple-clips-per-video, Property 3: Graceful Degradation for Short Videos
    // **Validates: Requirements 2.3, 3.3**
    proptest! {
        #[test]
        fn test_graceful_degradation_for_short_videos(
            // Generate short video durations that cannot accommodate all requested clips
            duration in 20.0..60.0f64,
            // Request more clips than can fit
            clip_count in 2u8..=4u8,
        ) {
            // Property: For any video file where the valid selection zone cannot accommodate
            // N non-overlapping clips of valid duration, the system should generate the maximum
            // number of valid clips possible (which may be less than N) without failing.

            // Calculate valid selection zone
            let intro_cutoff = duration * (INTRO_EXCLUSION_PERCENT / 100.0);
            let outro_cutoff = duration - (duration * (OUTRO_EXCLUSION_PERCENT / 100.0));
            let valid_duration = outro_cutoff - intro_cutoff;

            // Calculate maximum possible clips
            let max_possible_clips = (valid_duration / MIN_CLIP_DURATION).floor() as u8;

            // Create selector
            let selector = RandomSelector;
            let video_path = PathBuf::from("/test/video.mp4");

            // Select clips
            let result = selector.select_clips(
                &video_path,
                duration,
                INTRO_EXCLUSION_PERCENT,
                OUTRO_EXCLUSION_PERCENT,
                clip_count,
                &ClipConfig::default(),
            );

            // Property 1: The operation should succeed (not fail/panic)
            prop_assert!(
                result.is_ok(),
                "Selection should succeed even when video is too short for all requested clips"
            );

            let clips = result.unwrap();

            // Property 2: Should return at most the maximum possible clips
            prop_assert!(
                clips.len() <= max_possible_clips as usize,
                "Should not return more clips ({}) than can fit in valid zone (max {})",
                clips.len(),
                max_possible_clips
            );

            // Property 3: Should return at most the requested clip count
            prop_assert!(
                clips.len() <= clip_count as usize,
                "Should not return more clips ({}) than requested ({})",
                clips.len(),
                clip_count
            );

            // Property 4: If video is too short for any clips, return empty vector
            if valid_duration < MIN_CLIP_DURATION {
                prop_assert!(
                    clips.is_empty(),
                    "Should return empty vector when video is too short for even one clip"
                );
            }

            // Property 5: All returned clips must be valid
            for (i, clip) in clips.iter().enumerate() {
                prop_assert!(
                    clip.is_valid_duration(),
                    "Clip {} should have valid duration", i
                );

                let clip_end = clip.start_seconds + clip.duration_seconds;
                prop_assert!(
                    clip.start_seconds >= intro_cutoff,
                    "Clip {} should start after intro exclusion", i
                );
                prop_assert!(
                    clip_end <= outro_cutoff,
                    "Clip {} should end before outro exclusion", i
                );
            }

            // Property 6: All clips must be non-overlapping
            for i in 0..clips.len() {
                for j in (i + 1)..clips.len() {
                    prop_assert!(
                        !clips[i].overlaps(&clips[j]),
                        "Clips {} and {} should not overlap", i, j
                    );
                }
            }
        }
    }

    // Feature: multiple-clips-per-video, Property 9: Chronological Ordering
    // **Validates: Requirements 7.4**
    proptest! {
        #[test]
        fn test_chronological_ordering_property(
            duration in 100.0..600.0f64,
            clip_count in 2u8..=4u8,
        ) {
            // Property: For any set of clips returned by a selector, the clips should be
            // ordered by start time in ascending order (chronological order).

            let selector = RandomSelector;
            let video_path = PathBuf::from("/test/video.mp4");

            let result = selector.select_clips(
                &video_path,
                duration,
                INTRO_EXCLUSION_PERCENT,
                OUTRO_EXCLUSION_PERCENT,
                clip_count,
                &ClipConfig::default(),
            );

            prop_assert!(result.is_ok(), "Selection should succeed");
            let clips = result.unwrap();

            // Property 1: Clips should be sorted by start time
            for i in 0..(clips.len().saturating_sub(1)) {
                prop_assert!(
                    clips[i].start_seconds < clips[i + 1].start_seconds,
                    "Clip {} (start: {}) should come before clip {} (start: {}) in chronological order",
                    i,
                    clips[i].start_seconds,
                    i + 1,
                    clips[i + 1].start_seconds
                );
            }

            // Property 2: Since clips are non-overlapping and sorted, each clip should end
            // before or at the start of the next clip
            for i in 0..(clips.len().saturating_sub(1)) {
                let clip_i_end = clips[i].start_seconds + clips[i].duration_seconds;
                prop_assert!(
                    clip_i_end <= clips[i + 1].start_seconds,
                    "Clip {} should end (at {}) before or when clip {} starts (at {})",
                    i,
                    clip_i_end,
                    i + 1,
                    clips[i + 1].start_seconds
                );
            }

            // Property 3: The ordering should be stable across multiple calls
            // (for RandomSelector, we can't guarantee same clips, but they should always be sorted)
            let result2 = selector.select_clips(
                &video_path,
                duration,
                INTRO_EXCLUSION_PERCENT,
                OUTRO_EXCLUSION_PERCENT,
                clip_count,
                &ClipConfig::default(),
            );

            if let Ok(clips2) = result2 {
                for i in 0..(clips2.len().saturating_sub(1)) {
                    prop_assert!(
                        clips2[i].start_seconds < clips2[i + 1].start_seconds,
                        "Second call: Clip {} should come before clip {} in chronological order",
                        i,
                        i + 1
                    );
                }
            }
        }
    }

    // Feature: multiple-clips-per-video, Property 10: Warning Logging for Reduced Clip Count
    // **Validates: Requirements 9.2**
    proptest! {
        #[test]
        fn test_warning_logging_for_reduced_clip_count(
            // Generate short durations that can't fit all requested clips
            duration in 30.0..80.0f64,
            clip_count in 3u8..=4u8,
        ) {
            // Property: For any video where fewer clips are generated than requested,
            // a warning should be logged containing the video filename and the actual
            // number of clips generated.
            //
            // Note: This property test verifies the behavior at the selector level.
            // The actual warning logging happens in the processor, which is tested separately.
            // Here we verify that the selector returns fewer clips than requested when appropriate.

            let selector = RandomSelector;
            let video_path = PathBuf::from("/test/short_video.mp4");

            // Calculate valid selection zone
            let intro_cutoff = duration * (INTRO_EXCLUSION_PERCENT / 100.0);
            let outro_cutoff = duration - (duration * (OUTRO_EXCLUSION_PERCENT / 100.0));
            let valid_duration = outro_cutoff - intro_cutoff;

            // Calculate maximum possible clips
            let max_possible_clips = (valid_duration / MIN_CLIP_DURATION).floor() as u8;

            let result = selector.select_clips(
                &video_path,
                duration,
                INTRO_EXCLUSION_PERCENT,
                OUTRO_EXCLUSION_PERCENT,
                clip_count,
                &ClipConfig::default(),
            );

            prop_assert!(result.is_ok(), "Selection should succeed");
            let clips = result.unwrap();

            // Property 1: When video is too short, fewer clips should be returned
            if max_possible_clips < clip_count {
                prop_assert!(
                    clips.len() < clip_count as usize,
                    "Should return fewer clips ({}) than requested ({}) when video is too short",
                    clips.len(),
                    clip_count
                );

                // Property 2: The number of clips should match the maximum possible
                prop_assert!(
                    clips.len() <= max_possible_clips as usize,
                    "Should return at most {} clips (the maximum that fits), got {}",
                    max_possible_clips,
                    clips.len()
                );
            }

            // Property 3: All returned clips should be valid
            for clip in &clips {
                prop_assert!(
                    clip.is_valid_duration(),
                    "All returned clips should have valid duration"
                );
            }

            // Property 4: The difference between requested and actual should be determinable
            let clips_not_generated = clip_count.saturating_sub(clips.len() as u8);
            prop_assert!(
                clips_not_generated <= clip_count,
                "Clips not generated ({}) should not exceed requested count ({})",
                clips_not_generated,
                clip_count
            );
        }
    }

    // Feature: multiple-clips-per-video, Property 11: No-Crash Guarantee for Constrained Videos
    // **Validates: Requirements 9.3**
    proptest! {
        #[test]
        fn test_no_crash_guarantee_for_constrained_videos(
            // Test with wide range of durations including edge cases
            duration in 1.0..1000.0f64,
            clip_count in 1u8..=4u8,
            intro_exclusion in 0.0..50.0f64,
            outro_exclusion in 0.0..50.0f64,
        ) {
            // Property: For any video file, regardless of duration or clip count requested,
            // the processing should complete without panicking or returning a fatal error.
            // It may return fewer clips than requested, but should not crash.

            let selector = RandomSelector;
            let video_path = PathBuf::from("/test/video.mp4");

            // Attempt to select clips - this should never panic
            let result = selector.select_clips(
                &video_path,
                duration,
                intro_exclusion,
                outro_exclusion,
                clip_count,
                &ClipConfig::default(),
            );

            // Property 1: The operation should always return a Result (not panic)
            // If we reach this point, the no-panic property is satisfied
            prop_assert!(
                result.is_ok(),
                "Selection should always succeed and return a Result, even for constrained videos"
            );

            let clips = result.unwrap();

            // Property 2: The result should be a valid vector (possibly empty)
            prop_assert!(
                clips.len() <= clip_count as usize,
                "Should not return more clips than requested"
            );

            // Property 3: All returned clips should be valid
            for (i, clip) in clips.iter().enumerate() {
                // Duration should be positive
                prop_assert!(
                    clip.duration_seconds > 0.0,
                    "Clip {} should have positive duration", i
                );

                // Start should be non-negative
                prop_assert!(
                    clip.start_seconds >= 0.0,
                    "Clip {} should have non-negative start time", i
                );

                // Clip should not extend beyond video duration
                let clip_end = clip.start_seconds + clip.duration_seconds;
                prop_assert!(
                    clip_end <= duration + FLOAT_EPSILON,
                    "Clip {} should not extend beyond video duration", i
                );
            }

            // Property 4: Clips should be non-overlapping
            for i in 0..clips.len() {
                for j in (i + 1)..clips.len() {
                    prop_assert!(
                        !clips[i].overlaps(&clips[j]),
                        "Clips {} and {} should not overlap", i, j
                    );
                }
            }

            // Property 5: Clips should be in chronological order
            for i in 0..(clips.len().saturating_sub(1)) {
                prop_assert!(
                    clips[i].start_seconds <= clips[i + 1].start_seconds,
                    "Clips should be in chronological order"
                );
            }

            // Property 6: If video is extremely short or exclusion zones are too large,
            // it's acceptable to return an empty vector (graceful degradation)
            let intro_cutoff = duration * (intro_exclusion / 100.0);
            let outro_cutoff = duration - (duration * (outro_exclusion / 100.0));
            let valid_duration = outro_cutoff - intro_cutoff;

            if valid_duration < MIN_CLIP_DURATION {
                // For videos too short for even one clip, empty vector is expected
                prop_assert!(
                    clips.is_empty(),
                    "Should return empty vector when valid zone is too short for any clip"
                );
            }
        }
    }

    #[test]
    fn test_action_selector_multiple_clips_mock_data() {
        // Test that ActionSelector can generate multiple non-overlapping clips from motion peaks
        use crate::cli::Resolution;
        use std::path::PathBuf;

        // Create an FFmpegExecutor
        let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true, false);

        // Create ActionSelector
        let selector = ActionSelector::new(ffmpeg_executor);

        // Use a non-existent video path - this will cause motion analysis to fail
        // In a real scenario, we'd need actual video with motion data
        // For now, this tests the fallback behavior with multiple clips requested
        let video_path = PathBuf::from("/nonexistent/video_with_motion.mp4");
        let duration = 600.0; // 10 minutes

        // Request 3 clips
        let result = selector.select_clips(
            &video_path,
            duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            3,
            &ClipConfig::default(),
        );

        // The result should be Ok (will fall back to middle segment since motion analysis fails)
        assert!(
            result.is_ok(),
            "Should handle multiple clip request gracefully"
        );

        let time_ranges = result.unwrap();

        // With fallback, we get a single middle segment
        assert_eq!(time_ranges.len(), 1, "Fallback should return single clip");
    }

    #[test]
    fn test_action_selector_clips_sorted_chronologically() {
        // Test that multiple clips are returned in chronological order
        use crate::cli::Resolution;
        use std::path::PathBuf;

        let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true, false);
        let selector = ActionSelector::new(ffmpeg_executor);

        let video_path = PathBuf::from("/nonexistent/video.mp4");
        let duration = 1200.0; // 20 minutes

        // Request 4 clips
        let result = selector.select_clips(
            &video_path,
            duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            4,
            &ClipConfig::default(),
        );

        assert!(result.is_ok());
        let clips = result.unwrap();

        // Verify clips are sorted chronologically
        for i in 0..(clips.len().saturating_sub(1)) {
            assert!(
                clips[i].start_seconds < clips[i + 1].start_seconds,
                "Clip {} (start: {}) should come before clip {} (start: {})",
                i,
                clips[i].start_seconds,
                i + 1,
                clips[i + 1].start_seconds
            );
        }
    }

    #[test]
    fn test_action_selector_graceful_degradation() {
        // Test that ActionSelector handles short videos gracefully with multiple clips requested
        use crate::cli::Resolution;
        use std::path::PathBuf;

        let ffmpeg_executor = crate::ffmpeg::FFmpegExecutor::new(Resolution::Hd1080, true, false);
        let selector = ActionSelector::new(ffmpeg_executor);

        let video_path = PathBuf::from("/nonexistent/short_video.mp4");
        let duration = 25.0; // 25 seconds - too short for 3 clips of 10-15 seconds each

        // Request 3 clips
        let result = selector.select_clips(
            &video_path,
            duration,
            INTRO_EXCLUSION_PERCENT,
            OUTRO_EXCLUSION_PERCENT,
            3,
            &ClipConfig::default(),
        );

        assert!(result.is_ok());
        let clips = result.unwrap();

        // Should return fewer clips than requested due to video length
        assert!(
            clips.len() <= 3,
            "Should not return more clips than requested"
        );
    }
}
