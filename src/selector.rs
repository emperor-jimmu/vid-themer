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
    #[allow(dead_code)]
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
        clips.sort_by(|a, b| a.start_seconds.total_cmp(&b.start_seconds));

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

        if existing_clips.is_empty() {
            if outro_cutoff - intro_cutoff >= clip_duration {
                gaps.push((intro_cutoff, outro_cutoff));
            }
            return gaps;
        }

        let first_clip_start = existing_clips[0].start_seconds;
        if first_clip_start - intro_cutoff >= clip_duration {
            gaps.push((intro_cutoff, first_clip_start));
        }

        for i in 0..existing_clips.len() - 1 {
            let current_end = existing_clips[i].start_seconds + existing_clips[i].duration_seconds;
            let next_start = existing_clips[i + 1].start_seconds;
            let gap_size = next_start - current_end;

            if gap_size >= clip_duration {
                gaps.push((current_end, next_start));
            }
        }

        let last_clip = existing_clips.last().unwrap();
        let last_clip_end = last_clip.start_seconds + last_clip.duration_seconds;
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
    selected_clips.sort_by(|a, b| a.start_seconds.total_cmp(&b.start_seconds));

    Ok(selected_clips)
}

pub struct IntenseAudioSelector;

impl IntenseAudioSelector {
    pub fn new() -> Self {
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
            Err(_e) => {
                // Audio analysis failed, fall back to middle segment
                Ok(vec![config.middle_segment(duration)?])
            }
        }
    }
}

pub struct ActionSelector;

impl ActionSelector {
    pub fn new() -> Self {
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
pub enum SelectionError {}

#[cfg(test)]
#[path = "selector_tests.rs"]
mod tests;
