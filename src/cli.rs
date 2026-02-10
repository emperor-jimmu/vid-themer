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
mod tests {
    use super::*;
    use clap::CommandFactory;
    use proptest::prelude::*;

    #[test]
    fn test_default_values() {
        // Test that default values are applied when optional arguments are omitted
        let args = CliArgs::parse_from(&["video-clip-extractor", "/test/path"]);

        assert_eq!(args.directory, PathBuf::from("/test/path"));
        assert!(matches!(args.strategy, SelectionStrategy::Random));
        assert!(matches!(args.resolution, Resolution::Hd1080));
        assert_eq!(args.include_audio, true);
    }

    #[test]
    fn test_strategy_random() {
        // Test explicit random strategy flag
        let args =
            CliArgs::parse_from(&["video-clip-extractor", "/test/path", "--strategy", "random"]);

        assert!(matches!(args.strategy, SelectionStrategy::Random));
    }

    #[test]
    fn test_strategy_intense_audio() {
        // Test intense-audio strategy flag
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--strategy",
            "intense-audio",
        ]);

        assert!(matches!(args.strategy, SelectionStrategy::IntenseAudio));
    }

    #[test]
    fn test_strategy_short_flag() {
        // Test short flag for strategy
        let args =
            CliArgs::parse_from(&["video-clip-extractor", "/test/path", "-s", "intense-audio"]);

        assert!(matches!(args.strategy, SelectionStrategy::IntenseAudio));
    }

    #[test]
    fn test_strategy_action() {
        // Test action strategy flag
        let args =
            CliArgs::parse_from(&["video-clip-extractor", "/test/path", "--strategy", "action"]);

        assert!(matches!(args.strategy, SelectionStrategy::Action));
    }

    #[test]
    fn test_strategy_action_short_flag() {
        // Test short flag for action strategy
        let args = CliArgs::parse_from(&["video-clip-extractor", "/test/path", "-s", "action"]);

        assert!(matches!(args.strategy, SelectionStrategy::Action));
    }

    #[test]
    fn test_strategy_intense_action_alias() {
        // Test intense-action alias for action strategy
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--strategy",
            "intense-action",
        ]);

        assert!(matches!(args.strategy, SelectionStrategy::Action));
    }

    #[test]
    fn test_strategy_intense_action_alias_short_flag() {
        // Test short flag with intense-action alias
        let args =
            CliArgs::parse_from(&["video-clip-extractor", "/test/path", "-s", "intense-action"]);

        assert!(matches!(args.strategy, SelectionStrategy::Action));
    }

    #[test]
    fn test_help_contains_action_strategy() {
        // Test that help output includes action strategy documentation
        let mut cmd = CliArgs::command();
        let help = cmd.render_help().to_string();

        assert!(help.contains("action"));
    }

    #[test]
    fn test_resolution_720p() {
        // Test 720p resolution flag
        let args =
            CliArgs::parse_from(&["video-clip-extractor", "/test/path", "--resolution", "720p"]);

        assert!(matches!(args.resolution, Resolution::Hd720));
    }

    #[test]
    fn test_resolution_1080p() {
        // Test 1080p resolution flag
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--resolution",
            "1080p",
        ]);

        assert!(matches!(args.resolution, Resolution::Hd1080));
    }

    #[test]
    fn test_resolution_short_flag() {
        // Test short flag for resolution
        let args = CliArgs::parse_from(&["video-clip-extractor", "/test/path", "-r", "720p"]);

        assert!(matches!(args.resolution, Resolution::Hd720));
    }

    #[test]
    fn test_audio_true() {
        // Test explicit audio inclusion
        let args = CliArgs::parse_from(&["video-clip-extractor", "/test/path", "--audio", "true"]);

        assert_eq!(args.include_audio, true);
    }

    #[test]
    fn test_audio_false() {
        // Test audio exclusion
        let args = CliArgs::parse_from(&["video-clip-extractor", "/test/path", "--audio", "false"]);

        assert_eq!(args.include_audio, false);
    }

    #[test]
    fn test_audio_short_flag() {
        // Test short flag for audio
        let args = CliArgs::parse_from(&["video-clip-extractor", "/test/path", "-a", "false"]);

        assert_eq!(args.include_audio, false);
    }

    #[test]
    fn test_all_flags_combined() {
        // Test all flags together
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/my/videos",
            "--strategy",
            "intense-audio",
            "--resolution",
            "720p",
            "--audio",
            "false",
        ]);

        assert_eq!(args.directory, PathBuf::from("/my/videos"));
        assert!(matches!(args.strategy, SelectionStrategy::IntenseAudio));
        assert!(matches!(args.resolution, Resolution::Hd720));
        assert_eq!(args.include_audio, false);
    }

    #[test]
    fn test_all_short_flags_combined() {
        // Test all short flags together
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/my/videos",
            "-s",
            "random",
            "-r",
            "1080p",
            "-a",
            "true",
        ]);

        assert_eq!(args.directory, PathBuf::from("/my/videos"));
        assert!(matches!(args.strategy, SelectionStrategy::Random));
        assert!(matches!(args.resolution, Resolution::Hd1080));
        assert_eq!(args.include_audio, true);
    }

    #[test]
    fn test_mixed_long_and_short_flags() {
        // Test mixing long and short flags
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "-s",
            "intense-audio",
            "--resolution",
            "720p",
            "-a",
            "false",
        ]);

        assert!(matches!(args.strategy, SelectionStrategy::IntenseAudio));
        assert!(matches!(args.resolution, Resolution::Hd720));
        assert_eq!(args.include_audio, false);
    }

    #[test]
    fn test_missing_required_directory() {
        // Test that missing directory argument produces an error
        let result = CliArgs::try_parse_from(&["video-clip-extractor"]);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn test_invalid_strategy() {
        // Test that invalid strategy value produces an error
        let result = CliArgs::try_parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--strategy",
            "invalid-strategy",
        ]);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
    }

    #[test]
    fn test_invalid_resolution() {
        // Test that invalid resolution value produces an error
        let result =
            CliArgs::try_parse_from(&["video-clip-extractor", "/test/path", "--resolution", "4k"]);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
    }

    #[test]
    fn test_invalid_audio_value() {
        // Test that invalid audio value produces an error
        let result =
            CliArgs::try_parse_from(&["video-clip-extractor", "/test/path", "--audio", "maybe"]);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
    }

    #[test]
    fn test_unknown_flag() {
        // Test that unknown flags produce an error
        let result =
            CliArgs::try_parse_from(&["video-clip-extractor", "/test/path", "--unknown-flag"]);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
    }

    #[test]
    fn test_help_flag() {
        // Test that --help flag produces help output
        let result = CliArgs::try_parse_from(&["video-clip-extractor", "--help"]);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn test_help_flag_short() {
        // Test that -h flag produces help output
        let result = CliArgs::try_parse_from(&["video-clip-extractor", "-h"]);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
    }

    #[test]
    fn test_version_flag() {
        // Test that --version flag produces version or unknown argument error
        // (version only works if configured in Cargo.toml)
        let result = CliArgs::try_parse_from(&["video-clip-extractor", "--version"]);

        assert!(result.is_err());
        let err = result.unwrap_err();
        // Version flag may not be available if not configured in Cargo.toml
        assert!(
            err.kind() == clap::error::ErrorKind::DisplayVersion
                || err.kind() == clap::error::ErrorKind::UnknownArgument
        );
    }

    #[test]
    fn test_help_contains_usage_info() {
        // Test that help output contains expected information
        let mut cmd = CliArgs::command();
        let help = cmd.render_help().to_string();

        assert!(help.contains("Extract thematic clips from video files"));
        assert!(help.contains("PATH"));
        assert!(help.contains("--strategy"));
        assert!(help.contains("--resolution"));
        assert!(help.contains("--audio"));
        assert!(help.contains("--intro-exclusion"));
        assert!(help.contains("--outro-exclusion"));
        assert!(help.contains("--min-duration"));
        assert!(help.contains("--max-duration"));
    }

    // Tests for min-duration and max-duration parameters

    #[test]
    fn test_min_duration_default() {
        // Test that min-duration defaults to 20.0
        let args = CliArgs::parse_from(&["video-clip-extractor", "/test/path"]);
        assert_eq!(args.min_duration, 20.0);
    }

    #[test]
    fn test_max_duration_default() {
        // Test that max-duration defaults to 30.0
        let args = CliArgs::parse_from(&["video-clip-extractor", "/test/path"]);
        assert_eq!(args.max_duration, 30.0);
    }

    #[test]
    fn test_min_duration_custom() {
        // Test custom min-duration value
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--min-duration",
            "5.0",
        ]);
        assert_eq!(args.min_duration, 5.0);
    }

    #[test]
    fn test_max_duration_custom() {
        // Test custom max-duration value
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--max-duration",
            "20.0",
        ]);
        assert_eq!(args.max_duration, 20.0);
    }

    #[test]
    fn test_duration_range_equal() {
        // Test that min and max can be equal (fixed duration)
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--min-duration",
            "10.0",
            "--max-duration",
            "10.0",
        ]);
        assert_eq!(args.min_duration, 10.0);
        assert_eq!(args.max_duration, 10.0);
        assert!(args.validate_duration_range().is_ok());
    }

    #[test]
    fn test_duration_range_valid() {
        // Test that min < max is valid
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--min-duration",
            "8.0",
            "--max-duration",
            "12.0",
        ]);
        assert!(args.validate_duration_range().is_ok());
    }

    #[test]
    fn test_duration_range_invalid() {
        // Test that min > max is invalid
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--min-duration",
            "15.0",
            "--max-duration",
            "10.0",
        ]);
        let result = args.validate_duration_range();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be greater than"));
    }

    #[test]
    fn test_min_duration_invalid_zero() {
        // Test that zero min-duration produces an error
        let result =
            CliArgs::try_parse_from(&["video-clip-extractor", "/test/path", "--min-duration", "0"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_min_duration_invalid_negative() {
        // Test that negative min-duration produces an error
        let result = CliArgs::try_parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--min-duration",
            "-5.0",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn test_max_duration_invalid_over_limit() {
        // Test that max-duration over 300 produces an error
        let result = CliArgs::try_parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--max-duration",
            "301.0",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn test_duration_with_other_flags() {
        // Test duration parameters combined with other flags
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--min-duration",
            "8.0",
            "--max-duration",
            "12.0",
            "-s",
            "intense-audio",
            "-r",
            "720p",
            "-c",
            "2",
        ]);
        assert_eq!(args.min_duration, 8.0);
        assert_eq!(args.max_duration, 12.0);
        assert!(matches!(args.strategy, SelectionStrategy::IntenseAudio));
        assert!(matches!(args.resolution, Resolution::Hd720));
        assert_eq!(args.clip_count, 2);
    }

    #[test]
    fn test_intro_exclusion_default() {
        // Test that intro exclusion defaults to 2%
        let args = CliArgs::parse_from(&["video-clip-extractor", "/test/path"]);
        assert_eq!(args.intro_exclusion_percent, 2.0);
    }

    #[test]
    fn test_outro_exclusion_default() {
        // Test that outro exclusion defaults to 40%
        let args = CliArgs::parse_from(&["video-clip-extractor", "/test/path"]);
        assert_eq!(args.outro_exclusion_percent, 40.0);
    }

    #[test]
    fn test_intro_exclusion_custom() {
        // Test custom intro exclusion value
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--intro-exclusion",
            "5.0",
        ]);
        assert_eq!(args.intro_exclusion_percent, 5.0);
    }

    #[test]
    fn test_outro_exclusion_custom() {
        // Test custom outro exclusion value
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--outro-exclusion",
            "30.0",
        ]);
        assert_eq!(args.outro_exclusion_percent, 30.0);
    }

    #[test]
    fn test_exclusion_zones_combined() {
        // Test both exclusion zones together
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--intro-exclusion",
            "2.5",
            "--outro-exclusion",
            "25.0",
        ]);
        assert_eq!(args.intro_exclusion_percent, 2.5);
        assert_eq!(args.outro_exclusion_percent, 25.0);
    }

    #[test]
    fn test_intro_exclusion_invalid_negative() {
        // Test that negative intro exclusion produces an error
        let result = CliArgs::try_parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--intro-exclusion",
            "-5.0",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn test_intro_exclusion_invalid_over_100() {
        // Test that intro exclusion over 100 produces an error
        let result = CliArgs::try_parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--intro-exclusion",
            "101.0",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn test_outro_exclusion_invalid_negative() {
        // Test that negative outro exclusion produces an error
        let result = CliArgs::try_parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--outro-exclusion",
            "-10.0",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn test_outro_exclusion_invalid_over_100() {
        // Test that outro exclusion over 100 produces an error
        let result = CliArgs::try_parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--outro-exclusion",
            "150.0",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn test_exclusion_zero_values() {
        // Test that zero exclusion values are valid
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--intro-exclusion",
            "0",
            "--outro-exclusion",
            "0",
        ]);
        assert_eq!(args.intro_exclusion_percent, 0.0);
        assert_eq!(args.outro_exclusion_percent, 0.0);
    }

    #[test]
    fn test_exclusion_boundary_values() {
        // Test boundary values (0 and 100)
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--intro-exclusion",
            "100",
            "--outro-exclusion",
            "100",
        ]);
        assert_eq!(args.intro_exclusion_percent, 100.0);
        assert_eq!(args.outro_exclusion_percent, 100.0);
    }

    // Tests for exclusion zone validation (Task 5)

    #[test]
    fn test_exclusion_zones_valid() {
        // Test that valid exclusion zones pass validation
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--intro-exclusion",
            "10",
            "--outro-exclusion",
            "40",
        ]);
        assert!(args.validate_exclusion_zones().is_ok());
    }

    #[test]
    fn test_exclusion_zones_exactly_100() {
        // Test that exactly 100% total exclusion fails
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--intro-exclusion",
            "60",
            "--outro-exclusion",
            "40",
        ]);
        let result = args.validate_exclusion_zones();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be less than 100%"));
    }

    #[test]
    fn test_exclusion_zones_over_100() {
        // Test that over 100% total exclusion fails
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--intro-exclusion",
            "70",
            "--outro-exclusion",
            "50",
        ]);
        let result = args.validate_exclusion_zones();
        assert!(result.is_err());
        let error_msg = result.unwrap_err();
        assert!(error_msg.contains("must be less than 100%"));
        assert!(error_msg.contains("120%"));
    }

    #[test]
    fn test_exclusion_zones_boundary_99() {
        // Test that 99% total exclusion is valid (edge case)
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--intro-exclusion",
            "49.5",
            "--outro-exclusion",
            "49.5",
        ]);
        assert!(args.validate_exclusion_zones().is_ok());
    }

    #[test]
    fn test_exclusion_zones_zero() {
        // Test that zero exclusion zones are valid
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--intro-exclusion",
            "0",
            "--outro-exclusion",
            "0",
        ]);
        assert!(args.validate_exclusion_zones().is_ok());
    }

    // Tests for clip_count parameter (Task 1.1)

    #[test]
    fn test_clip_count_default() {
        // Test that clip count defaults to 1
        let args = CliArgs::parse_from(&["video-clip-extractor", "/test/path"]);
        assert_eq!(args.clip_count, 1);
    }

    #[test]
    fn test_clip_count_valid_1() {
        // Test valid clip count: 1
        let args =
            CliArgs::parse_from(&["video-clip-extractor", "/test/path", "--clip-count", "1"]);
        assert_eq!(args.clip_count, 1);
    }

    #[test]
    fn test_clip_count_valid_2() {
        // Test valid clip count: 2
        let args =
            CliArgs::parse_from(&["video-clip-extractor", "/test/path", "--clip-count", "2"]);
        assert_eq!(args.clip_count, 2);
    }

    #[test]
    fn test_clip_count_valid_3() {
        // Test valid clip count: 3
        let args =
            CliArgs::parse_from(&["video-clip-extractor", "/test/path", "--clip-count", "3"]);
        assert_eq!(args.clip_count, 3);
    }

    #[test]
    fn test_clip_count_valid_4() {
        // Test valid clip count: 4
        let args =
            CliArgs::parse_from(&["video-clip-extractor", "/test/path", "--clip-count", "4"]);
        assert_eq!(args.clip_count, 4);
    }

    #[test]
    fn test_clip_count_short_flag() {
        // Test short flag for clip count
        let args = CliArgs::parse_from(&["video-clip-extractor", "/test/path", "-c", "3"]);
        assert_eq!(args.clip_count, 3);
    }

    #[test]
    fn test_clip_count_invalid_zero() {
        // Test that clip count of 0 produces an error
        let result =
            CliArgs::try_parse_from(&["video-clip-extractor", "/test/path", "--clip-count", "0"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn test_clip_count_invalid_5() {
        // Test that clip count of 5 produces an error
        let result =
            CliArgs::try_parse_from(&["video-clip-extractor", "/test/path", "--clip-count", "5"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn test_clip_count_invalid_negative() {
        // Test that negative clip count produces an error
        let result =
            CliArgs::try_parse_from(&["video-clip-extractor", "/test/path", "--clip-count", "-1"]);
        assert!(result.is_err());
        // Negative values fail at parse stage
    }

    #[test]
    fn test_clip_count_invalid_non_numeric() {
        // Test that non-numeric clip count produces an error
        let result =
            CliArgs::try_parse_from(&["video-clip-extractor", "/test/path", "--clip-count", "abc"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
    }

    #[test]
    fn test_clip_count_with_other_flags() {
        // Test clip count combined with other flags
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "-c",
            "2",
            "-s",
            "intense-audio",
            "-r",
            "720p",
        ]);
        assert_eq!(args.clip_count, 2);
        assert!(matches!(args.strategy, SelectionStrategy::IntenseAudio));
        assert!(matches!(args.resolution, Resolution::Hd720));
    }

    // Feature: multiple-clips-per-video, Property 1: CLI Input Validation
    proptest! {
        #[test]
        fn test_clip_count_validation_property(clip_count in any::<i32>()) {
            // Property: For any input value to the --clip-count parameter,
            // if the value is an integer between 1 and 4 (inclusive), it should be accepted;
            // otherwise, it should be rejected with an error message.
            // Validates: Requirements 1.2, 1.4

            let clip_count_str = clip_count.to_string();
            let result = CliArgs::try_parse_from(&[
                "video-clip-extractor",
                "/test/path",
                "--clip-count",
                &clip_count_str,
            ]);

            if (1..=4).contains(&clip_count) {
                // Valid range: should be accepted
                prop_assert!(result.is_ok(), "Valid clip count {} should be accepted", clip_count);
                if let Ok(args) = result {
                    prop_assert_eq!(args.clip_count, clip_count as u8);
                }
            } else {
                // Invalid range: should be rejected
                prop_assert!(result.is_err(), "Invalid clip count {} should be rejected", clip_count);
                // The error kind may vary (ValueValidation for out-of-range positive values,
                // UnknownArgument for negative values due to parsing issues)
                // The important property is that it's rejected, not the specific error kind
            }
        }
    }
}
