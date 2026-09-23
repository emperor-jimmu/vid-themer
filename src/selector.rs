// Clip selection strategies (trait + implementations)

use rand::RngExt;
use std::path::Path;

/// Configuration for clip duration constraints.
#[derive(Clone)]
pub struct ClipConfig {
    pub min_duration: f64,
    pub max_duration: f64,
}

impl Default for ClipConfig {
    fn default() -> Self {
        Self {
            min_duration: 20.0,
            max_duration: 30.0,
        }
    }
}

/// A time segment within a video.
#[derive(Clone, Debug)]
pub struct TimeRange {
    pub start_seconds: f64,
    pub duration_seconds: f64,
}

impl TimeRange {
    pub fn overlaps(&self, other: &TimeRange) -> bool {
        let self_end = self.start_seconds + self.duration_seconds;
        let other_end = other.start_seconds + other.duration_seconds;
        self.start_seconds < other_end && other.start_seconds < self_end
    }

    fn end(&self) -> f64 {
        self.start_seconds + self.duration_seconds
    }
}

#[derive(Clone)]
struct Policy {
    intro_exclusion_percent: f64,
    outro_exclusion_percent: f64,
    clip_count: u8,
    config: ClipConfig,
}

impl Policy {
    fn new(intro: f64, outro: f64, clip_count: u8, config: ClipConfig) -> Self {
        Self {
            intro_exclusion_percent: intro,
            outro_exclusion_percent: outro,
            clip_count,
            config,
        }
    }
}

fn zone(duration: f64, policy: &Policy) -> (f64, f64) {
    let intro = duration * (policy.intro_exclusion_percent / 100.0);
    let outro = duration - (duration * (policy.outro_exclusion_percent / 100.0));
    (intro, outro.max(intro))
}

/// One clip when selection finds nothing. Zone-middle if the zone fits, file-middle if only the file fits, otherwise nothing.
fn fallback(duration: f64, policy: &Policy) -> Vec<TimeRange> {
    let (intro, outro) = zone(duration, policy);
    let zone_len = outro - intro;
    let cfg = &policy.config;
    if zone_len >= cfg.min_duration {
        let len = cfg.max_duration.min(zone_len);
        return vec![TimeRange {
            start_seconds: intro + (zone_len - len) / 2.0,
            duration_seconds: len,
        }];
    }
    if duration >= cfg.min_duration {
        let len = cfg.max_duration.min(duration);
        return vec![TimeRange {
            start_seconds: ((duration - len) / 2.0).max(0.0),
            duration_seconds: len,
        }];
    }
    vec![]
}

fn gaps_for(
    intro: f64,
    outro: f64,
    clips: &[TimeRange],
    clip_duration: f64,
) -> Vec<(f64, f64)> {
    let mut sorted = clips.to_vec();
    sorted.sort_by(|a, b| a.start_seconds.total_cmp(&b.start_seconds));

    let mut gaps = Vec::new();
    if sorted.is_empty() {
        if outro - intro >= clip_duration {
            gaps.push((intro, outro));
        }
        return gaps;
    }

    if sorted[0].start_seconds - intro >= clip_duration {
        gaps.push((intro, sorted[0].start_seconds));
    }
    for pair in sorted.windows(2) {
        let gap = pair[1].start_seconds - pair[0].end();
        if gap >= clip_duration {
            gaps.push((pair[0].end(), pair[1].start_seconds));
        }
    }
    let last_end = sorted.last().unwrap().end();
    if outro - last_end >= clip_duration {
        gaps.push((last_end, outro));
    }
    gaps
}

