// FFmpeg command construction

use crate::cli::Resolution;
use crate::selector::TimeRange;
use std::path::Path;

use super::constants::{analysis, audio, color, encoding, fade, muxer, seeking};

/// Build audio-related FFmpeg arguments based on configuration
pub fn build_audio_args(include_audio: bool) -> Vec<String> {
    if !include_audio {
        // Exclude audio track
        vec!["-an".to_string()]
    } else {
        // Include audio with AAC codec
        // Apply loudness normalization (EBU R128) then reduce volume
        // Downmix to stereo to handle complex channel layouts (e.g., 5.1.2 Dolby Atmos)
        vec![
            "-af".to_string(),
            format!(
                "loudnorm=I={}:TP={}:LRA={},volume={}",
                audio::LOUDNESS_TARGET,
                audio::TRUE_PEAK,
                audio::LOUDNESS_RANGE,
                audio::VOLUME_REDUCTION
            ),
            "-c:a".to_string(),
            audio::CODEC.to_string(),
            "-b:a".to_string(),
            audio::BITRATE.to_string(),
            "-ar".to_string(),
            audio::SAMPLE_RATE.to_string(),
            "-ac".to_string(),
            audio::CHANNELS.to_string(),
        ]
    }
}

/// Build video codec arguments (hardware or software)
pub fn build_video_codec_args(use_hw_accel: bool) -> Vec<String> {
    if use_hw_accel {
        // Hardware acceleration uses platform-specific encoders
        #[cfg(target_os = "macos")]
        {
            vec![
                "-c:v".to_string(),
                "h264_videotoolbox".to_string(),
                "-b:v".to_string(),
                encoding::HW_ACCEL_BITRATE.to_string(),
            ]
        }
        #[cfg(not(target_os = "macos"))]
        {
            vec![
                "-c:v".to_string(),
                "h264_nvenc".to_string(),
                "-preset".to_string(),
                "p4".to_string(),
                "-b:v".to_string(),
                encoding::HW_ACCEL_BITRATE.to_string(),
            ]
        }
    } else {
        // Software encoding with libx264
        vec![
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            encoding::PRESET.to_string(),
            "-crf".to_string(),
            encoding::CRF.to_string(),
            "-profile:v".to_string(),
            encoding::PROFILE.to_string(),
            "-level:v".to_string(),
            encoding::LEVEL.to_string(),
        ]
    }
}

/// Build color space arguments for SDR output
pub fn build_color_args() -> Vec<String> {
    vec![
        "-colorspace".to_string(),
        color::COLORSPACE.to_string(),
        "-color_primaries".to_string(),
        color::COLOR_PRIMARIES.to_string(),
        "-color_trc".to_string(),
        color::COLOR_TRC.to_string(),
        "-color_range".to_string(),
        color::COLOR_RANGE.to_string(),
    ]
}

/// Build GOP (Group of Pictures) arguments for streaming compatibility
pub fn build_gop_args() -> Vec<String> {
    vec![
        "-g".to_string(),
        encoding::GOP_SIZE.to_string(),
        "-keyint_min".to_string(),
        encoding::KEYINT_MIN.to_string(),
        "-sc_threshold".to_string(),
        encoding::SC_THRESHOLD.to_string(),
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
    codec: &str,
) -> String {
    let mut filters = Vec::new();

    // Check if source is HDR
    let is_hdr = codec.contains("hevc") || codec.contains("h265") || codec.contains("vp9");

    if is_hdr {
        // HDR to SDR tone mapping
        filters.push("zscale=t=linear:npl=100,format=gbrpf32le,zscale=p=bt709,tonemap=tonemap=hable:desat=0,zscale=t=bt709:m=bt709:r=tv,format=yuv420p".to_string());
    } else {
        // For SDR sources, ensure proper format
        filters.push(format!("format={}", encoding::PIX_FMT));
    }

    // Add scale filter if needed
    if let Some(scale_filter) = calculate_scale_filter(source_resolution, target_resolution) {
        filters.push(scale_filter);
    }

    filters.join(",")
}

/// Build seeking arguments based on codec type
pub fn build_seeking_args(
    time_range: &TimeRange,
    codec: &str,
) -> (Vec<String>, Vec<String>) {
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
        before_input.extend(vec![
            "-analyzeduration".to_string(),
            analysis::HEVC_BUFFER_SIZE.to_string(),
            "-probesize".to_string(),
            analysis::HEVC_BUFFER_SIZE.to_string(),
        ]);
    }

    // Add fast seek if applicable
    if fast_seek_pos > 0.0 {
        before_input.extend(vec![
            "-ss".to_string(),
            fast_seek_pos.to_string(),
            "-noaccurate_seek".to_string(),
        ]);
    }

    // Add accurate seek
    let accurate_seek_pos = time_range.start_seconds - fast_seek_pos;
    if accurate_seek_pos > 0.0 {
        after_input.extend(vec![
            "-ss".to_string(),
            accurate_seek_pos.to_string(),
        ]);
    }

    (before_input, after_input)
}

