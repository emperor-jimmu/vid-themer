// FFmpeg command construction

use crate::cli::Resolution;
use crate::selector::TimeRange;
use std::ffi::OsString;
use std::path::Path;

use super::constants::{analysis, audio, color, encoding, fade, muxer, seeking};

/// Configuration for building FFmpeg extract commands
pub struct ExtractConfig<'a> {
    pub video_path: &'a Path,
    pub time_range: &'a TimeRange,
    pub output_path: &'a Path,
    pub source_resolution: (u32, u32),
    pub codec: &'a str,
    pub color_transfer: Option<&'a str>,
    pub target_resolution: Resolution,
    pub include_audio: bool,
    pub use_hw_accel: bool,
    /// Absolute stream index of the preferred audio stream (English first, fallback to first)
    pub audio_stream_index: Option<usize>,
}

/// Build audio-related FFmpeg arguments based on configuration
pub fn build_audio_args(include_audio: bool) -> Vec<OsString> {
    if !include_audio {
        vec!["-an".into()]
    } else {
        vec![
            "-af".into(),
            format!(
                "aformat=sample_fmts=fltp:channel_layouts=stereo,loudnorm=I={}:TP={}:LRA={},volume={}",
                audio::LOUDNESS_TARGET,
                audio::TRUE_PEAK,
                audio::LOUDNESS_RANGE,
                audio::VOLUME_REDUCTION
            ).into(),
            "-c:a".into(),
            audio::CODEC.into(),
            "-b:a".into(),
            audio::BITRATE.into(),
            "-ar".into(),
            audio::SAMPLE_RATE.into(),
        ]
    }
}

/// Build video codec arguments (hardware or software)
pub fn build_video_codec_args(use_hw_accel: bool) -> Vec<OsString> {
    if use_hw_accel {
        #[cfg(target_os = "macos")]
        {
            vec![
                "-c:v".into(),
                "h264_videotoolbox".into(),
                "-b:v".into(),
                encoding::HW_ACCEL_BITRATE.into(),
            ]
        }
        #[cfg(not(target_os = "macos"))]
        {
            vec![
                "-c:v".into(),
                "h264_nvenc".into(),
                "-preset".into(),
                "p4".into(),
                "-b:v".into(),
                encoding::HW_ACCEL_BITRATE.into(),
            ]
        }
    } else {
        vec![
            "-c:v".into(),
            "libx264".into(),
            "-preset".into(),
            encoding::PRESET.into(),
            "-crf".into(),
            encoding::CRF.into(),
            "-profile:v".into(),
            encoding::PROFILE.into(),
            "-level:v".into(),
            encoding::LEVEL.into(),
        ]
    }
}

/// Build color space arguments for SDR output
pub fn build_color_args() -> Vec<OsString> {
    vec![
        "-colorspace".into(),
        color::COLORSPACE.into(),
        "-color_primaries".into(),
        color::COLOR_PRIMARIES.into(),
        "-color_trc".into(),
        color::COLOR_TRC.into(),
        "-color_range".into(),
        color::COLOR_RANGE.into(),
    ]
}

/// Build GOP (Group of Pictures) arguments for streaming compatibility
pub fn build_gop_args() -> Vec<OsString> {
    vec![
        "-g".into(),
        encoding::GOP_SIZE.into(),
        "-keyint_min".into(),
        encoding::KEYINT_MIN.into(),
        "-sc_threshold".into(),
        encoding::SC_THRESHOLD.into(),
    ]
}

/// Calculate the scale filter for FFmpeg based on target resolution
/// Returns None if source resolution is smaller than target (no upscaling)
/// Returns Some(filter_string) with letterboxing if scaling is needed
pub fn calculate_scale_filter(
    source_resolution: (u32, u32),
    target_resolution: Resolution,
) -> Option<String> {
    let (source_width, source_height) = source_resolution;

    // Determine target resolution based on configuration
    let (target_width, target_height) = match target_resolution {
        Resolution::Hd720 => (1280u32, 720u32),
        Resolution::Hd1080 => (1920u32, 1080u32),
    };

    // No upscaling: if source is smaller than target, return None
    if source_width <= target_width && source_height <= target_height {
        return None;
    }

    // Generate scale filter with letterboxing
    let filter = format!(
        "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2",
        target_width, target_height, target_width, target_height
    );

    Some(filter)
}