fn select_random(duration: f64, policy: &Policy) -> Vec<TimeRange> {
    let (intro, outro) = zone(duration, policy);
    if outro - intro < policy.config.min_duration {
        return fallback(duration, policy);
    }

    let mut clips = Vec::new();
    let mut rng = rand::rng();
    let mut attempts = 0u32;
    let mut empty_gaps = 0u32;
    while clips.len() < policy.clip_count as usize && attempts < 1000 {
        attempts += 1;
        let clip_duration = rng.random_range(policy.config.min_duration..=policy.config.max_duration);
        let gaps = gaps_for(intro, outro, &clips, clip_duration);
        if gaps.is_empty() {
            empty_gaps += 1;
            if empty_gaps > 50 {
                break;
            }
            continue;
        }
        empty_gaps = 0;
        let (gap_start, gap_end) = gaps[rng.random_range(0..gaps.len())];
        let start = rng.random_range(gap_start..=(gap_end - clip_duration));
        let candidate = TimeRange {
            start_seconds: start,
            duration_seconds: clip_duration,
        };
        if !clips.iter().any(|existing| candidate.overlaps(existing)) {
            clips.push(candidate);
        }
    }
    if clips.is_empty() {
        fallback(duration, policy)
    } else {
        clips.sort_by(|a, b| a.start_seconds.total_cmp(&b.start_seconds));
        clips
    }
}

struct Peak {
    start: f64,
    duration: f64,
}

fn select_from_peaks(peaks: &[Peak], duration: f64, policy: &Policy) -> Vec<TimeRange> {
    let (intro, outro) = zone(duration, policy);
    if outro - intro < policy.config.min_duration {
        return fallback(duration, policy);
    }

    let clip_duration =
        policy.config.min_duration + (policy.config.max_duration - policy.config.min_duration) * 0.5;
    let mut selected = Vec::new();
    for peak in peaks {
        if selected.len() >= policy.clip_count as usize {
            break;
        }
        let peak_end = peak.start + peak.duration;
        if peak.start < intro || peak_end > outro {
            continue;
        }
        let mut start = (peak.start - clip_duration / 2.0).max(intro);
        let mut end = start + clip_duration;
        if end > outro {
            end = outro;
            start = (end - clip_duration).max(intro);
        }
        let actual = end - start;
        if !(policy.config.min_duration..=policy.config.max_duration).contains(&actual) {
            continue;
        }
        let candidate = TimeRange {
            start_seconds: start,
            duration_seconds: actual,
        };
        if selected.iter().any(|existing| candidate.overlaps(existing)) {
            continue;
        }
        selected.push(candidate);
    }
    if selected.is_empty() {
        fallback(duration, policy)
    } else {
        selected.sort_by(|a, b| a.start_seconds.total_cmp(&b.start_seconds));
        selected
    }
}

pub trait ClipSelector: Send + Sync {
    fn select(&self, video_path: &Path, duration: f64) -> Vec<TimeRange>;
}

pub struct RandomSelector {
    policy: Policy,
}

impl RandomSelector {
    pub fn new(intro: f64, outro: f64, clip_count: u8, config: ClipConfig) -> Self {
        Self {
            policy: Policy::new(intro, outro, clip_count, config),
        }
    }
}

impl ClipSelector for RandomSelector {
    fn select(&self, _video_path: &Path, duration: f64) -> Vec<TimeRange> {
        select_random(duration, &self.policy)
    }
}

pub struct IntenseAudioSelector {
    policy: Policy,
}

impl IntenseAudioSelector {
    pub fn new(intro: f64, outro: f64, clip_count: u8, config: ClipConfig) -> Self {
        Self {
            policy: Policy::new(intro, outro, clip_count, config),
        }
    }
}

impl ClipSelector for IntenseAudioSelector {
    fn select(&self, video_path: &Path, duration: f64) -> Vec<TimeRange> {
        match crate::ffmpeg::analyze_audio_intensity(video_path, duration) {
            Ok(segments) if !segments.is_empty() => {
                let peaks = segments
                    .iter()
                    .map(|s| Peak {
                        start: s.start_time,
                        duration: s.duration,
                    })
                    .collect::<Vec<_>>();
                select_from_peaks(&peaks, duration, &self.policy)
            }
            _ => fallback(duration, &self.policy),
        }
    }
}

pub struct ActionSelector {
    policy: Policy,
}

impl ActionSelector {
    pub fn new(intro: f64, outro: f64, clip_count: u8, config: ClipConfig) -> Self {
        Self {
            policy: Policy::new(intro, outro, clip_count, config),
        }
    }
}