/// Build FFmpeg command for extracting a clip
pub fn build_extract_command(
    video_path: &Path,
    time_range: &TimeRange,
    output_path: &Path,
    source_resolution: (u32, u32),
    codec: &str,
    target_resolution: Resolution,
    include_audio: bool,
    use_hw_accel: bool,
) -> Vec<String> {
    let mut args = vec![
        "-err_detect".to_string(),
        "ignore_err".to_string(),
    ];

    // Build seeking arguments
    let (before_input, after_input) = build_seeking_args(time_range, codec);
    args.extend(before_input);

    // Input file
    args.extend(vec![
        "-i".to_string(),
        video_path.to_string_lossy().to_string(),
    ]);

    // Accurate seek (after input)
    args.extend(after_input);

    // Timestamp handling
    args.extend(vec![
        "-avoid_negative_ts".to_string(),
        "make_zero".to_string(),
    ]);

    // Duration and stream mapping
    args.extend(vec![
        "-t".to_string(),
        time_range.duration_seconds.to_string(),
        "-map".to_string(),
        "0:v:0".to_string(),
        "-map".to_string(),
        "0:a:0?".to_string(),
        "-map_metadata".to_string(),
        "-1".to_string(),
    ]);

    // Video codec
    args.extend(build_video_codec_args(use_hw_accel));

    // Pixel format and GOP settings
    args.extend(vec![
        "-pix_fmt".to_string(),
        encoding::PIX_FMT.to_string(),
    ]);
    args.extend(build_gop_args());
    args.extend(build_color_args());

    // Video filters
    let filters = build_video_filters(source_resolution, target_resolution, codec);
    args.extend(vec!["-vf".to_string(), filters]);

    // Audio
    args.extend(build_audio_args(include_audio));

    // Muxer options
    args.extend(vec![
        "-movflags".to_string(),
        muxer::MOVFLAGS.to_string(),
    ]);

    // Output
    args.extend(vec![
        "-y".to_string(),
        output_path.to_string_lossy().to_string(),
    ]);

    args
}

/// Build FFmpeg command for applying fade effects
pub fn build_fade_command(
    input_path: &Path,
    output_path: &Path,
    duration: f64,
) -> Vec<String> {
    let fade_out_start = duration - fade::FADE_OUT_DURATION;

    vec![
        "-i".to_string(),
        input_path.to_string_lossy().to_string(),
        "-vf".to_string(),
        format!(
            "fade=type=in:duration={}:start_time=0,fade=type=out:duration={}:start_time={}",
            fade::FADE_IN_DURATION,
            fade::FADE_OUT_DURATION,
            fade_out_start
        ),
        "-af".to_string(),
        format!(
            "afade=type=in:duration={}:start_time=0,afade=type=out:duration={}:start_time={}",
            fade::FADE_IN_DURATION,
            fade::FADE_OUT_DURATION,
            fade_out_start
        ),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        encoding::PRESET.to_string(),
        "-crf".to_string(),
        encoding::CRF.to_string(),
        "-profile:v".to_string(),
        encoding::PROFILE.to_string(),
        "-level:v".to_string(),
        encoding::LEVEL.to_string(),
        "-pix_fmt".to_string(),
        encoding::PIX_FMT.to_string(),
        "-g".to_string(),
        encoding::GOP_SIZE.to_string(),
        "-keyint_min".to_string(),
        encoding::KEYINT_MIN.to_string(),
        "-sc_threshold".to_string(),
        encoding::SC_THRESHOLD.to_string(),
        "-colorspace".to_string(),
        color::COLORSPACE.to_string(),
        "-color_primaries".to_string(),
        color::COLOR_PRIMARIES.to_string(),
        "-color_trc".to_string(),
        color::COLOR_TRC.to_string(),
        "-color_range".to_string(),
        color::COLOR_RANGE.to_string(),
        "-c:a".to_string(),
        audio::CODEC.to_string(),
        "-b:a".to_string(),
        audio::BITRATE.to_string(),
        "-ar".to_string(),
        audio::SAMPLE_RATE.to_string(),
        "-movflags".to_string(),
        muxer::MOVFLAGS.to_string(),
        "-y".to_string(),
        output_path.to_string_lossy().to_string(),
    ]
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
        assert!(args.contains(&"-c:a".to_string()));
        assert!(args.contains(&"aac".to_string()));
        assert!(!args.contains(&"-an".to_string()));
    }

    #[test]
    fn test_build_audio_args_without_audio() {
        let args = build_audio_args(false);
        assert!(args.contains(&"-an".to_string()));
        assert!(!args.contains(&"-c:a".to_string()));
    }
}
