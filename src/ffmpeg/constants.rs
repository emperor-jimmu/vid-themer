// FFmpeg encoding and processing constants

/// Video encoding settings
pub mod encoding {
    /// Target bitrate for hardware-accelerated encoding (5 Mbps)
    pub const HW_ACCEL_BITRATE: &str = "5M";

    /// CRF value for software encoding (26 = good quality for background/ambient clips)
    pub const CRF: &str = "26";

    /// Encoding preset for libx264
    pub const PRESET: &str = "medium";

    /// H.264 Profile for web compatibility
    pub const PROFILE: &str = "high";

    /// H.264 Level (4.0 supports up to 1080p @ 30fps, maximum compatibility)
    pub const LEVEL: &str = "4.0";

    /// Output pixel format (8-bit yuv420p for maximum compatibility)
    pub const PIX_FMT: &str = "yuv420p";

    /// Fixed GOP size for streaming compatibility (48 frames = 2 seconds at 24fps)
    /// Smaller GOP for short clips ensures better browser compatibility
    pub const GOP_SIZE: &str = "48";

    /// Minimum keyframe interval
    pub const KEYINT_MIN: &str = "48";

    /// Scene-cut detection threshold (0 = disabled for fixed GOP structure)
    pub const SC_THRESHOLD: &str = "0";
}

/// Color space settings for SDR output
pub mod color {
    /// Color space (BT.709 for HD)
    pub const COLORSPACE: &str = "bt709";

    /// Color primaries (BT.709 for HD)
    pub const COLOR_PRIMARIES: &str = "bt709";

    /// Transfer characteristics (BT.709 for HD)
    pub const COLOR_TRC: &str = "bt709";

    /// Color range (TV range for maximum compatibility)
    pub const COLOR_RANGE: &str = "tv";
}

/// Audio encoding settings
pub mod audio {
    /// Audio codec
    pub const CODEC: &str = "aac";

    /// Audio bitrate
    pub const BITRATE: &str = "128k";

    /// Sample rate (48 kHz for maximum compatibility)
    pub const SAMPLE_RATE: &str = "48000";

    /// Volume reduction factor (0.25 = 25%)
    pub const VOLUME_REDUCTION: f32 = 0.25;

    /// Loudness normalization target (EBU R128)
    pub const LOUDNESS_TARGET: &str = "-16";

    /// True peak limit
    pub const TRUE_PEAK: &str = "-1.5";

    /// Loudness range
    pub const LOUDNESS_RANGE: &str = "11";
}

/// Seeking optimization settings
pub mod seeking {
    /// Fast seek offset for H.264 videos (seconds before target)
    /// Larger offset = faster seeking but more decoding needed
    pub const H264_FAST_SEEK_OFFSET: f64 = 5.0;

    /// Fast seek offset for HEVC videos (seconds before target)
    /// Smaller offset for HEVC due to more complex decoding
    pub const HEVC_FAST_SEEK_OFFSET: f64 = 2.0;
}

/// Analysis settings
pub mod analysis {
    /// Maximum duration to analyze for long videos (5 minutes)
    /// Limits processing time while providing representative samples
    pub const MAX_ANALYSIS_DURATION: f64 = 300.0;

    /// Duration of each analysis segment (12.5 seconds)
    /// Balances granularity with statistical significance
    pub const SEGMENT_DURATION: f64 = 12.5;

    /// HEVC buffer size for analyzeduration and probesize (100 MB)
    /// Larger buffers help with HEVC's more complex structure
    pub const HEVC_BUFFER_SIZE: &str = "100M";
}

/// MP4 muxer options
pub mod muxer {
    /// MP4 muxer flags for streaming compatibility
    /// faststart: Move moov atom to beginning for streaming (critical for browser playback)
    /// Note: Removed frag_keyframe as it can cause playback issues in some browsers
    pub const MOVFLAGS: &str = "+faststart";
}

/// Fade effect settings
pub mod fade {
    /// Fade-in duration in seconds
    pub const FADE_IN_DURATION: f64 = 1.0;

    /// Fade-out duration in seconds
    pub const FADE_OUT_DURATION: f64 = 1.0;
}