impl ClipSelector for ActionSelector {
    fn select(&self, video_path: &Path, duration: f64) -> Vec<TimeRange> {
        match crate::ffmpeg::analyze_motion_intensity(video_path, duration) {
            Ok(segments) if !segments.is_empty() => {
                let peaks = segments
                    .iter()
                    .map(|s| Peak {
                        start: s.start_time,
                        duration: s.duration,
                    })
                    .collect::<Vec<_>>();
                select_from_peaks(&peaks, duration, &self.policy)
            }
            _ => fallback(duration, &self.policy),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(intro: f64, outro: f64, count: u8, min: f64, max: f64) -> Policy {
        Policy::new(
            intro,
            outro,
            count,
            ClipConfig {
                min_duration: min,
                max_duration: max,
            },
        )
    }

    fn in_zone(range: &TimeRange, duration: f64, policy: &Policy) -> bool {
        let (intro, outro) = zone(duration, policy);
        range.start_seconds >= intro - 1e-9 && range.end() <= outro + 1e-9
    }

    #[test]
    fn fallback_uses_zone_middle_when_zone_fits() {
        let policy = policy(10.0, 10.0, 1, 20.0, 30.0);
        let clips = fallback(100.0, &policy);
        assert_eq!(clips.len(), 1);
        assert!((clips[0].duration_seconds - 30.0).abs() < 1e-9);
        assert!(in_zone(&clips[0], 100.0, &policy));
        assert!((clips[0].start_seconds - 35.0).abs() < 1e-9);
    }

    #[test]
    fn fallback_uses_file_middle_when_only_the_file_fits() {
        let policy = policy(45.0, 45.0, 1, 20.0, 30.0);
        let clips = fallback(100.0, &policy);
        assert_eq!(clips.len(), 1);
        assert!((clips[0].duration_seconds - 30.0).abs() < 1e-9);
        assert!((clips[0].start_seconds - 35.0).abs() < 1e-9);
        assert!(!in_zone(&clips[0], 100.0, &policy));
    }

    #[test]
    fn fallback_is_empty_when_the_file_is_shorter_than_min() {
        let policy = policy(0.0, 0.0, 1, 20.0, 30.0);
        assert!(fallback(10.0, &policy).is_empty());
    }

    #[test]
    fn short_list_is_not_padded() {
        let policy = policy(0.0, 0.0, 3, 20.0, 30.0);
        let peaks = [Peak {
            start: 40.0,
            duration: 10.0,
        }];
        let clips = select_from_peaks(&peaks, 200.0, &policy);
        assert_eq!(clips.len(), 1);
    }

    #[test]
    fn overlapping_peaks_yield_one_clip() {
        let policy = policy(0.0, 0.0, 2, 20.0, 30.0);
        let peaks = [
            Peak {
                start: 50.0,
                duration: 10.0,
            },
            Peak {
                start: 52.0,
                duration: 10.0,
            },
        ];
        let clips = select_from_peaks(&peaks, 200.0, &policy);
        assert_eq!(clips.len(), 1);
        assert!(!clips[0].overlaps(&clips[0]) || clips.len() == 1);
        for pair in clips.windows(2) {
            assert!(!pair[0].overlaps(&pair[1]));
        }
    }

    #[test]
    fn peak_outside_the_zone_is_not_used() {
        let policy = policy(20.0, 20.0, 1, 20.0, 30.0);
        let peaks = [Peak {
            start: 1.0,
            duration: 5.0,
        }];
        let clips = select_from_peaks(&peaks, 100.0, &policy);
        assert_eq!(clips.len(), 1);
        assert!(in_zone(&clips[0], 100.0, &policy));
        assert!(clips[0].start_seconds > 10.0);
    }

    #[test]
    fn gaps_see_clips_that_were_picked_out_of_order() {
        let clips = [
            TimeRange {
                start_seconds: 40.0,
                duration_seconds: 10.0,
            },
            TimeRange {
                start_seconds: 10.0,
                duration_seconds: 10.0,
            },
        ];
        let gaps = gaps_for(0.0, 100.0, &clips, 10.0);
        assert!(gaps.iter().any(|(start, end)| *start == 20.0 && *end == 40.0));
    }
}