/// Build video filter chain
pub fn build_video_filters(
    source_resolution: (u32, u32),
    target_resolution: Resolution,
    color_transfer: Option<&str>,
) -> String {
    let mut filters = Vec::new();

    // Detect HDR based on color transfer characteristics only.
    // Only PQ (smpte2084) and HLG (arib-std-b67) are true HDR transfer functions.
    // 10-bit pixel formats (yuv420p10le) are common in SDR HEVC encodes and do NOT
    // indicate HDR — applying zscale tone mapping to these causes "no path between
    // colorspaces" errors because there's no HDR-to-SDR conversion to perform.
    let is_hdr = match color_transfer {
        Some(transfer) => transfer == "smpte2084" || transfer == "arib-std-b67",
        None => false,
    };

    if is_hdr {
        // HDR to SDR tone mapping with explicit input colorspace specification
        // This approach specifies both input and output colorspaces to avoid "no path" errors
        // Reference: https://ffmpeg.org/ffmpeg-filters.html#tonemap
        filters.push("zscale=transfer=linear:primaries=input:matrix=input:range=input".to_string());
        filters.push("tonemap=tonemap=hable:desat=0".to_string());
        filters
            .push("zscale=transfer=bt709:primaries=bt709:matrix=bt709:range=limited".to_string());
        filters.push(format!("format={}", encoding::PIX_FMT));
    } else {
        // For SDR sources, just ensure proper pixel format
        filters.push(format!("format={}", encoding::PIX_FMT));
    }

    // Set color metadata explicitly for browser compatibility
    // This ensures the output has proper BT.709 tags regardless of input
    filters.push(
        "setparams=color_primaries=bt709:color_trc=bt709:colorspace=bt709:range=tv".to_string(),
    );

    // Add scale filter if needed (downscaling only, no upscaling)
    if let Some(scale_filter) = calculate_scale_filter(source_resolution, target_resolution) {
        filters.push(scale_filter);
    }

    filters.join(",")
}

/// Build seeking arguments based on codec type
pub fn build_seeking_args(time_range: &TimeRange, codec: &str) -> (Vec<OsString>, Vec<OsString>) {
    let is_hevc = codec == "hevc" || codec == "h265";

    let fast_seek_offset = if is_hevc {
        seeking::HEVC_FAST_SEEK_OFFSET
    } else {
        seeking::H264_FAST_SEEK_OFFSET
    };

    let fast_seek_pos = if time_range.start_seconds > fast_seek_offset {
        time_range.start_seconds - fast_seek_offset
    } else {
        0.0
    };

    let mut before_input = Vec::new();
    let mut after_input = Vec::new();

    // Add HEVC-specific buffer settings
    if is_hevc {
        before_input.extend([
            OsString::from("-analyzeduration"),
            analysis::HEVC_BUFFER_SIZE.into(),
            "-probesize".into(),
            analysis::HEVC_BUFFER_SIZE.into(),
        ]);
    }

    // Add fast seek if applicable
    if fast_seek_pos > 0.0 {
        before_input.extend([
            OsString::from("-ss"),
            fast_seek_pos.to_string().into(),
            "-noaccurate_seek".into(),
        ]);
    }

    // Add accurate seek
    let accurate_seek_pos = time_range.start_seconds - fast_seek_pos;
    if accurate_seek_pos > 0.0 {
        after_input.extend([OsString::from("-ss"), accurate_seek_pos.to_string().into()]);
    }

    (before_input, after_input)
}

