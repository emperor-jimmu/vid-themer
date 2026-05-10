use super::*;
use clap::CommandFactory;
use proptest::prelude::*;

#[test]
fn test_default_values() {
    let args = CliArgs::parse_from(["video-clip-extractor", "/test/path"]);

    assert_eq!(args.directory, PathBuf::from("/test/path"));
    assert!(matches!(args.strategy, SelectionStrategy::Random));
    assert!(matches!(args.resolution, Resolution::Hd1080));
    assert!(args.include_audio);
}

fn parse_ok(args: &[&str]) -> CliArgs {
    CliArgs::parse_from(args)
}

fn parse_err_kind(args: &[&str]) -> clap::error::ErrorKind {
    CliArgs::try_parse_from(args).unwrap_err().kind()
}

#[test]
fn test_strategy_variants() {
    let cases = [
        (
            &["video-clip-extractor", "/test/path", "--strategy", "random"][..],
            "random",
        ),
        (
            &[
                "video-clip-extractor",
                "/test/path",
                "--strategy",
                "intense-audio",
            ][..],
            "intense-audio",
        ),
        (
            &["video-clip-extractor", "/test/path", "-s", "intense-audio"][..],
            "intense-audio",
        ),
        (
            &["video-clip-extractor", "/test/path", "--strategy", "action"][..],
            "action",
        ),
        (
            &["video-clip-extractor", "/test/path", "-s", "action"][..],
            "action",
        ),
        (
            &[
                "video-clip-extractor",
                "/test/path",
                "--strategy",
                "intense-action",
            ][..],
            "intense-action",
        ),
        (
            &["video-clip-extractor", "/test/path", "-s", "intense-action"][..],
            "intense-action",
        ),
    ];

    for (argv, expected) in cases {
        let args = parse_ok(argv);
        match expected {
            "random" => assert!(matches!(args.strategy, SelectionStrategy::Random)),
            "intense-audio" => assert!(matches!(args.strategy, SelectionStrategy::IntenseAudio)),
            "action" | "intense-action" => {
                assert!(matches!(args.strategy, SelectionStrategy::Action))
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn test_resolution_variants() {
    let args = parse_ok(&["video-clip-extractor", "/test/path", "--resolution", "720p"]);
    assert!(matches!(args.resolution, Resolution::Hd720));

    let args = parse_ok(&[
        "video-clip-extractor",
        "/test/path",
        "--resolution",
        "1080p",
    ]);
    assert!(matches!(args.resolution, Resolution::Hd1080));

    let args = parse_ok(&["video-clip-extractor", "/test/path", "-r", "720p"]);
    assert!(matches!(args.resolution, Resolution::Hd720));
}

#[test]
fn test_audio_variants() {
    let args = parse_ok(&["video-clip-extractor", "/test/path", "--audio", "true"]);
    assert!(args.include_audio);

    let args = parse_ok(&["video-clip-extractor", "/test/path", "--audio", "false"]);
    assert!(!args.include_audio);

    let args = parse_ok(&["video-clip-extractor", "/test/path", "-a", "false"]);
    assert!(!args.include_audio);
}

#[test]
fn test_combined_flags() {
    let args = parse_ok(&[
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
    assert!(!args.include_audio);

    let args = parse_ok(&[
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
    assert!(args.include_audio);
}

#[test]
fn test_mixed_long_and_short_flags() {
    let args = parse_ok(&[
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
    assert!(!args.include_audio);
}

#[test]
fn test_parse_errors() {
    let cases = [
        (
            vec!["video-clip-extractor"],
            clap::error::ErrorKind::MissingRequiredArgument,
        ),
        (
            vec![
                "video-clip-extractor",
                "/test/path",
                "--strategy",
                "invalid-strategy",
            ],
            clap::error::ErrorKind::InvalidValue,
        ),
        (
            vec!["video-clip-extractor", "/test/path", "--resolution", "4k"],
            clap::error::ErrorKind::InvalidValue,
        ),
        (
            vec!["video-clip-extractor", "/test/path", "--audio", "maybe"],
            clap::error::ErrorKind::InvalidValue,
        ),
        (
            vec!["video-clip-extractor", "/test/path", "--unknown-flag"],
            clap::error::ErrorKind::UnknownArgument,
        ),
        (
            vec!["video-clip-extractor", "--help"],
            clap::error::ErrorKind::DisplayHelp,
        ),
        (
            vec!["video-clip-extractor", "-h"],
            clap::error::ErrorKind::DisplayHelp,
        ),
    ];

    for (argv, expected_kind) in cases {
        let kind = parse_err_kind(&argv);
        assert_eq!(kind, expected_kind);
    }
}

#[test]
fn test_version_flag() {
    let result = CliArgs::try_parse_from(["video-clip-extractor", "--version"]);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.kind() == clap::error::ErrorKind::DisplayVersion
            || err.kind() == clap::error::ErrorKind::UnknownArgument
    );
}

#[test]
fn test_help_contains_usage_info() {
    let mut cmd = CliArgs::command();
    let help = cmd.render_help().to_string();

    for expected in [
        "Extract thematic clips from video files",
        "PATH",
        "--strategy",
        "--resolution",
        "--audio",
        "--intro-exclusion",
        "--outro-exclusion",
        "--min-duration",
        "--max-duration",
        "action",
    ] {
        assert!(help.contains(expected));
    }
}

#[test]
fn test_duration_params_and_validation() {
    let args = parse_ok(&["video-clip-extractor", "/test/path"]);
    assert_eq!(args.min_duration, 20.0);
    assert_eq!(args.max_duration, 30.0);

    let args = parse_ok(&[
        "video-clip-extractor",
        "/test/path",
        "--min-duration",
        "5.0",
        "--max-duration",
        "20.0",
    ]);
    assert_eq!(args.min_duration, 5.0);
    assert_eq!(args.max_duration, 20.0);

    let valid_equal = parse_ok(&[
        "video-clip-extractor",
        "/test/path",
        "--min-duration",
        "10.0",
        "--max-duration",
        "10.0",
    ]);
    assert!(valid_equal.validate_duration_range().is_ok());

    let valid = parse_ok(&[
        "video-clip-extractor",
        "/test/path",
        "--min-duration",
        "8.0",
        "--max-duration",
        "12.0",
    ]);
    assert!(valid.validate_duration_range().is_ok());

    let invalid = parse_ok(&[
        "video-clip-extractor",
        "/test/path",
        "--min-duration",
        "15.0",
        "--max-duration",
        "10.0",
    ]);
    assert!(
        invalid
            .validate_duration_range()
            .unwrap_err()
            .contains("cannot be greater than")
    );

    assert!(
        CliArgs::try_parse_from(["video-clip-extractor", "/test/path", "--min-duration", "0"])
            .is_err()
    );
    assert!(
        CliArgs::try_parse_from([
            "video-clip-extractor",
            "/test/path",
            "--min-duration",
            "-5.0"
        ])
        .is_err()
    );
    assert!(
        CliArgs::try_parse_from([
            "video-clip-extractor",
            "/test/path",
            "--max-duration",
            "301.0"
        ])
        .is_err()
    );
}

#[test]
fn test_duration_with_other_flags() {
    let args = parse_ok(&[
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
fn test_exclusion_values_and_validation() {
    let args = parse_ok(&["video-clip-extractor", "/test/path"]);
    assert_eq!(args.intro_exclusion_percent, 2.0);
    assert_eq!(args.outro_exclusion_percent, 40.0);

    let args = parse_ok(&[
        "video-clip-extractor",
        "/test/path",
        "--intro-exclusion",
        "5.0",
        "--outro-exclusion",
        "30.0",
    ]);
    assert_eq!(args.intro_exclusion_percent, 5.0);
    assert_eq!(args.outro_exclusion_percent, 30.0);

    let args = parse_ok(&[
        "video-clip-extractor",
        "/test/path",
        "--intro-exclusion",
        "0",
        "--outro-exclusion",
        "0",
    ]);
    assert_eq!(args.intro_exclusion_percent, 0.0);
    assert_eq!(args.outro_exclusion_percent, 0.0);
    assert!(args.validate_exclusion_zones().is_ok());

    let args = parse_ok(&[
        "video-clip-extractor",
        "/test/path",
        "--intro-exclusion",
        "49.5",
        "--outro-exclusion",
        "49.5",
    ]);
    assert!(args.validate_exclusion_zones().is_ok());

    let args = parse_ok(&[
        "video-clip-extractor",
        "/test/path",
        "--intro-exclusion",
        "60",
        "--outro-exclusion",
        "40",
    ]);
    assert!(
        args.validate_exclusion_zones()
            .unwrap_err()
            .contains("must be less than 100%")
    );

    let args = parse_ok(&[
        "video-clip-extractor",
        "/test/path",
        "--intro-exclusion",
        "70",
        "--outro-exclusion",
        "50",
    ]);
    let err = args.validate_exclusion_zones().unwrap_err();
    assert!(err.contains("must be less than 100%"));
    assert!(err.contains("120%"));

    assert!(
        CliArgs::try_parse_from([
            "video-clip-extractor",
            "/test/path",
            "--intro-exclusion",
            "-5.0"
        ])
        .is_err()
    );
    assert!(
        CliArgs::try_parse_from([
            "video-clip-extractor",
            "/test/path",
            "--intro-exclusion",
            "101.0"
        ])
        .is_err()
    );
    assert!(
        CliArgs::try_parse_from([
            "video-clip-extractor",
            "/test/path",
            "--outro-exclusion",
            "-10.0"
        ])
        .is_err()
    );
    assert!(
        CliArgs::try_parse_from([
            "video-clip-extractor",
            "/test/path",
            "--outro-exclusion",
            "150.0"
        ])
        .is_err()
    );

    let boundary = parse_ok(&[
        "video-clip-extractor",
        "/test/path",
        "--intro-exclusion",
        "100",
        "--outro-exclusion",
        "100",
    ]);
    assert_eq!(boundary.intro_exclusion_percent, 100.0);
    assert_eq!(boundary.outro_exclusion_percent, 100.0);
}

#[test]
fn test_clip_count_values() {
    let args = parse_ok(&["video-clip-extractor", "/test/path"]);
    assert_eq!(args.clip_count, 1);

    for value in ["1", "2", "3", "4"] {
        let args = parse_ok(&["video-clip-extractor", "/test/path", "--clip-count", value]);
        assert_eq!(args.clip_count.to_string(), value);
    }

    let args = parse_ok(&["video-clip-extractor", "/test/path", "-c", "3"]);
    assert_eq!(args.clip_count, 3);

    let invalid = ["0", "5", "abc"];
    for value in invalid {
        let err =
            CliArgs::try_parse_from(["video-clip-extractor", "/test/path", "--clip-count", value])
                .unwrap_err();
        assert_eq!(err.kind(), clap::error::ErrorKind::ValueValidation);
    }

    assert!(
        CliArgs::try_parse_from(["video-clip-extractor", "/test/path", "--clip-count", "-1"])
            .is_err()
    );
}

#[test]
fn test_clip_count_with_other_flags() {
    let args = parse_ok(&[
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

proptest! {
    #[test]
    fn test_clip_count_validation_property(clip_count in any::<i32>()) {
        let clip_count_str = clip_count.to_string();
        let result = CliArgs::try_parse_from([
            "video-clip-extractor",
            "/test/path",
            "--clip-count",
            &clip_count_str,
        ]);

        if (1..=4).contains(&clip_count) {
            prop_assert!(result.is_ok(), "Valid clip count {} should be accepted", clip_count);
            if let Ok(args) = result {
                prop_assert_eq!(args.clip_count, clip_count as u8);
            }
        } else {
            prop_assert!(result.is_err(), "Invalid clip count {} should be rejected", clip_count);
        }
    }
}
