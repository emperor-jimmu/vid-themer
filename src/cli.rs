// CLI argument parsing using clap derive macros

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "video-clip-extractor")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Extract thematic clips from video files", long_about = None)]
pub struct CliArgs {
    /// Directory path to scan for videos
    #[arg(value_name = "PATH")]
    pub directory: PathBuf,

    /// Clip selection strategy (random, intense-audio, action/intense-action)
    #[arg(short = 's', long = "strategy", value_enum, default_value = "random")]
    pub strategy: SelectionStrategy,

    /// Output resolution
    #[arg(short = 'r', long = "resolution", value_enum, default_value = "1080p")]
    pub resolution: Resolution,

    /// Include audio in output clip
    #[arg(short = 'a', long = "audio", action = clap::ArgAction::Set, default_value_t = true)]
    pub include_audio: bool,

    /// Intro exclusion zone as percentage of video duration (0-100)
    #[arg(long = "intro-exclusion", default_value_t = 2.0, value_parser = validate_percentage)]
    pub intro_exclusion_percent: f64,

    /// Outro exclusion zone as percentage of video duration (0-100)
    #[arg(long = "outro-exclusion", default_value_t = 40.0, value_parser = validate_percentage)]
    pub outro_exclusion_percent: f64,

    /// Number of clips to generate per video (1-4)
    #[arg(short = 'c', long = "clip-count", default_value = "1", value_parser = validate_clip_count)]
    pub clip_count: u8,

    /// Minimum clip duration in seconds
    #[arg(long = "min-duration", default_value_t = 20.0, value_parser = validate_duration)]
    pub min_duration: f64,

    /// Maximum clip duration in seconds
    #[arg(long = "max-duration", default_value_t = 30.0, value_parser = validate_duration)]
    pub max_duration: f64,

    /// Force regeneration of all clips, ignoring existing clips
    #[arg(short = 'f', long = "force", action = clap::ArgAction::SetTrue, default_value_t = false)]
    pub force: bool,

    /// Use hardware acceleration for encoding (h264_videotoolbox on macOS, h264_nvenc elsewhere)
    #[arg(long = "hw-accel", action = clap::ArgAction::SetTrue, default_value_t = false)]
    pub hw_accel: bool,
}

fn validate_duration(s: &str) -> Result<f64, String> {
    let value: f64 = s
        .parse()
        .map_err(|_| format!("'{}' is not a valid number", s))?;
    if value <= 0.0 {
        return Err(format!("duration must be greater than 0, got {}", value));
    }
    if value > 300.0 {
        return Err(format!(
            "duration must be 300 seconds or less, got {}",
            value
        ));
    }
    Ok(value)
}

fn validate_percentage(s: &str) -> Result<f64, String> {
    let value: f64 = s
        .parse()
        .map_err(|_| format!("'{}' is not a valid number", s))?;
    if !(0.0..=100.0).contains(&value) {
        return Err(format!(
            "percentage must be between 0 and 100, got {}",
            value
        ));
    }
    Ok(value)
}

fn validate_clip_count(s: &str) -> Result<u8, String> {
    let count = s
        .parse::<u8>()
        .map_err(|_| "Clip count must be a number".to_string())?;

    if !(1..=4).contains(&count) {
        return Err("Clip count must be between 1 and 4".to_string());
    }

    Ok(count)
}

impl CliArgs {
    /// Validate that min_duration <= max_duration
    pub fn validate_duration_range(&self) -> Result<(), String> {
        if self.min_duration > self.max_duration {
            return Err(format!(
                "min-duration ({}) cannot be greater than max-duration ({})",
                self.min_duration, self.max_duration
            ));
        }
        Ok(())
    }

    /// Validate that intro + outro exclusion zones don't exceed 100%
    pub fn validate_exclusion_zones(&self) -> Result<(), String> {
        let total_exclusion = self.intro_exclusion_percent + self.outro_exclusion_percent;
        if total_exclusion >= 100.0 {
            return Err(format!(
                "intro-exclusion ({}) + outro-exclusion ({}) must be less than 100%, got {}%",
                self.intro_exclusion_percent, self.outro_exclusion_percent, total_exclusion
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum SelectionStrategy {
    Random,
    IntenseAudio,
    #[value(name = "action", alias = "intense-action")]
    Action,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum Resolution {
    #[value(name = "720p")]
    Hd720,
    #[value(name = "1080p")]
    Hd1080,
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