/// Build FFmpeg command for extracting a clip
pub fn build_extract_command(config: &ExtractConfig) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["-err_detect".into(), "ignore_err".into()];

    // Build seeking arguments
    let (before_input, after_input) = build_seeking_args(config.time_range, config.codec);
    args.extend(before_input);

    // Input file (pass as OsStr to preserve non-UTF-8 path bytes)
    args.push("-i".into());
    args.push(config.video_path.into());

    // Accurate seek (after input)
    args.extend(after_input);

    // Timestamp handling - ensure proper timing for browser playback
    args.extend(["-avoid_negative_ts".into(), "make_zero".into()]);

    // Duration and stream mapping
    // Use absolute stream index for audio to prefer English track
    let audio_map = match config.audio_stream_index {
        Some(idx) => format!("0:{}?", idx),
        None => "0:a:0?".to_string(),
    };
    args.extend([
        "-t".into(),
        config.time_range.duration_seconds.to_string().into(),
        "-map".into(),
        "0:v:0".into(),
        "-map".into(),
        audio_map.into(),
        "-map_metadata".into(),
        "-1".into(),
    ]);

    // Video codec
    args.extend(build_video_codec_args(config.use_hw_accel));

    // Pixel format and GOP settings
    args.extend(["-pix_fmt".into(), encoding::PIX_FMT.into()]);
    args.extend(build_gop_args());

    args.extend(build_color_args());

    // Video filters
    let filters = build_video_filters(
        config.source_resolution,
        config.target_resolution,
        config.color_transfer,
    );
    args.extend(["-vf".into(), filters.into()]);

    // Audio
    args.extend(build_audio_args(config.include_audio));

    // Muxer options - faststart moves moov atom to beginning for browser streaming
    args.extend(["-movflags".into(), muxer::MOVFLAGS.into()]);

    // Output (pass as OsStr to preserve non-UTF-8 path bytes)
    args.push("-y".into());
    args.push(config.output_path.into());

    args
}

/// Build FFmpeg command for applying fade effects
pub fn build_fade_command(input_path: &Path, output_path: &Path, duration: f64) -> Vec<OsString> {
    let fade_out_start = duration - fade::FADE_OUT_DURATION;

    let mut args: Vec<OsString> = vec!["-i".into()];
    args.push(input_path.into());
    args.extend([
        "-vf".into(),
        format!(
            "fade=type=in:duration={}:start_time=0,fade=type=out:duration={}:start_time={}",
            fade::FADE_IN_DURATION,
            fade::FADE_OUT_DURATION,
            fade_out_start
        )
        .into(),
        "-af".into(),
        format!(
            "afade=type=in:duration={}:start_time=0,afade=type=out:duration={}:start_time={}",
            fade::FADE_IN_DURATION,
            fade::FADE_OUT_DURATION,
            fade_out_start
        )
        .into(),
    ]);
    args.extend(build_video_codec_args(false));
    args.extend(["-pix_fmt".into(), encoding::PIX_FMT.into()]);
    args.extend(build_gop_args());
    args.extend(build_color_args());
    args.extend([
        "-c:a".into(),
        audio::CODEC.into(),
        "-b:a".into(),
        audio::BITRATE.into(),
        "-ar".into(),
        audio::SAMPLE_RATE.into(),
        "-movflags".into(),
        muxer::MOVFLAGS.into(),
        "-y".into(),
    ]);
    args.push(output_path.into());
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_scale_filter_no_upscaling() {
        let result = calculate_scale_filter((1280, 720), Resolution::Hd1080);
        assert_eq!(result, None);
    }

    #[test]
    fn test_calculate_scale_filter_downscaling() {
        let result = calculate_scale_filter((3840, 2160), Resolution::Hd1080);
        assert!(result.is_some());
        let filter = result.unwrap();
        assert!(filter.contains("scale=1920:1080"));
    }

    #[test]
    fn test_build_audio_args_with_audio() {
        let args = build_audio_args(true);
        assert!(args.contains(&OsString::from("-c:a")));
        assert!(args.contains(&OsString::from("aac")));
        assert!(!args.contains(&OsString::from("-an")));
    }

    #[test]
    fn test_build_audio_args_without_audio() {
        let args = build_audio_args(false);
        assert!(args.contains(&OsString::from("-an")));
        assert!(!args.contains(&OsString::from("-c:a")));
    }
}
