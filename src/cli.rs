// CLI argument parsing using clap derive macros

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "video-clip-extractor")]
#[command(about = "Extract thematic clips from video files", long_about = None)]
pub struct CliArgs {
    /// Directory path to scan for videos
    #[arg(value_name = "PATH")]
    pub directory: PathBuf,

    /// Clip selection strategy
    #[arg(short = 's', long = "strategy", value_enum, default_value = "random")]
    pub strategy: SelectionStrategy,

    /// Output resolution
    #[arg(short = 'r', long = "resolution", value_enum, default_value = "1080p")]
    pub resolution: Resolution,

    /// Include audio in output clip
    #[arg(short = 'a', long = "audio", action = clap::ArgAction::Set, default_value_t = true)]
    pub include_audio: bool,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum SelectionStrategy {
    Random,
    IntenseAudio,
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
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--strategy",
            "random"
        ]);
        
        assert!(matches!(args.strategy, SelectionStrategy::Random));
    }

    #[test]
    fn test_strategy_intense_audio() {
        // Test intense-audio strategy flag
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--strategy",
            "intense-audio"
        ]);
        
        assert!(matches!(args.strategy, SelectionStrategy::IntenseAudio));
    }

    #[test]
    fn test_strategy_short_flag() {
        // Test short flag for strategy
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "-s",
            "intense-audio"
        ]);
        
        assert!(matches!(args.strategy, SelectionStrategy::IntenseAudio));
    }

    #[test]
    fn test_resolution_720p() {
        // Test 720p resolution flag
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--resolution",
            "720p"
        ]);
        
        assert!(matches!(args.resolution, Resolution::Hd720));
    }

    #[test]
    fn test_resolution_1080p() {
        // Test 1080p resolution flag
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--resolution",
            "1080p"
        ]);
        
        assert!(matches!(args.resolution, Resolution::Hd1080));
    }

    #[test]
    fn test_resolution_short_flag() {
        // Test short flag for resolution
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "-r",
            "720p"
        ]);
        
        assert!(matches!(args.resolution, Resolution::Hd720));
    }

    #[test]
    fn test_audio_true() {
        // Test explicit audio inclusion
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--audio",
            "true"
        ]);
        
        assert_eq!(args.include_audio, true);
    }

    #[test]
    fn test_audio_false() {
        // Test audio exclusion
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--audio",
            "false"
        ]);
        
        assert_eq!(args.include_audio, false);
    }

    #[test]
    fn test_audio_short_flag() {
        // Test short flag for audio
        let args = CliArgs::parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "-a",
            "false"
        ]);
        
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
            "false"
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
            "true"
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
            "false"
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
            "invalid-strategy"
        ]);
        
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
    }

    #[test]
    fn test_invalid_resolution() {
        // Test that invalid resolution value produces an error
        let result = CliArgs::try_parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--resolution",
            "4k"
        ]);
        
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
    }

    #[test]
    fn test_invalid_audio_value() {
        // Test that invalid audio value produces an error
        let result = CliArgs::try_parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--audio",
            "maybe"
        ]);
        
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::InvalidValue);
    }

    #[test]
    fn test_unknown_flag() {
        // Test that unknown flags produce an error
        let result = CliArgs::try_parse_from(&[
            "video-clip-extractor",
            "/test/path",
            "--unknown-flag"
        ]);
        
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
            err.kind() == clap::error::ErrorKind::DisplayVersion ||
            err.kind() == clap::error::ErrorKind::UnknownArgument
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
    }
}
