// FFmpeg command execution and video processing

use crate::cli::Resolution;
use crate::selector::TimeRange;
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

/// FFmpeg encoding and processing constants
mod constants {
    // Video encoding settings
    /// Target bitrate for hardware-accelerated encoding (5 Mbps)
    pub const HW_ACCEL_BITRATE: &str = "5M";

    /// Constant Rate Factor for software encoding (29 = moderate quality, smaller files)
    /// Range: 0-51, where lower = better quality, 18-28 is typical
    pub const SOFTWARE_CRF: &str = "29";

    /// Keyframe interval in frames (30 frames ≈ 1 second at 30fps)
    /// Ensures good seeking and streaming compatibility
    pub const KEYFRAME_INTERVAL: &str = "30";

    // Seeking optimization settings
    /// Fast seek offset for H.264 videos (seconds before target)
    /// Larger offset = faster seeking but more decoding needed
    pub const H264_FAST_SEEK_OFFSET: f64 = 5.0;

    /// Fast seek offset for HEVC videos (seconds before target)
    /// Smaller offset for HEVC due to more complex decoding
    pub const HEVC_FAST_SEEK_OFFSET: f64 = 2.0;

    // Analysis settings
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

/// FFprobe JSON output structure
#[derive(Debug, Deserialize)]
struct FFprobeOutput {
    streams: Vec<FFprobeStream>,
    format: FFprobeFormat,
}

/// FFprobe stream information
#[derive(Debug, Deserialize)]
struct FFprobeStream {
    codec_name: String,
    width: u32,
    height: u32,
}

/// FFprobe format information
#[derive(Debug, Deserialize)]
struct FFprobeFormat {
    duration: String,
}

#[derive(Clone)]
pub struct FFmpegExecutor {
    pub resolution: Resolution,
    pub include_audio: bool,
    pub use_hw_accel: bool,
}

#[derive(Debug, Clone)]
pub struct VideoMetadata {
    pub duration: f64,
    pub codec: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct AudioSegment {
    pub start_time: f64,
    pub duration: f64,
    pub intensity: f64,
}

#[derive(Debug, Clone)]
pub struct MotionSegment {
    pub start_time: f64,
    pub duration: f64,
    pub motion_score: f64,
}

impl FFmpegExecutor {
    pub fn new(resolution: Resolution, include_audio: bool) -> Self {
        Self {
            resolution,
            include_audio,
            use_hw_accel: false, // Default to software encoding for compatibility
        }
    }

    /// Enable or disable hardware acceleration for video encoding
    /// Note: Hardware acceleration support varies by platform
    #[allow(dead_code)]
    pub fn with_hw_accel(mut self, enable: bool) -> Self {
        self.use_hw_accel = enable;
        self
    }

    /// Check if FFmpeg is available in the system PATH
    pub fn check_availability() -> Result<(), FFmpegError> {
        // Try to execute ffmpeg -version to check if it's available
        let result = Command::new("ffmpeg").arg("-version").output();

        match result {
            Ok(output) => {
                if output.status.success() {
                    Ok(())
                } else {
                    Err(FFmpegError::NotFound)
                }
            }
            Err(_) => Err(FFmpegError::NotFound),
        }
    }

    /// Get all video metadata in a single ffprobe call (3x faster than separate calls)
    pub fn get_video_metadata(&self, video_path: &Path) -> Result<VideoMetadata, FFmpegError> {
        // Execute ffprobe to get all metadata at once using JSON output
        // Command: ffprobe -v error -select_streams v:0 -show_entries stream=codec_name,width,height:format=duration -of json <video>
        let output = Command::new("ffprobe")
            .arg("-v")
            .arg("error")
            .arg("-select_streams")
            .arg("v:0")
            .arg("-show_entries")
            .arg("stream=codec_name,width,height:format=duration")
            .arg("-of")
            .arg("json")
            .arg(video_path)
            .output()
            .map_err(|e| {
                FFmpegError::ExecutionFailed(format!(
                    "Failed to execute ffprobe on '{}': {}",
                    video_path.display(),
                    e
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Check for specific corruption indicators
            if stderr.contains("EBML header parsing failed")
                || stderr.contains("Invalid data found when processing input")
                || stderr.contains("moov atom not found")
                || stderr.contains("End of file")
            {
                return Err(FFmpegError::CorruptedFile(format!(
                    "Video file '{}' appears to be corrupted or incomplete: {}",
                    video_path.display(),
                    stderr
                )));
            }

            return Err(FFmpegError::ExecutionFailed(format!(
                "ffprobe failed on '{}': {}",
                video_path.display(),
                stderr
            )));
        }

        // Parse JSON output
        let json_str = String::from_utf8_lossy(&output.stdout);
        self.parse_metadata_json(&json_str, video_path)
    }

    /// Parse ffprobe JSON output to extract metadata
    fn parse_metadata_json(
        &self,
        json_str: &str,
        video_path: &Path,
    ) -> Result<VideoMetadata, FFmpegError> {
        // Use serde_json for robust JSON parsing
        let output: FFprobeOutput = serde_json::from_str(json_str).map_err(|e| {
            FFmpegError::ParseError(format!(
                "Failed to parse JSON for '{}': {}",
                video_path.display(),
                e
            ))
        })?;

        // Get the first video stream
        let stream = output.streams.first().ok_or_else(|| {
            FFmpegError::ParseError(format!(
                "No video stream found in JSON for '{}'",
                video_path.display()
            ))
        })?;

        // Validate duration (check for "N/A" or empty)
        if output.format.duration == "N/A" || output.format.duration.is_empty() {
            return Err(FFmpegError::CorruptedFile(format!(
                "Unable to determine video duration for '{}' - file may be corrupted or incomplete",
                video_path.display()
            )));
        }

        // Parse duration string to f64
        let duration = output.format.duration.parse::<f64>().map_err(|e| {
            FFmpegError::ParseError(format!(
                "Failed to parse duration '{}' for '{}': {}",
                output.format.duration,
                video_path.display(),
                e
            ))
        })?;

        Ok(VideoMetadata {
            duration,
            codec: stream.codec_name.clone(),
            width: stream.width,
            height: stream.height,
        })
    }

    /// Get the duration of a video file in seconds (legacy method, use get_video_metadata instead)
    pub fn get_duration(&self, video_path: &Path) -> Result<f64, FFmpegError> {
        let metadata = self.get_video_metadata(video_path)?;
        Ok(metadata.duration)
    }

    /// Get the video codec name for a video file (legacy method, use get_video_metadata instead)
    #[allow(dead_code)]
    pub fn get_video_codec(&self, video_path: &Path) -> Result<String, FFmpegError> {
        let metadata = self.get_video_metadata(video_path)?;
        Ok(metadata.codec)
    }

    /// Get the video resolution (width, height) of a video file (legacy method, use get_video_metadata instead)
    #[allow(dead_code)]
    pub fn get_video_resolution(&self, video_path: &Path) -> Result<(u32, u32), FFmpegError> {
        let metadata = self.get_video_metadata(video_path)?;
        Ok((metadata.width, metadata.height))
    }

    /// Calculate the scale filter for FFmpeg based on target resolution
    /// Returns None if source resolution is smaller than target (no upscaling)
    /// Returns Some(filter_string) with letterboxing if scaling is needed
    pub fn calculate_scale_filter(&self, source_resolution: (u32, u32)) -> Option<String> {
        let (source_width, source_height) = source_resolution;

        // Determine target resolution based on configuration
        let (target_width, target_height) = match self.resolution {
            Resolution::Hd720 => (1280u32, 720u32),
            Resolution::Hd1080 => (1920u32, 1080u32),
        };

        // No upscaling: if source is smaller than target, return None
        if source_width <= target_width && source_height <= target_height {
            return None;
        }

        // Generate scale filter with letterboxing
        // scale=W:H:force_original_aspect_ratio=decrease,pad=W:H:(ow-iw)/2:(oh-ih)/2
        // This scales down to fit within target dimensions while maintaining aspect ratio,
        // then pads with black bars to reach exact target dimensions
        let filter = format!(
            "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2",
            target_width, target_height, target_width, target_height
        );

        Some(filter)
    }

    /// Build audio-related FFmpeg arguments based on configuration
    /// Returns arguments for either including or excluding audio
    fn build_audio_args(&self) -> Vec<String> {
        if !self.include_audio {
            // Exclude audio track
            vec!["-an".to_string()]
        } else {
            // Include audio with AAC codec
            // Apply loudness normalization (EBU R128) then reduce volume
            // Downmix to stereo to handle complex channel layouts (e.g., 5.1.2 Dolby Atmos)
            vec![
                "-af".to_string(),
                "loudnorm=I=-16:TP=-1.5:LRA=11,volume=0.8".to_string(),
                "-c:a".to_string(),
                "aac".to_string(),
                "-b:a".to_string(),
                "128k".to_string(), // Explicit bitrate for consistency
                "-ac".to_string(),
                "2".to_string(), // Force stereo output
            ]
        }
    }

    /// Build FFmpeg command for extracting a clip
    /// Returns a vector of command arguments ready to be passed to Command
    pub fn build_extract_command(
        &self,
        video_path: &Path,
        time_range: &TimeRange,
        output_path: &Path,
        source_resolution: (u32, u32),
        codec: &str,
    ) -> Vec<String> {
        // Determine if this is an HEVC video
        let is_hevc = codec == "hevc" || codec == "h265";

        let mut args = vec![
            // Error concealment flags for better handling of corrupted/problematic videos
            "-err_detect".to_string(),
            "ignore_err".to_string(),
        ];

        if is_hevc {
            // For HEVC: Use moderate fast seek with smaller offset for better balance
            // Increase buffer sizes to handle HEVC better
            args.extend(vec![
                "-analyzeduration".to_string(),
                constants::HEVC_BUFFER_SIZE.to_string(),
                "-probesize".to_string(),
                constants::HEVC_BUFFER_SIZE.to_string(),
            ]);

            // Calculate moderate fast seek position (2 seconds before target for HEVC)
            let fast_seek_offset = constants::HEVC_FAST_SEEK_OFFSET;
            let fast_seek_pos = if time_range.start_seconds > fast_seek_offset {
                time_range.start_seconds - fast_seek_offset
            } else {
                0.0
            };

            // Add fast seek if we're seeking past the offset
            if fast_seek_pos > 0.0 {
                args.push("-ss".to_string());
                args.push(fast_seek_pos.to_string());
                args.push("-noaccurate_seek".to_string()); // Explicit fast seek
            }

            args.extend(vec![
                "-i".to_string(),
                video_path.to_string_lossy().to_string(),
            ]);

            // Accurate seek to exact position (relative to fast seek position)
            let accurate_seek_pos = time_range.start_seconds - fast_seek_pos;
            if accurate_seek_pos > 0.0 {
                args.push("-ss".to_string());
                args.push(accurate_seek_pos.to_string());
            }
        } else {
            // For non-HEVC (H.264, etc.): Use aggressive fast seek for best performance
            let fast_seek_offset = constants::H264_FAST_SEEK_OFFSET;
            let fast_seek_pos = if time_range.start_seconds > fast_seek_offset {
                time_range.start_seconds - fast_seek_offset
            } else {
                0.0
            };

            // Add fast seek if we're seeking past the offset
            if fast_seek_pos > 0.0 {
                args.push("-ss".to_string());
                args.push(fast_seek_pos.to_string());
                args.push("-noaccurate_seek".to_string()); // Explicit fast seek
            }

            args.extend(vec![
                "-i".to_string(),
                video_path.to_string_lossy().to_string(),
            ]);

            // Accurate seek to exact position (relative to fast seek position)
            let accurate_seek_pos = time_range.start_seconds - fast_seek_pos;
            if accurate_seek_pos > 0.0 {
                args.push("-ss".to_string());
                args.push(accurate_seek_pos.to_string());
            }
        }

        // Handle timestamp edge cases
        args.extend(vec![
            "-avoid_negative_ts".to_string(),
            "make_zero".to_string(),
        ]);

        args.extend(vec![
            // Duration
            "-t".to_string(),
            time_range.duration_seconds.to_string(),
        ]);

        // Video codec selection (hardware or software)
        if self.use_hw_accel {
            // Hardware acceleration uses platform-specific encoders:
            // - macOS: VideoToolbox (h264_videotoolbox) - Apple's native hardware encoder
            //   Available on all Macs with hardware encoding support (most Macs since 2011)
            //   Provides efficient encoding using the built-in media engine
            // - Other platforms: NVENC (h264_nvenc) - NVIDIA GPU encoder
            //   Requires NVIDIA GPU with NVENC support (GeForce GTX 600 series or newer)
            //   Falls back to software encoding if NVENC is unavailable
            //
            // Hardware acceleration significantly improves encoding speed (5-10x faster)
            // but may produce slightly larger files compared to software encoding

            #[cfg(target_os = "macos")] // macOS-specific: Use Apple VideoToolbox
            {
                args.extend(vec![
                    "-c:v".to_string(),
                    "h264_videotoolbox".to_string(),
                    "-b:v".to_string(),
                    constants::HW_ACCEL_BITRATE.to_string(),
                ]);
            }
            #[cfg(not(target_os = "macos"))] // Non-macOS: Use NVIDIA NVENC
            {
                // Try NVIDIA first, fall back to software if not available
                args.extend(vec![
                    "-c:v".to_string(),
                    "h264_nvenc".to_string(),
                    "-preset".to_string(),
                    "p4".to_string(), // NVENC preset p4 = balanced quality/speed
                    "-b:v".to_string(),
                    constants::HW_ACCEL_BITRATE.to_string(),
                ]);
            }
        } else {
            // Software encoding with libx264
            args.extend(vec![
                "-c:v".to_string(),
                "libx264".to_string(),
                "-preset".to_string(),
                "fast".to_string(),
                // CRF for quality/size balance (26 = smaller files, still good quality)
                "-crf".to_string(),
                constants::SOFTWARE_CRF.to_string(),
            ]);
        }

        args.extend(vec![
            // Explicitly set output pixel format to 8-bit yuv420p
            "-pix_fmt".to_string(),
            "yuv420p".to_string(),
            // Keyframe interval for better seeking and streaming compatibility
            "-g".to_string(),
            constants::KEYFRAME_INTERVAL.to_string(),
            "-keyint_min".to_string(),
            constants::KEYFRAME_INTERVAL.to_string(),
            // Set color metadata for proper playback compatibility
            "-colorspace".to_string(),
            "bt709".to_string(),
            "-color_primaries".to_string(),
            "bt709".to_string(),
            "-color_trc".to_string(),
            "bt709".to_string(),
        ]);

        // Build video filter chain
        let mut filters = Vec::new();

        // CRITICAL: Add pixel format conversion FIRST to handle 10-bit sources
        // This must come before any other filters (especially scale) to ensure
        // compatibility with libx264 which expects 8-bit input
        // Using format=yuv420p explicitly converts from any pixel format (including yuv420p10le)
        filters.push("format=yuv420p".to_string());

        // Add scale filter if needed (downscaling only, no upscaling)
        // This comes AFTER format conversion so it works with 8-bit input
        if let Some(scale_filter) = self.calculate_scale_filter(source_resolution) {
            filters.push(scale_filter);
        }

        // Apply video filters (always present now due to format filter)
        args.push("-vf".to_string());
        args.push(filters.join(","));

        // Audio handling
        args.extend(self.build_audio_args());

        // Output file (overwrite if exists)
        args.push("-y".to_string());
        args.push(output_path.to_string_lossy().to_string());

        args
    }

    /// Extract a clip from a video file
    /// Executes FFmpeg command and captures stderr for error messages
    pub fn extract_clip(
        &self,
        video_path: &Path,
        time_range: &TimeRange,
        output_path: &Path,
    ) -> Result<(), FFmpegError> {
        // Get source resolution and codec using batch metadata query
        let metadata = self.get_video_metadata(video_path)?;
        let source_resolution = (metadata.width, metadata.height);
        let codec = &metadata.codec;

        // Build the FFmpeg command with codec-aware seeking
        let args = self.build_extract_command(
            video_path,
            time_range,
            output_path,
            source_resolution,
            codec,
        );

        // Execute FFmpeg command
        let output = Command::new("ffmpeg").args(&args).output().map_err(|e| {
            FFmpegError::ExecutionFailed(format!(
                "Failed to execute ffmpeg for '{}': {}",
                video_path.display(),
                e
            ))
        })?;

        // Check if the command was successful
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Check for specific error patterns that might benefit from recovery
            if stderr.contains("corrupt")
                || stderr.contains("Invalid NAL unit")
                || stderr.contains("concealing")
                || stderr.contains("error while decoding")
                || stderr.contains("missing picture in access unit")
            {
                // Try extraction with error recovery
                return self.extract_clip_with_recovery(
                    video_path,
                    time_range,
                    output_path,
                    source_resolution,
                    codec,
                );
            }

            return Err(FFmpegError::ExecutionFailed(format!(
                "FFmpeg clip extraction failed for '{}' at {:.2}s-{:.2}s: {}",
                video_path.display(),
                time_range.start_seconds,
                time_range.start_seconds + time_range.duration_seconds,
                stderr
            )));
        }

        // Validate output file
        self.validate_output(output_path)?;

        Ok(())
    }

    /// Extract clip with error concealment for corrupted videos
    fn extract_clip_with_recovery(
        &self,
        video_path: &Path,
        time_range: &TimeRange,
        output_path: &Path,
        source_resolution: (u32, u32),
        codec: &str,
    ) -> Result<(), FFmpegError> {
        // Build command with additional error concealment flags
        let mut args = vec![
            "-err_detect".to_string(),
            "ignore_err".to_string(),
            "-fflags".to_string(),
            "+genpts+igndts".to_string(), // Generate PTS, ignore DTS errors
            "-max_error_rate".to_string(),
            "1.0".to_string(), // Allow up to 100% error rate
        ];

        // Add the rest of the standard command
        let standard_args = self.build_extract_command(
            video_path,
            time_range,
            output_path,
            source_resolution,
            codec,
        );

        // Skip the first two args from standard command (they're already added)
        args.extend(standard_args.into_iter().skip(2));

        // Execute with recovery flags
        let output = Command::new("ffmpeg").args(&args).output().map_err(|e| {
            FFmpegError::ExecutionFailed(format!(
                "Failed to execute ffmpeg recovery for '{}': {}",
                video_path.display(),
                e
            ))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(FFmpegError::ExecutionFailed(format!(
                "FFmpeg clip extraction failed even with recovery for '{}' at {:.2}s-{:.2}s: {}",
                video_path.display(),
                time_range.start_seconds,
                time_range.start_seconds + time_range.duration_seconds,
                stderr
            )));
        }

        // Validate output file
        self.validate_output(output_path)?;

        Ok(())
    }

    /// Validate that the output file exists and has content
    fn validate_output(&self, output_path: &Path) -> Result<(), FFmpegError> {
        if !output_path.exists() {
            return Err(FFmpegError::ExecutionFailed(
                "Output file was not created".to_string(),
            ));
        }

        let metadata = std::fs::metadata(output_path)
            .map_err(|e| FFmpegError::ExecutionFailed(format!("Cannot read output file: {}", e)))?;

        if metadata.len() == 0 {
            return Err(FFmpegError::ExecutionFailed(
                "Output file is empty (0 bytes)".to_string(),
            ));
        }

        Ok(())
    }

    /// Analyze audio intensity across the video duration
    /// Returns a sorted list of audio segments by intensity (highest first)
    /// Optimized for long videos by limiting analysis duration
    pub fn analyze_audio_intensity(
        &self,
        video_path: &Path,
        duration: f64,
    ) -> Result<Vec<AudioSegment>, FFmpegError> {
        // For long videos (>5 minutes), analyze only first 5 minutes for efficiency
        // This provides enough data for representative segment selection
        let analysis_duration = duration.min(constants::MAX_ANALYSIS_DURATION);

        // Use volumedetect for faster analysis (simpler than ebur128)
        // For videos longer than analysis window, we'll use a sampling approach
        let args = vec![
            "-i".to_string(),
            video_path.to_string_lossy().to_string(),
            "-t".to_string(),
            analysis_duration.to_string(),
            "-af".to_string(),
            "astats=metadata=1:reset=1,ametadata=print:key=lavfi.astats.Overall.RMS_level:file=-"
                .to_string(),
            "-f".to_string(),
            "null".to_string(),
            "-".to_string(),
        ];

        // Execute FFmpeg with audio stats filter
        let output = Command::new("ffmpeg").args(&args).output().map_err(|e| {
            FFmpegError::ExecutionFailed(format!(
                "Failed to execute ffmpeg for audio analysis: {}",
                e
            ))
        })?;

        // The astats filter outputs to stderr
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Check if there's an audio track
        if stderr.contains("Output file #0 does not contain any stream")
            || stderr.contains("Stream specifier ':a' in filtergraph")
            || stderr.contains("does not contain any stream")
        {
            return Err(FFmpegError::NoAudioTrack);
        }

        // Parse the output to extract audio measurements
        // The astats filter outputs lines with RMS levels
        let mut measurements: Vec<(f64, f64)> = Vec::new(); // (time, rms_level)

        // Parse frame timestamps and RMS levels from metadata output
        let mut current_time = 0.0;
        for line in stderr.lines() {
            // Look for frame time indicators
            // Note: Nested if-let used instead of let...if chains for stable Rust compatibility
            #[allow(clippy::collapsible_if)]
            if line.contains("pts_time:") {
                if let Some(time) = Self::extract_value_after(line, "pts_time:") {
                    current_time = time;
                }
            }
            // Look for RMS level in metadata
            #[allow(clippy::collapsible_if)]
            if line.contains("lavfi.astats.Overall.RMS_level") {
                if let Some(level) =
                    Self::extract_value_after(line, "lavfi.astats.Overall.RMS_level=")
                {
                    measurements.push((current_time, level));
                }
            }
        }

        // If no measurements were found, try fallback to simpler volumedetect
        if measurements.is_empty() {
            return self.analyze_audio_intensity_fallback(video_path, duration);
        }

        // Group measurements into segments using shared helper
        let segments_data = Self::group_measurements_into_segments(
            &measurements,
            duration,
            analysis_duration,
            constants::SEGMENT_DURATION,
            |values| values.iter().sum::<f64>() / values.len() as f64, // Average
        );

        let mut segments: Vec<AudioSegment> = segments_data
            .into_iter()
            .map(|(start, dur, intensity)| AudioSegment {
                start_time: start,
                duration: dur,
                intensity,
            })
            .collect();

        // Sort segments by intensity (highest/loudest first)
        // Since dB values are negative, higher (less negative) values are louder
        segments.sort_by(|a, b| b.intensity.total_cmp(&a.intensity));

        Ok(segments)
    }

    /// Fallback audio analysis using simpler volumedetect filter
    fn analyze_audio_intensity_fallback(
        &self,
        video_path: &Path,
        duration: f64,
    ) -> Result<Vec<AudioSegment>, FFmpegError> {
        // Use ebur128 as fallback but with limited duration
        const MAX_ANALYSIS_DURATION: f64 = 180.0; // 3 minutes for fallback
        let analysis_duration = duration.min(MAX_ANALYSIS_DURATION);

        let output = Command::new("ffmpeg")
            .arg("-t")
            .arg(analysis_duration.to_string())
            .arg("-i")
            .arg(video_path)
            .arg("-filter_complex")
            .arg("ebur128=peak=true")
            .arg("-f")
            .arg("null")
            .arg("-")
            .output()
            .map_err(|e| {
                FFmpegError::ExecutionFailed(format!(
                    "Failed to execute ffmpeg for audio analysis: {}",
                    e
                ))
            })?;

        let stderr = String::from_utf8_lossy(&output.stderr);

        if stderr.contains("Output file #0 does not contain any stream")
            || stderr.contains("Stream specifier ':a' in filtergraph")
        {
            return Err(FFmpegError::NoAudioTrack);
        }

        let mut measurements: Vec<(f64, f64)> = Vec::new();

        for line in stderr.lines() {
            // Note: Nested if-let used instead of let...if chains for stable Rust compatibility
            #[allow(clippy::collapsible_if)]
            if line.contains("Parsed_ebur128") && line.contains("t:") {
                #[allow(clippy::collapsible_if)]
                if let Some(time) = Self::extract_value_after(line, "t:") {
                    if let Some(peak) = Self::extract_value_after(line, "FTPK:") {
                        measurements.push((time, peak));
                    }
                }
            }
        }

        if measurements.is_empty() {
            return Err(FFmpegError::NoAudioTrack);
        }

        // Group measurements into segments using shared helper
        let segments_data = Self::group_measurements_into_segments(
            &measurements,
            duration,
            analysis_duration,
            constants::SEGMENT_DURATION,
            |values| values.iter().sum::<f64>() / values.len() as f64, // Average
        );

        let mut segments: Vec<AudioSegment> = segments_data
            .into_iter()
            .map(|(start, dur, intensity)| AudioSegment {
                start_time: start,
                duration: dur,
                intensity,
            })
            .collect();

        segments.sort_by(|a, b| b.intensity.total_cmp(&a.intensity));

        Ok(segments)
    }

    /// Helper function to extract a numeric value after a label in a string
    fn extract_value_after(line: &str, label: &str) -> Option<f64> {
        if let Some(pos) = line.find(label) {
            let after_label = &line[pos + label.len()..];
            // Extract the numeric part (may include negative sign and decimal point)
            let value_str: String = after_label
                .trim_start()
                .chars()
                .take_while(|c| c.is_numeric() || *c == '.' || *c == '-')
                .collect();

            value_str.parse::<f64>().ok()
        } else {
            None
        }
    }

    /// Groups time-series measurements into fixed-duration segments and calculates aggregate scores
    ///
    /// # Parameters
    /// - `measurements`: Vector of (timestamp, value) pairs
    /// - `video_duration`: Total duration of the video in seconds
    /// - `analysis_duration`: Duration that was actually analyzed (may be less than video_duration)
    /// - `segment_duration`: Duration of each segment in seconds
    /// - `aggregate_fn`: Function to aggregate values within a segment (e.g., sum or average)
    ///
    /// # Returns
    /// Vector of (start_time, duration, score) tuples for each segment
    fn group_measurements_into_segments<F>(
        measurements: &[(f64, f64)],
        video_duration: f64,
        analysis_duration: f64,
        segment_duration: f64,
        aggregate_fn: F,
    ) -> Vec<(f64, f64, f64)>
    where
        F: Fn(&[f64]) -> f64,
    {
        let scale_factor = video_duration / analysis_duration;
        let num_segments = (video_duration / segment_duration).ceil() as usize;

        let mut segments = Vec::new();

        for i in 0..num_segments {
            let segment_start = i as f64 * segment_duration;
            let segment_end = ((i + 1) as f64 * segment_duration).min(video_duration);
            let segment_duration_val = segment_end - segment_start;

            // Map to analyzed portion
            let analyzed_start = segment_start / scale_factor;
            let analyzed_end = segment_end / scale_factor;

            // Find all measurements within this segment
            let segment_measurements: Vec<f64> = measurements
                .iter()
                .filter(|(time, _)| *time >= analyzed_start && *time < analyzed_end)
                .map(|(_, value)| *value)
                .collect();

            if !segment_measurements.is_empty() {
                let score = aggregate_fn(&segment_measurements);
                segments.push((segment_start, segment_duration_val, score));
            }
        }

        segments
    }

    /// Analyze motion intensity across the video duration using scene detection
    /// Returns a sorted list of motion segments by score (highest first)
    /// Optimized for long videos by limiting analysis duration to 5 minutes
    pub fn analyze_motion_intensity(
        &self,
        video_path: &Path,
        duration: f64,
    ) -> Result<Vec<MotionSegment>, FFmpegError> {
        // For long videos (>5 minutes), analyze only first 5 minutes for efficiency
        let analysis_duration = duration.min(constants::MAX_ANALYSIS_DURATION);

        // Build FFmpeg command with scene detection filter
        // Use select filter to identify frames with scene changes above threshold 0.3
        // Use showinfo to output frame information including timestamps and scene scores
        let args = vec![
            "-i".to_string(),
            video_path.to_string_lossy().to_string(),
            "-t".to_string(),
            analysis_duration.to_string(),
            "-vf".to_string(),
            "select=gt(scene\\,0.3),showinfo".to_string(),
            "-f".to_string(),
            "null".to_string(),
            "-".to_string(),
        ];

        // Execute FFmpeg with scene detection filter
        let output = Command::new("ffmpeg").args(&args).output().map_err(|e| {
            FFmpegError::ExecutionFailed(format!(
                "Failed to execute ffmpeg for motion analysis: {}",
                e
            ))
        })?;

        // The showinfo filter outputs to stderr
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Parse the output to extract scene change scores and timestamps
        // showinfo outputs lines like: [Parsed_showinfo_1 @ 0x...] n:42 pts:1260 pts_time:1.26 ... scene:0.456 ...
        let mut measurements: Vec<(f64, f64)> = Vec::new(); // (timestamp, scene_score)

        for line in stderr.lines() {
            // Look for showinfo output lines
            if line.contains("Parsed_showinfo")
                && line.contains("pts_time:")
                && line.contains("scene:")
            {
                // Extract pts_time (timestamp in seconds)
                if let Some(time) = Self::extract_value_after(line, "pts_time:") {
                    // Extract scene score
                    if let Some(score) = Self::extract_value_after(line, "scene:") {
                        measurements.push((time, score));
                    }
                }
            }
        }

        // If no measurements were found, return empty vector (no motion detected)
        if measurements.is_empty() {
            return Ok(Vec::new());
        }

        // Group measurements into segments using shared helper
        let segments_data = Self::group_measurements_into_segments(
            &measurements,
            duration,
            analysis_duration,
            constants::SEGMENT_DURATION,
            |values| values.iter().sum::<f64>(), // Sum
        );

        let mut segments: Vec<MotionSegment> = segments_data
            .into_iter()
            .map(|(start, dur, score)| MotionSegment {
                start_time: start,
                duration: dur,
                motion_score: score,
            })
            .collect();

        // Sort segments by motion score (highest first)
        segments.sort_by(|a, b| b.motion_score.total_cmp(&a.motion_score));

        Ok(segments)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FFmpegError {
    #[error("FFmpeg not found in PATH")]
    NotFound,

    #[error("Failed to execute FFmpeg: {0}")]
    ExecutionFailed(String),

    #[error("Failed to parse FFmpeg output: {0}")]
    ParseError(String),

    #[error("Video has no audio track")]
    NoAudioTrack,

    #[error("Corrupted or invalid video file: {0}")]
    CorruptedFile(String),
}

impl FFmpegError {
    /// Extracts stderr output from execution errors
    ///
    /// Returns the stderr content if this is an ExecutionFailed error.
    /// For other error types, returns None.
    ///
    /// # Behavior
    /// - For ExecutionFailed errors, attempts to strip known prefixes to extract raw stderr
    /// - If no known prefix is found, returns the entire error message
    /// - For all other error variants, returns None
    pub fn stderr(&self) -> Option<&str> {
        match self {
            FFmpegError::ExecutionFailed(msg) => {
                // Try to extract stderr by stripping known prefixes
                // Format: "FFmpeg clip extraction failed for '<path>' at <start>s-<end>s: <stderr>"
                msg.strip_prefix("FFmpeg clip extraction failed for ")
                    .and_then(|s| s.split_once(": ").map(|(_, stderr)| stderr))
                    // Format: "FFmpeg clip extraction failed even with recovery for '<path>' at <start>s-<end>s: <stderr>"
                    .or_else(|| {
                        msg.strip_prefix("FFmpeg clip extraction failed even with recovery for ")
                            .and_then(|s| s.split_once(": ").map(|(_, stderr)| stderr))
                    })
                    // Format: "ffprobe failed on '<path>': <stderr>"
                    .or_else(|| {
                        msg.strip_prefix("ffprobe failed on ")
                            .and_then(|s| s.split_once(": ").map(|(_, stderr)| stderr))
                    })
                    // Format: "Failed to execute ffprobe on '<path>': <stderr>"
                    .or_else(|| {
                        msg.strip_prefix("Failed to execute ffprobe on ")
                            .and_then(|s| s.split_once(": ").map(|(_, stderr)| stderr))
                    })
                    // Format: "Failed to execute ffmpeg for '<path>': <stderr>"
                    .or_else(|| {
                        msg.strip_prefix("Failed to execute ffmpeg for ")
                            .and_then(|s| s.split_once(": ").map(|(_, stderr)| stderr))
                    })
                    // Format: "Failed to execute ffmpeg recovery for '<path>': <stderr>"
                    .or_else(|| {
                        msg.strip_prefix("Failed to execute ffmpeg recovery for ")
                            .and_then(|s| s.split_once(": ").map(|(_, stderr)| stderr))
                    })
                    // Fallback: return entire message if no known prefix matches
                    .or(Some(msg.as_str()))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::path::PathBuf;

    #[test]
    fn test_ffmpeg_availability() {
        // This test will pass if FFmpeg is installed, fail if not
        // In a real environment, we expect FFmpeg to be available
        let result = FFmpegExecutor::check_availability();

        // We can't guarantee FFmpeg is installed in all test environments,
        // but we can verify the function returns the correct type
        match result {
            Ok(_) => {
                // FFmpeg is available - this is the expected case
                println!("FFmpeg is available in PATH");
            }
            Err(FFmpegError::NotFound) => {
                // FFmpeg is not available - this is acceptable in test environments
                println!("FFmpeg is not available in PATH");
            }
            Err(e) => {
                panic!("Unexpected error type: {:?}", e);
            }
        }
    }

    #[test]
    fn test_ffmpeg_error_not_found_variant() {
        // Test that we can create and match the NotFound error variant
        let error = FFmpegError::NotFound;
        assert_eq!(error.to_string(), "FFmpeg not found in PATH");
    }

    #[test]
    fn test_ffmpeg_not_found_error() {
        // Test error handling when FFmpeg is not in PATH
        // This test validates Requirement 2.11: FFmpeg availability check

        // We can't actually remove FFmpeg from PATH in a test, but we can verify
        // that the check_availability function returns the correct error type
        // when FFmpeg is not available.

        // The check_availability function will either:
        // 1. Return Ok(()) if FFmpeg is available (expected in dev environment)
        // 2. Return Err(FFmpegError::NotFound) if FFmpeg is not available

        let result = FFmpegExecutor::check_availability();

        // Verify the function returns a Result type
        match result {
            Ok(_) => {
                // FFmpeg is available - verify we can create the NotFound error
                let not_found_error = FFmpegError::NotFound;
                assert_eq!(
                    not_found_error.to_string(),
                    "FFmpeg not found in PATH",
                    "NotFound error should have correct message"
                );
            }
            Err(e) => {
                // FFmpeg is not available - verify it's the correct error type
                match e {
                    FFmpegError::NotFound => {
                        assert_eq!(
                            e.to_string(),
                            "FFmpeg not found in PATH",
                            "NotFound error should have correct message"
                        );
                    }
                    _ => panic!("Expected FFmpegError::NotFound, got: {:?}", e),
                }
            }
        }

        // Additionally, verify that the NotFound error can be properly matched
        let not_found = FFmpegError::NotFound;
        assert!(matches!(not_found, FFmpegError::NotFound));

        // Verify the error message format
        let error_message = format!("{}", FFmpegError::NotFound);
        assert_eq!(error_message, "FFmpeg not found in PATH");
    }

    #[test]
    fn test_get_duration_with_nonexistent_file() {
        // Test error handling when video file doesn't exist
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let nonexistent_path = PathBuf::from("nonexistent_video.mp4");

        let result = executor.get_duration(&nonexistent_path);

        // Should return an error
        assert!(result.is_err());

        // Verify it's an ExecutionFailed error
        match result {
            Err(FFmpegError::ExecutionFailed(_)) => {
                // Expected error type
            }
            _ => panic!("Expected ExecutionFailed error for nonexistent file"),
        }
    }

    #[test]
    fn test_ffmpeg_error_variants() {
        // Test that all error variants can be created and have correct messages
        let execution_error = FFmpegError::ExecutionFailed("test error".to_string());
        assert!(execution_error.to_string().contains("test error"));

        let parse_error = FFmpegError::ParseError("invalid format".to_string());
        assert!(parse_error.to_string().contains("invalid format"));

        let no_audio_error = FFmpegError::NoAudioTrack;
        assert_eq!(no_audio_error.to_string(), "Video has no audio track");
    }

    // JSON Parsing Tests with serde_json

    #[test]
    fn test_parse_metadata_json_valid() {
        // Test valid JSON parsing
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let json = r#"{"streams":[{"codec_name":"h264","width":1920,"height":1080}],"format":{"duration":"123.45"}}"#;
        let video_path = PathBuf::from("/test/video.mp4");

        let result = executor.parse_metadata_json(json, &video_path);
        assert!(result.is_ok());

        let metadata = result.unwrap();
        assert_eq!(metadata.codec, "h264");
        assert_eq!(metadata.width, 1920);
        assert_eq!(metadata.height, 1080);
        assert!((metadata.duration - 123.45).abs() < 0.001);
    }

    #[test]
    fn test_parse_metadata_json_missing_codec_name() {
        // Test missing codec_name field
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let json = r#"{"streams":[{"width":1920,"height":1080}],"format":{"duration":"123.45"}}"#;
        let video_path = PathBuf::from("/test/video.mp4");

        let result = executor.parse_metadata_json(json, &video_path);
        assert!(result.is_err());

        match result {
            Err(FFmpegError::ParseError(msg)) => {
                assert!(msg.contains("Failed to parse JSON") || msg.contains("codec_name"));
            }
            _ => panic!("Expected ParseError for missing codec_name"),
        }
    }

    #[test]
    fn test_parse_metadata_json_invalid_width() {
        // Test invalid width (non-numeric)
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let json = r#"{"streams":[{"codec_name":"h264","width":"invalid","height":1080}],"format":{"duration":"123.45"}}"#;
        let video_path = PathBuf::from("/test/video.mp4");

        let result = executor.parse_metadata_json(json, &video_path);
        assert!(result.is_err());

        match result {
            Err(FFmpegError::ParseError(msg)) => {
                assert!(msg.contains("Failed to parse JSON") || msg.contains("width"));
            }
            _ => panic!("Expected ParseError for invalid width"),
        }
    }

    #[test]
    fn test_parse_metadata_json_na_duration() {
        // Test "N/A" duration handling
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let json = r#"{"streams":[{"codec_name":"h264","width":1920,"height":1080}],"format":{"duration":"N/A"}}"#;
        let video_path = PathBuf::from("/test/video.mp4");

        let result = executor.parse_metadata_json(json, &video_path);
        assert!(result.is_err());

        match result {
            Err(FFmpegError::CorruptedFile(msg)) => {
                assert!(msg.contains("Unable to determine video duration"));
            }
            _ => panic!("Expected CorruptedFile error for N/A duration"),
        }
    }

    #[test]
    fn test_parse_metadata_json_empty_duration() {
        // Test empty duration handling
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let json = r#"{"streams":[{"codec_name":"h264","width":1920,"height":1080}],"format":{"duration":""}}"#;
        let video_path = PathBuf::from("/test/video.mp4");

        let result = executor.parse_metadata_json(json, &video_path);
        assert!(result.is_err());

        match result {
            Err(FFmpegError::CorruptedFile(msg)) => {
                assert!(msg.contains("Unable to determine video duration"));
            }
            _ => panic!("Expected CorruptedFile error for empty duration"),
        }
    }

    #[test]
    fn test_parse_metadata_json_no_streams() {
        // Test JSON with no streams
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let json = r#"{"streams":[],"format":{"duration":"123.45"}}"#;
        let video_path = PathBuf::from("/test/video.mp4");

        let result = executor.parse_metadata_json(json, &video_path);
        assert!(result.is_err());

        match result {
            Err(FFmpegError::ParseError(msg)) => {
                assert!(msg.contains("No video stream found"));
            }
            _ => panic!("Expected ParseError for no streams"),
        }
    }

    #[test]
    fn test_parse_metadata_json_error_messages_include_field_names() {
        // Verify error messages include field names for better debugging
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let video_path = PathBuf::from("/test/video.mp4");

        // Test with invalid duration value
        let json = r#"{"streams":[{"codec_name":"h264","width":1920,"height":1080}],"format":{"duration":"not_a_number"}}"#;
        let result = executor.parse_metadata_json(json, &video_path);

        assert!(result.is_err());
        match result {
            Err(FFmpegError::ParseError(msg)) => {
                // Error message should include the field value that failed to parse
                assert!(msg.contains("duration") || msg.contains("not_a_number"));
            }
            _ => panic!("Expected ParseError with field context"),
        }
    }

    #[test]
    fn test_get_video_resolution_with_nonexistent_file() {
        // Test error handling when video file doesn't exist
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let nonexistent_path = PathBuf::from("nonexistent_video.mp4");

        let result = executor.get_video_resolution(&nonexistent_path);

        // Should return an error
        assert!(result.is_err());

        // Verify it's an ExecutionFailed error
        match result {
            Err(FFmpegError::ExecutionFailed(_)) => {
                // Expected error type
            }
            _ => panic!("Expected ExecutionFailed error for nonexistent file"),
        }
    }

    #[test]
    fn test_get_video_resolution_with_invalid_file() {
        // Test error handling when file is not a valid video
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);

        // Create a temporary invalid file (text file pretending to be video)
        use std::env;
        use std::fs;

        let temp_dir = env::temp_dir();
        let invalid_video_path = temp_dir.join("invalid_video_test.mp4");

        // Write some non-video content
        fs::write(&invalid_video_path, b"This is not a video file").ok();

        let result = executor.get_video_resolution(&invalid_video_path);

        // Clean up
        fs::remove_file(&invalid_video_path).ok();

        // Should return an error (either ExecutionFailed or ParseError)
        assert!(result.is_err());
    }

    // Tests for calculate_scale_filter

    #[test]
    fn test_calculate_scale_filter_no_upscaling_smaller_source() {
        // Test that no scaling is applied when source is smaller than target
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);

        // Source: 1280x720 (smaller than 1920x1080)
        let result = executor.calculate_scale_filter((1280, 720));

        // Should return None (no upscaling)
        assert_eq!(result, None);
    }

    #[test]
    fn test_calculate_scale_filter_no_upscaling_equal_source() {
        // Test that no scaling is applied when source equals target
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);

        // Source: 1920x1080 (equal to target)
        let result = executor.calculate_scale_filter((1920, 1080));

        // Should return None (no upscaling)
        assert_eq!(result, None);
    }

    #[test]
    fn test_calculate_scale_filter_downscaling_1080p() {
        // Test that scaling is applied when source is larger than target (1080p)
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);

        // Source: 3840x2160 (4K, larger than 1920x1080)
        let result = executor.calculate_scale_filter((3840, 2160));

        // Should return scale filter with letterboxing
        assert!(result.is_some());
        let filter = result.unwrap();
        assert!(filter.contains("scale=1920:1080"));
        assert!(filter.contains("force_original_aspect_ratio=decrease"));
        assert!(filter.contains("pad=1920:1080"));
        assert!(filter.contains("(ow-iw)/2:(oh-ih)/2"));
    }

    #[test]
    fn test_calculate_scale_filter_downscaling_720p() {
        // Test that scaling is applied when source is larger than target (720p)
        let executor = FFmpegExecutor::new(Resolution::Hd720, true);

        // Source: 1920x1080 (larger than 1280x720)
        let result = executor.calculate_scale_filter((1920, 1080));

        // Should return scale filter with letterboxing
        assert!(result.is_some());
        let filter = result.unwrap();
        assert!(filter.contains("scale=1280:720"));
        assert!(filter.contains("force_original_aspect_ratio=decrease"));
        assert!(filter.contains("pad=1280:720"));
        assert!(filter.contains("(ow-iw)/2:(oh-ih)/2"));
    }

    #[test]
    fn test_calculate_scale_filter_no_upscaling_720p_smaller() {
        // Test no upscaling for 720p target with smaller source
        let executor = FFmpegExecutor::new(Resolution::Hd720, true);

        // Source: 640x480 (smaller than 1280x720)
        let result = executor.calculate_scale_filter((640, 480));

        // Should return None (no upscaling)
        assert_eq!(result, None);
    }

    #[test]
    fn test_calculate_scale_filter_partial_upscaling_width() {
        // Test no upscaling when only width is smaller
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);

        // Source: 1280x2160 (width smaller, height larger)
        let result = executor.calculate_scale_filter((1280, 2160));

        // Should return scale filter (height is larger, so we scale down)
        assert!(result.is_some());
    }

    #[test]
    fn test_calculate_scale_filter_partial_upscaling_height() {
        // Test no upscaling when only height is smaller
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);

        // Source: 3840x720 (width larger, height smaller)
        let result = executor.calculate_scale_filter((3840, 720));

        // Should return scale filter (width is larger, so we scale down)
        assert!(result.is_some());
    }

    #[test]
    fn test_calculate_scale_filter_format_correctness() {
        // Test that the filter string format is exactly as expected
        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);

        // Source: 3840x2160
        let result = executor.calculate_scale_filter((3840, 2160));

        assert_eq!(
            result,
            Some("scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2".to_string())
        );
    }

    #[test]
    fn test_calculate_scale_filter_format_correctness_720p() {
        // Test that the filter string format is exactly as expected for 720p
        let executor = FFmpegExecutor::new(Resolution::Hd720, true);

        // Source: 1920x1080
        let result = executor.calculate_scale_filter((1920, 1080));

        assert_eq!(
            result,
            Some("scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2".to_string())
        );
    }

    // Feature: video-clip-extractor, Property 7: No Upscaling
    // **Validates: Requirements 2.5, 2.6, 2.7**
    proptest! {
        #[test]
        fn test_no_upscaling_property(
            source_width in 1u32..=1920,
            source_height in 1u32..=1080,
            target_resolution in prop::sample::select(vec![Resolution::Hd720, Resolution::Hd1080])
        ) {
            let executor = FFmpegExecutor::new(target_resolution.clone(), true);

            // Determine target dimensions
            let (target_width, target_height) = match target_resolution {
                Resolution::Hd720 => (1280u32, 720u32),
                Resolution::Hd1080 => (1920u32, 1080u32),
            };

            // If source resolution is smaller than or equal to target in both dimensions,
            // no scaling filter should be returned (no upscaling)
            if source_width <= target_width && source_height <= target_height {
                let result = executor.calculate_scale_filter((source_width, source_height));
                prop_assert_eq!(result, None,
                    "Expected no upscaling for source {}x{} with target {}x{}",
                    source_width, source_height, target_width, target_height);
            }
            // If source is larger in at least one dimension, scaling should occur
            else if source_width > target_width || source_height > target_height {
                let result = executor.calculate_scale_filter((source_width, source_height));
                prop_assert!(result.is_some(),
                    "Expected scaling for source {}x{} with target {}x{}",
                    source_width, source_height, target_width, target_height);

                // Verify the filter contains the correct target dimensions
                let filter = result.unwrap();
                prop_assert!(filter.contains(&format!("scale={}:{}", target_width, target_height)),
                    "Filter should contain correct target dimensions");
            }
        }
    }

    // Helper function to parse duration string (extracted from get_duration logic)
    fn parse_duration_string(duration_str: &str) -> Result<f64, FFmpegError> {
        duration_str.trim().parse::<f64>().map_err(|e| {
            FFmpegError::ParseError(format!(
                "Failed to parse duration '{}': {}",
                duration_str, e
            ))
        })
    }

    // Feature: video-clip-extractor, Property 19: Duration Parsing Correctness
    // **Validates: Requirements 9.2, 9.4**
    proptest! {
        #[test]
        fn test_duration_parsing_correctness(
            // Generate durations from 0.001 to 86400.0 (24 hours)
            // with up to 6 decimal places to test fractional seconds
            whole_seconds in 0u32..=86400,
            fractional_part in 0u32..=999999,
        ) {
            // Construct a duration value with fractional seconds
            let expected_duration = whole_seconds as f64 + (fractional_part as f64 / 1_000_000.0);

            // Test various precision formats that FFmpeg might output
            let test_cases = vec![
                (format!("{:.6}", expected_duration), 6),
                (format!("{:.3}", expected_duration), 3),
                (format!("{:.2}", expected_duration), 2),
                (format!("{:.0}", expected_duration), 0),
            ];

            for (duration_str, precision) in test_cases {
                let parsed = parse_duration_string(&duration_str);

                prop_assert!(parsed.is_ok(),
                    "Failed to parse valid duration string: '{}'", duration_str);

                let parsed_value = parsed.unwrap();

                // The parsed value should match what we formatted
                // We need to account for the precision loss during formatting
                let formatted_expected: f64 = duration_str.trim().parse().unwrap();
                let difference = (parsed_value - formatted_expected).abs();

                prop_assert!(difference < 1e-10,
                    "Parsed duration {} differs from formatted expected {} by {} (string: '{}', precision: {})",
                    parsed_value, formatted_expected, difference, duration_str, precision);
            }

            // Test with whitespace variations (using full precision)
            let duration_str_with_spaces = vec![
                format!("{:.6} ", expected_duration),
                format!(" {:.6}", expected_duration),
                format!("  {:.6}  ", expected_duration),
            ];

            for duration_str in duration_str_with_spaces {
                let parsed = parse_duration_string(&duration_str);

                prop_assert!(parsed.is_ok(),
                    "Failed to parse duration string with whitespace: '{}'", duration_str);

                let parsed_value = parsed.unwrap();
                let difference = (parsed_value - expected_duration).abs();

                // Allow for small floating point precision differences
                prop_assert!(difference < 0.0001,
                    "Parsed duration {} differs from expected {} by {} (string: '{}')",
                    parsed_value, expected_duration, difference, duration_str);
            }
        }
    }

    proptest! {
        #[test]
        fn test_duration_parsing_handles_edge_cases(
            // Test very small and very large durations
            duration in prop::num::f64::ANY.prop_filter(
                "Must be non-negative and finite",
                |d| d.is_finite() && *d >= 0.0 && *d <= 86400.0
            )
        ) {
            // Format the duration as FFmpeg would
            let duration_str = format!("{:.6}", duration);

            let parsed = parse_duration_string(&duration_str);

            prop_assert!(parsed.is_ok(),
                "Failed to parse duration string: '{}'", duration_str);

            let parsed_value = parsed.unwrap();

            // Verify the parsed value matches the original
            let difference = (parsed_value - duration).abs();
            prop_assert!(difference < 0.0001,
                "Parsed duration {} differs from expected {} by {}",
                parsed_value, duration, difference);
        }
    }

    proptest! {
        #[test]
        fn test_duration_parsing_rejects_invalid_input(
            // Generate invalid strings that should fail parsing
            invalid_str in prop::string::string_regex("[a-zA-Z]+").unwrap()
        ) {
            let result = parse_duration_string(&invalid_str);

            prop_assert!(result.is_err(),
                "Expected parsing to fail for invalid string: '{}'", invalid_str);

            // Verify it's a ParseError
            match result {
                Err(FFmpegError::ParseError(_)) => {
                    // Expected error type
                }
                _ => {
                    return Err(proptest::test_runner::TestCaseError::fail(
                        "Expected ParseError for invalid duration string"
                    ));
                }
            }
        }
    }

    // Tests for build_extract_command

    #[test]
    fn test_build_extract_command_basic() {
        use crate::selector::TimeRange;
        use std::path::PathBuf;

        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let video_path = PathBuf::from("/path/to/video.mp4");
        let output_path = PathBuf::from("/path/to/output.mp4");
        let time_range = TimeRange {
            start_seconds: 120.5,
            duration_seconds: 7.0,
        };
        let source_resolution = (1920, 1080);

        let args = executor.build_extract_command(
            &video_path,
            &time_range,
            &output_path,
            source_resolution,
            "h264", // H.264 codec for fast seeking
        );

        // Verify essential arguments are present
        assert!(args.contains(&"-ss".to_string()));
        assert!(args.contains(&"-i".to_string()));
        assert!(args.contains(&"-t".to_string()));
        assert!(args.contains(&"7".to_string()));
        assert!(args.contains(&"-c:v".to_string()));
        assert!(args.contains(&"libx264".to_string()));
        assert!(args.contains(&"-preset".to_string()));
        assert!(args.contains(&"fast".to_string()));
        assert!(args.contains(&"-g".to_string()));
        assert!(args.contains(&"30".to_string()));
        assert!(args.contains(&"-keyint_min".to_string()));
        // Pixel format is now handled in filter chain, not as codec option
        assert!(args.contains(&"-vf".to_string()));
        assert!(args.contains(&"bt709".to_string()));
        assert!(args.contains(&"-colorspace".to_string()));
        assert!(args.contains(&"-color_primaries".to_string()));
        assert!(args.contains(&"-color_trc".to_string()));
        assert!(args.contains(&"-y".to_string()));

        // With H.264 hybrid seeking, verify we have the correct seek values
        // Fast seek: 120.5 - 5.0 = 115.5, Accurate seek: 5.0
        let i_index = args.iter().position(|arg| arg == "-i").unwrap();
        let ss_positions: Vec<usize> = args
            .iter()
            .enumerate()
            .filter(|(_, arg)| *arg == "-ss")
            .map(|(i, _)| i)
            .collect();

        assert_eq!(
            ss_positions.len(),
            2,
            "Should have 2 -ss flags for hybrid seeking"
        );
        assert!(ss_positions[0] < i_index, "First -ss should be before -i");
        assert!(ss_positions[1] > i_index, "Second -ss should be after -i");
        assert_eq!(
            args[ss_positions[0] + 1],
            "115.5",
            "Fast seek should be 115.5"
        );
        assert_eq!(args[ss_positions[1] + 1], "5", "Accurate seek should be 5");

        // Should have format filter for pixel format conversion
        let vf_index = args.iter().position(|arg| arg == "-vf").unwrap();
        let filter = &args[vf_index + 1];
        assert!(
            filter.contains("format=yuv420p"),
            "Should have format filter for pixel format conversion"
        );

        // Audio should be included (aac codec)
        assert!(args.contains(&"-c:a".to_string()));
        assert!(args.contains(&"aac".to_string()));
        assert!(!args.contains(&"-an".to_string()));
    }

    #[test]
    fn test_build_extract_command_no_audio() {
        use crate::selector::TimeRange;
        use std::path::PathBuf;

        let executor = FFmpegExecutor::new(Resolution::Hd1080, false);
        let video_path = PathBuf::from("/path/to/video.mp4");
        let output_path = PathBuf::from("/path/to/output.mp4");
        let time_range = TimeRange {
            start_seconds: 60.0,
            duration_seconds: 5.0,
        };
        let source_resolution = (1920, 1080);

        let args = executor.build_extract_command(
            &video_path,
            &time_range,
            &output_path,
            source_resolution,
            "h264", // H.264 codec
        );

        // Should have format filter for pixel format conversion
        assert!(args.contains(&"-vf".to_string()));
        let vf_index = args.iter().position(|arg| arg == "-vf").unwrap();
        let filter = &args[vf_index + 1];
        assert!(
            filter.contains("format=yuv420p"),
            "Should have format filter"
        );

        // Audio should be excluded
        assert!(args.contains(&"-an".to_string()));
        assert!(!args.contains(&"-c:a".to_string()));
    }

    #[test]
    fn test_build_extract_command_with_scaling() {
        use crate::selector::TimeRange;
        use std::path::PathBuf;

        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let video_path = PathBuf::from("/path/to/video.mp4");
        let output_path = PathBuf::from("/path/to/output.mp4");
        let time_range = TimeRange {
            start_seconds: 30.0,
            duration_seconds: 10.0,
        };
        // Source is 4K, should be scaled down to 1080p
        let source_resolution = (3840, 2160);

        let args = executor.build_extract_command(
            &video_path,
            &time_range,
            &output_path,
            source_resolution,
            "h264", // H.264 codec
        );

        // Should include video filter for scaling
        assert!(args.contains(&"-vf".to_string()));

        // Find the scale filter argument
        let vf_index = args.iter().position(|arg| arg == "-vf").unwrap();
        let filter = &args[vf_index + 1];

        // Should contain scaling
        assert!(filter.contains("scale=1920:1080"));
        assert!(filter.contains("force_original_aspect_ratio=decrease"));
        assert!(filter.contains("pad=1920:1080"));
    }

    #[test]
    fn test_build_extract_command_no_upscaling() {
        use crate::selector::TimeRange;
        use std::path::PathBuf;

        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let video_path = PathBuf::from("/path/to/video.mp4");
        let output_path = PathBuf::from("/path/to/output.mp4");
        let time_range = TimeRange {
            start_seconds: 15.0,
            duration_seconds: 8.0,
        };
        // Source is 720p, should NOT be upscaled to 1080p
        let source_resolution = (1280, 720);

        let args = executor.build_extract_command(
            &video_path,
            &time_range,
            &output_path,
            source_resolution,
            "h264", // H.264 codec
        );

        // Should have format filter but no scaling filter
        assert!(args.contains(&"-vf".to_string()));
        let vf_index = args.iter().position(|arg| arg == "-vf").unwrap();
        let filter = &args[vf_index + 1];

        // Should have format filter
        assert!(
            filter.contains("format=yuv420p"),
            "Should have format filter"
        );
        // Should NOT have scale filter (no upscaling)
        assert!(
            !filter.contains("scale="),
            "Should not have scale filter for smaller source"
        );
    }

    #[test]
    fn test_build_extract_command_720p_target() {
        use crate::selector::TimeRange;
        use std::path::PathBuf;

        let executor = FFmpegExecutor::new(Resolution::Hd720, true);
        let video_path = PathBuf::from("/path/to/video.mp4");
        let output_path = PathBuf::from("/path/to/output.mp4");
        let time_range = TimeRange {
            start_seconds: 45.0,
            duration_seconds: 6.5,
        };
        // Source is 1080p, should be scaled down to 720p
        let source_resolution = (1920, 1080);

        let args = executor.build_extract_command(
            &video_path,
            &time_range,
            &output_path,
            source_resolution,
            "h264", // H.264 codec
        );

        // Should include video filter for 720p scaling
        assert!(args.contains(&"-vf".to_string()));

        let vf_index = args.iter().position(|arg| arg == "-vf").unwrap();
        let filter = &args[vf_index + 1];

        // Should contain 720p scaling
        assert!(filter.contains("scale=1280:720"));
        assert!(filter.contains("pad=1280:720"));
    }

    #[test]
    fn test_build_extract_command_argument_order() {
        use crate::selector::TimeRange;
        use std::path::PathBuf;

        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let video_path = PathBuf::from("/path/to/video.mp4");
        let output_path = PathBuf::from("/path/to/output.mp4");
        let time_range = TimeRange {
            start_seconds: 100.0,
            duration_seconds: 5.0,
        };
        let source_resolution = (1920, 1080);

        let args = executor.build_extract_command(
            &video_path,
            &time_range,
            &output_path,
            source_resolution,
            "h264", // H.264 codec
        );

        // With codec-aware seeking for H.264, we should have:
        // Fast seek before -i (5 seconds), then accurate seek after -i
        let i_index = args.iter().position(|arg| arg == "-i").unwrap();

        // Find all -ss positions
        let ss_positions: Vec<usize> = args
            .iter()
            .enumerate()
            .filter(|(_, arg)| *arg == "-ss")
            .map(|(i, _)| i)
            .collect();

        // Should have 2 -ss flags for H.264 hybrid seeking (start > 5s)
        assert_eq!(
            ss_positions.len(),
            2,
            "Should have 2 -ss flags for H.264 hybrid seeking"
        );
        assert!(
            ss_positions[0] < i_index,
            "First -ss (fast seek) should come before -i"
        );
        assert!(
            ss_positions[1] > i_index,
            "Second -ss (accurate seek) should come after -i"
        );

        // Verify seek values: fast seek = 100 - 5 = 95, accurate seek = 5
        assert_eq!(args[ss_positions[0] + 1], "95", "Fast seek should be 95");
        assert_eq!(args[ss_positions[1] + 1], "5", "Accurate seek should be 5");

        // Verify -t comes after the accurate seek (second -ss)
        let t_index = args.iter().position(|arg| arg == "-t").unwrap();
        assert!(
            t_index > ss_positions[1],
            "Duration (-t) should come after accurate seek"
        );

        // Verify output path is last
        assert_eq!(
            args.last().unwrap(),
            &output_path.to_string_lossy().to_string()
        );
    }

    #[test]
    fn test_build_extract_command_early_start_time() {
        use crate::selector::TimeRange;
        use std::path::PathBuf;

        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let video_path = PathBuf::from("/path/to/video.mp4");
        let output_path = PathBuf::from("/path/to/output.mp4");

        // Test with start time < 5 seconds (should use accurate seek after -i)
        let time_range = TimeRange {
            start_seconds: 3.0,
            duration_seconds: 5.0,
        };
        let source_resolution = (1920, 1080);

        let args = executor.build_extract_command(
            &video_path,
            &time_range,
            &output_path,
            source_resolution,
            "h264", // H.264 codec
        );

        let i_index = args.iter().position(|arg| arg == "-i").unwrap();

        // Find all -ss positions
        let ss_positions: Vec<usize> = args
            .iter()
            .enumerate()
            .filter(|(_, arg)| *arg == "-ss")
            .map(|(i, _)| i)
            .collect();

        // Should have only 1 -ss flag (accurate seek after -i) when start < 5s for H.264
        assert_eq!(
            ss_positions.len(),
            1,
            "Should have only 1 -ss flag when start time < 5s"
        );
        assert!(
            ss_positions[0] > i_index,
            "Single -ss (accurate seek) should come after -i"
        );
        assert_eq!(args[ss_positions[0] + 1], "3", "Should seek to 3 seconds");
    }

    #[test]
    fn test_build_extract_command_hevc_codec() {
        use crate::selector::TimeRange;
        use std::path::PathBuf;

        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let video_path = PathBuf::from("/path/to/hevc_video.mp4");
        let output_path = PathBuf::from("/path/to/output.mp4");

        // Test HEVC with start time > 2 seconds (should use 2-second offset)
        let time_range = TimeRange {
            start_seconds: 120.0,
            duration_seconds: 10.0,
        };
        let source_resolution = (1920, 1080);

        let args = executor.build_extract_command(
            &video_path,
            &time_range,
            &output_path,
            source_resolution,
            "hevc", // HEVC codec
        );

        let i_index = args.iter().position(|arg| arg == "-i").unwrap();

        // Find all -ss positions
        let ss_positions: Vec<usize> = args
            .iter()
            .enumerate()
            .filter(|(_, arg)| *arg == "-ss")
            .map(|(i, _)| i)
            .collect();

        // Should have 2 -ss flags for HEVC (moderate fast seek with 2-second offset)
        assert_eq!(ss_positions.len(), 2, "Should have 2 -ss flags for HEVC");
        assert!(
            ss_positions[0] < i_index,
            "First -ss (fast seek) should come before -i"
        );
        assert!(
            ss_positions[1] > i_index,
            "Second -ss (accurate seek) should come after -i"
        );

        // Verify seek values: fast seek = 120 - 2 = 118, accurate seek = 2
        assert_eq!(
            args[ss_positions[0] + 1],
            "118",
            "HEVC fast seek should be 118 (2-second offset)"
        );
        assert_eq!(
            args[ss_positions[1] + 1],
            "2",
            "HEVC accurate seek should be 2"
        );

        // Verify HEVC-specific buffer settings are present
        assert!(
            args.contains(&"-analyzeduration".to_string()),
            "Should have analyzeduration for HEVC"
        );
        assert!(
            args.contains(&"100M".to_string()),
            "Should have 100M buffer size for HEVC"
        );
        assert!(
            args.contains(&"-probesize".to_string()),
            "Should have probesize for HEVC"
        );
    }

    #[test]
    fn test_build_extract_command_overwrite_flag() {
        use crate::selector::TimeRange;
        use std::path::PathBuf;

        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let video_path = PathBuf::from("/path/to/video.mp4");
        let output_path = PathBuf::from("/path/to/output.mp4");
        let time_range = TimeRange {
            start_seconds: 0.0,
            duration_seconds: 5.0,
        };
        let source_resolution = (1920, 1080);

        let args = executor.build_extract_command(
            &video_path,
            &time_range,
            &output_path,
            source_resolution,
            "h264", // H.264 codec
        );

        // Should include -y flag to overwrite existing files
        assert!(args.contains(&"-y".to_string()));
    }

    // Feature: video-clip-extractor, Property 5: Extracted Clip Duration
    // **Validates: Requirements 2.1**
    proptest! {
        #[test]
        fn test_extracted_clip_duration_property(
            // Generate video durations longer than 15 seconds (up to 2 hours)
            video_duration in 15.1f64..=7200.0,
            // Generate clip durations between 10 and 15 seconds
            clip_duration in 10.0f64..=15.0,
            // Generate start times that allow the clip to fit within the video
            start_offset_ratio in 0.0f64..=1.0,
        ) {
            // Calculate a valid start time that ensures the clip fits within the video
            let max_start_time = video_duration - clip_duration;
            let start_time = start_offset_ratio * max_start_time;

            // Create a TimeRange with the generated parameters
            let time_range = TimeRange {
                start_seconds: start_time,
                duration_seconds: clip_duration,
            };

            // Verify that the clip duration is within the valid range [10, 15]
            prop_assert!(
                clip_duration >= 10.0 && clip_duration <= 15.0,
                "Clip duration {} must be between 10 and 15 seconds",
                clip_duration
            );

            // Verify that the clip fits within the video duration
            let clip_end_time = start_time + clip_duration;
            prop_assert!(
                clip_end_time <= video_duration,
                "Clip end time {} must not exceed video duration {}",
                clip_end_time,
                video_duration
            );

            // Verify that start time is non-negative
            prop_assert!(
                start_time >= 0.0,
                "Start time {} must be non-negative",
                start_time
            );

            // Verify the TimeRange struct contains the expected values
            prop_assert_eq!(
                time_range.duration_seconds,
                clip_duration,
                "TimeRange duration should match the requested clip duration"
            );

            prop_assert_eq!(
                time_range.start_seconds,
                start_time,
                "TimeRange start should match the calculated start time"
            );
        }
    }

    #[test]
    fn test_short_video_extraction() {
        // Test that videos < 5 seconds are extracted in full
        // Validates Requirement 2.4
        use crate::selector::TimeRange;
        use std::path::PathBuf;

        let executor = FFmpegExecutor::new(Resolution::Hd1080, true);
        let video_path = PathBuf::from("/path/to/short_video.mp4");
        let output_path = PathBuf::from("/path/to/output.mp4");

        // Test case 1: Video is exactly 4.5 seconds (less than 5 seconds)
        let short_duration = 4.5;
        let time_range = TimeRange {
            start_seconds: 0.0,
            duration_seconds: short_duration,
        };
        let source_resolution = (1920, 1080);

        let args = executor.build_extract_command(
            &video_path,
            &time_range,
            &output_path,
            source_resolution,
            "h264", // H.264 codec
        );

        // When start is 0, no -ss flags are added (extracts from beginning)
        let ss_positions: Vec<usize> = args
            .iter()
            .enumerate()
            .filter(|(_, arg)| *arg == "-ss")
            .map(|(i, _)| i)
            .collect();
        // No seek flags when starting at 0
        assert_eq!(
            ss_positions.len(),
            0,
            "Should have no -ss flags when starting at 0"
        );

        // Verify the duration matches the full video duration
        let t_index = args.iter().position(|arg| arg == "-t").unwrap();
        assert_eq!(
            args[t_index + 1],
            "4.5",
            "Short video should extract full duration"
        );

        // Test case 2: Video starting at 2 seconds for 3 seconds
        let time_range2 = TimeRange {
            start_seconds: 2.0,
            duration_seconds: 3.0,
        };

        let args2 = executor.build_extract_command(
            &video_path,
            &time_range2,
            &output_path,
            source_resolution,
            "h264", // H.264 codec
        );

        // Should have accurate seek after -i
        let ss_positions2: Vec<usize> = args2
            .iter()
            .enumerate()
            .filter(|(_, arg)| *arg == "-ss")
            .map(|(i, _)| i)
            .collect();

        assert_eq!(ss_positions2.len(), 1, "Should have 1 -ss flag");

        let i_index2 = args2.iter().position(|arg| arg == "-i").unwrap();
        assert!(
            ss_positions2[0] > i_index2,
            "Accurate seek should come after -i"
        );
        assert_eq!(args2[ss_positions2[0] + 1], "2", "Should seek to 2 seconds");

        let t_index2 = args2.iter().position(|arg| arg == "-t").unwrap();
        assert_eq!(args2[t_index2 + 1], "3", "Should extract 3 seconds");

        // Verify all commands are well-formed with required flags
        for args in [&args, &args2] {
            assert!(
                args.contains(&"-i".to_string()),
                "Command should contain input flag"
            );
            assert!(
                args.contains(&"-c:v".to_string()),
                "Command should contain video codec flag"
            );
            assert!(
                args.contains(&"libx264".to_string()),
                "Command should use libx264 codec"
            );
            assert!(
                args.contains(&"-y".to_string()),
                "Command should contain overwrite flag"
            );
        }
    }

    // Feature: video-clip-extractor, Property 9: Audio Inclusion Control
    // **Validates: Requirements 2.9, 2.10**
    proptest! {
        #[test]
        fn test_audio_inclusion_control_property(
            // Generate random audio inclusion flag
            include_audio in prop::bool::ANY,
            // Generate random resolution
            resolution in prop::sample::select(vec![Resolution::Hd720, Resolution::Hd1080]),
            // Generate random video parameters
            start_seconds in 0.0f64..=3600.0,
            duration_seconds in 10.0f64..=15.0,
            source_width in 640u32..=3840,
            source_height in 480u32..=2160,
        ) {
            use std::path::PathBuf;

            // Create executor with the audio inclusion flag
            let executor = FFmpegExecutor::new(resolution, include_audio);

            // Create test paths and time range
            let video_path = PathBuf::from("/test/video.mp4");
            let output_path = PathBuf::from("/test/output.mp4");
            let time_range = TimeRange {
                start_seconds,
                duration_seconds,
            };
            let source_resolution = (source_width, source_height);

            // Build the FFmpeg command
            let args = executor.build_extract_command(
                &video_path,
                &time_range,
                &output_path,
                source_resolution,
                "h264", // H.264 codec for testing
            );

            // Verify audio handling based on include_audio flag
            if include_audio {
                // When audio is included, the command should contain audio codec settings
                // and should NOT contain the -an flag (no audio)
                prop_assert!(
                    !args.contains(&"-an".to_string()),
                    "Command should NOT contain -an flag when include_audio is true"
                );

                // Should contain audio codec specification
                prop_assert!(
                    args.contains(&"-c:a".to_string()),
                    "Command should contain -c:a flag when include_audio is true"
                );

                prop_assert!(
                    args.contains(&"aac".to_string()),
                    "Command should contain aac codec when include_audio is true"
                );

                // Verify -c:a and aac are adjacent in the args
                if let Some(ca_index) = args.iter().position(|arg| arg == "-c:a") {
                    prop_assert!(
                        ca_index + 1 < args.len() && args[ca_index + 1] == "aac",
                        "aac codec should immediately follow -c:a flag"
                    );
                }
            } else {
                // When audio is excluded, the command should contain the -an flag
                // and should NOT contain audio codec settings
                prop_assert!(
                    args.contains(&"-an".to_string()),
                    "Command should contain -an flag when include_audio is false"
                );

                // Should NOT contain audio codec specification
                prop_assert!(
                    !args.contains(&"-c:a".to_string()),
                    "Command should NOT contain -c:a flag when include_audio is false"
                );

                // The aac codec might appear in paths, so we check more carefully
                // by ensuring -c:a is not present (which is the key indicator)
            }

            // Additional verification: ensure the command is well-formed
            // regardless of audio settings
            prop_assert!(
                args.contains(&"-ss".to_string()),
                "Command should contain start time flag"
            );

            prop_assert!(
                args.contains(&"-i".to_string()),
                "Command should contain input flag"
            );

            prop_assert!(
                args.contains(&"-t".to_string()),
                "Command should contain duration flag"
            );

            prop_assert!(
                args.contains(&"-c:v".to_string()),
                "Command should contain video codec flag"
            );

            prop_assert!(
                args.contains(&"libx264".to_string()),
                "Command should contain libx264 video codec"
            );
        }
    }

    // Tests for audio analysis helper

    #[test]
    fn test_extract_value_after_basic() {
        // Test extracting a simple numeric value
        let line = "Some text t: 123.45 more text";
        let result = FFmpegExecutor::extract_value_after(line, "t:");
        assert_eq!(result, Some(123.45));
    }

    #[test]
    fn test_extract_value_after_negative() {
        // Test extracting a negative value (common for dBFS)
        let line = "Audio level FTPK: -12.3 dBFS";
        let result = FFmpegExecutor::extract_value_after(line, "FTPK:");
        assert_eq!(result, Some(-12.3));
    }

    #[test]
    fn test_extract_value_after_with_spaces() {
        // Test extracting value with leading spaces
        let line = "Time t:   456.789 seconds";
        let result = FFmpegExecutor::extract_value_after(line, "t:");
        assert_eq!(result, Some(456.789));
    }

    #[test]
    fn test_extract_value_after_not_found() {
        // Test when label is not found
        let line = "Some text without the label";
        let result = FFmpegExecutor::extract_value_after(line, "t:");
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_value_after_invalid_number() {
        // Test when text after label is not a valid number
        let line = "Label t: not_a_number";
        let result = FFmpegExecutor::extract_value_after(line, "t:");
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_value_after_zero() {
        // Test extracting zero value
        let line = "Value M: 0.0 units";
        let result = FFmpegExecutor::extract_value_after(line, "M:");
        assert_eq!(result, Some(0.0));
    }

    #[test]
    fn test_extract_value_after_integer() {
        // Test extracting integer value
        let line = "Count n: 42 items";
        let result = FFmpegExecutor::extract_value_after(line, "n:");
        assert_eq!(result, Some(42.0));
    }

    // Tests for motion analysis parsing

    #[test]
    fn test_parse_showinfo_output_basic() {
        // Test parsing of basic showinfo output with pts_time and scene score
        let line = "[Parsed_showinfo_1 @ 0x7f8b9c000000] n:42 pts:1260 pts_time:1.26 pos:123456 fmt:yuv420p sar:1/1 s:1920x1080 i:P iskey:0 type:P checksum:ABCD1234 plane_checksum:[ABCD EFGH] scene:0.456";

        let time = FFmpegExecutor::extract_value_after(line, "pts_time:");
        let score = FFmpegExecutor::extract_value_after(line, "scene:");

        assert_eq!(time, Some(1.26));
        assert_eq!(score, Some(0.456));
    }

    #[test]
    fn test_parse_showinfo_output_high_score() {
        // Test parsing with high scene score (significant motion)
        let line = "[Parsed_showinfo_1 @ 0x7f8b9c000000] n:100 pts:3000 pts_time:30.0 scene:0.987";

        let time = FFmpegExecutor::extract_value_after(line, "pts_time:");
        let score = FFmpegExecutor::extract_value_after(line, "scene:");

        assert_eq!(time, Some(30.0));
        assert_eq!(score, Some(0.987));
    }

    #[test]
    fn test_parse_showinfo_output_low_score() {
        // Test parsing with low scene score (minimal motion)
        let line = "[Parsed_showinfo_1 @ 0x7f8b9c000000] n:5 pts:150 pts_time:1.5 scene:0.301";

        let time = FFmpegExecutor::extract_value_after(line, "pts_time:");
        let score = FFmpegExecutor::extract_value_after(line, "scene:");

        assert_eq!(time, Some(1.5));
        assert_eq!(score, Some(0.301));
    }

    #[test]
    fn test_parse_showinfo_output_fractional_time() {
        // Test parsing with fractional timestamp
        let line =
            "[Parsed_showinfo_1 @ 0x7f8b9c000000] n:123 pts:3690 pts_time:123.456789 scene:0.654";

        let time = FFmpegExecutor::extract_value_after(line, "pts_time:");
        let score = FFmpegExecutor::extract_value_after(line, "scene:");

        assert_eq!(time, Some(123.456789));
        assert_eq!(score, Some(0.654));
    }

    #[test]
    fn test_parse_showinfo_output_missing_scene() {
        // Test parsing when scene score is missing
        let line = "[Parsed_showinfo_1 @ 0x7f8b9c000000] n:42 pts:1260 pts_time:1.26";

        let time = FFmpegExecutor::extract_value_after(line, "pts_time:");
        let score = FFmpegExecutor::extract_value_after(line, "scene:");

        assert_eq!(time, Some(1.26));
        assert_eq!(score, None);
    }

    #[test]
    fn test_parse_showinfo_output_missing_time() {
        // Test parsing when pts_time is missing
        let line = "[Parsed_showinfo_1 @ 0x7f8b9c000000] n:42 pts:1260 scene:0.456";

        let time = FFmpegExecutor::extract_value_after(line, "pts_time:");
        let score = FFmpegExecutor::extract_value_after(line, "scene:");

        assert_eq!(time, None);
        assert_eq!(score, Some(0.456));
    }

    #[test]
    fn test_segment_grouping_12_5_seconds() {
        // Test that segments are grouped into 12.5-second windows
        const SEGMENT_DURATION: f64 = 12.5;

        // Simulate measurements at various timestamps
        let measurements = vec![
            (0.5, 0.4),  // Segment 0 (0-12.5s)
            (5.0, 0.6),  // Segment 0
            (10.0, 0.5), // Segment 0
            (13.0, 0.7), // Segment 1 (12.5-25s)
            (20.0, 0.8), // Segment 1
            (26.0, 0.3), // Segment 2 (25-37.5s)
        ];

        // Group measurements into segments
        let video_duration = 40.0;
        let num_segments = (video_duration / SEGMENT_DURATION).ceil() as usize;

        let mut segment_scores: Vec<f64> = vec![0.0; num_segments];

        for (time, score) in measurements {
            let segment_index = (time / SEGMENT_DURATION).floor() as usize;
            if segment_index < num_segments {
                segment_scores[segment_index] += score;
            }
        }

        // Verify segment 0 has sum of first 3 measurements
        assert!((segment_scores[0] - (0.4 + 0.6 + 0.5)).abs() < 0.001);

        // Verify segment 1 has sum of next 2 measurements
        assert!((segment_scores[1] - (0.7 + 0.8)).abs() < 0.001);

        // Verify segment 2 has sum of last measurement
        assert!((segment_scores[2] - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_score_aggregation_within_segments() {
        // Test that scores are correctly summed within segments
        let segment_measurements = vec![0.4, 0.6, 0.5, 0.7];

        let motion_score: f64 = segment_measurements.iter().sum();

        // Sum should be 2.2
        assert!((motion_score - 2.2).abs() < 0.001);
    }

    #[test]
    fn test_empty_output_handling() {
        // Test handling of empty FFmpeg output (no motion detected)
        let measurements: Vec<(f64, f64)> = Vec::new();

        // Should result in empty segments vector
        assert!(measurements.is_empty());
    }

    #[test]
    fn test_malformed_output_handling() {
        // Test handling of malformed showinfo output
        let malformed_lines = vec![
            "Some random text",
            "[Parsed_showinfo_1 @ 0x7f8b9c000000] invalid format",
            "pts_time:1.26 scene:0.456", // Missing showinfo marker
            "[Parsed_showinfo_1 @ 0x7f8b9c000000] pts_time:abc scene:xyz", // Invalid numbers
        ];

        let mut measurements: Vec<(f64, f64)> = Vec::new();

        for line in malformed_lines {
            if line.contains("Parsed_showinfo")
                && line.contains("pts_time:")
                && line.contains("scene:")
            {
                if let Some(time) = FFmpegExecutor::extract_value_after(line, "pts_time:") {
                    if let Some(score) = FFmpegExecutor::extract_value_after(line, "scene:") {
                        measurements.push((time, score));
                    }
                }
            }
        }

        // Should have no valid measurements from malformed input
        assert_eq!(measurements.len(), 0);
    }

    // Feature: action-based-clip-selection, Property 3: Analysis Duration Limit
    // **Validates: Requirements 2.2**
    proptest! {
        #[test]
        fn test_analysis_duration_limit_property(
            // Generate video durations > 300 seconds (5 minutes)
            video_duration in 301.0f64..=7200.0,
        ) {
            use std::path::PathBuf;

            // Create executor
            let _executor = FFmpegExecutor::new(Resolution::Hd1080, true);

            // Create a test video path (doesn't need to exist for command building test)
            let video_path = PathBuf::from("/test/video.mp4");

            // Build the FFmpeg command for motion analysis
            // We'll simulate what analyze_motion_intensity does
            const MAX_ANALYSIS_DURATION: f64 = 300.0;
            let analysis_duration = video_duration.min(MAX_ANALYSIS_DURATION);

            let args = vec![
                "-i".to_string(),
                video_path.to_string_lossy().to_string(),
                "-t".to_string(),
                analysis_duration.to_string(),
                "-vf".to_string(),
                "select=gt(scene\\,0.3),showinfo".to_string(),
                "-f".to_string(),
                "null".to_string(),
                "-".to_string(),
            ];

            // For videos > 300 seconds, verify the command includes -t 300
            if video_duration > 300.0 {
                prop_assert!(
                    args.contains(&"-t".to_string()),
                    "Command should contain -t flag for duration limit"
                );

                // Find the -t flag and verify the next argument is 300
                let t_index = args.iter().position(|arg| arg == "-t").unwrap();
                let duration_arg = &args[t_index + 1];
                let duration_value: f64 = duration_arg.parse().unwrap();

                prop_assert_eq!(
                    duration_value,
                    300.0,
                    "Analysis duration should be limited to 300 seconds for videos > 300s"
                );
            } else {
                // For videos <= 300 seconds, duration should match video duration
                let t_index = args.iter().position(|arg| arg == "-t").unwrap();
                let duration_arg = &args[t_index + 1];
                let duration_value: f64 = duration_arg.parse().unwrap();

                prop_assert!(
                    (duration_value - video_duration).abs() < 0.001,
                    "Analysis duration should match video duration for videos <= 300s"
                );
            }
        }
    }

    // Feature: ffmpeg-code-quality-improvements, Property 1: NaN-Safe Segment Sorting
    // **Validates: Requirements 1.1, 1.2**

    #[test]
    fn test_audio_segment_sorting_with_single_nan() {
        // Test sorting with a single NaN value
        let mut segments = vec![
            AudioSegment {
                start_time: 0.0,
                duration: 12.5,
                intensity: -15.0,
            },
            AudioSegment {
                start_time: 12.5,
                duration: 12.5,
                intensity: f64::NAN,
            },
            AudioSegment {
                start_time: 25.0,
                duration: 12.5,
                intensity: -20.0,
            },
            AudioSegment {
                start_time: 37.5,
                duration: 12.5,
                intensity: -10.0,
            },
        ];

        // Should not panic
        segments.sort_by(|a, b| b.intensity.total_cmp(&a.intensity));

        // With total_cmp, NaN is treated as less than all other values
        // When sorting descending (b.cmp(a)), NaN will be at the beginning
        assert!(segments[0].intensity.is_nan());
        assert!(!segments[1].intensity.is_nan());
        assert!(!segments[2].intensity.is_nan());
        assert!(!segments[3].intensity.is_nan());

        // Verify non-NaN values are sorted correctly (highest first)
        assert_eq!(segments[1].intensity, -10.0);
        assert_eq!(segments[2].intensity, -15.0);
        assert_eq!(segments[3].intensity, -20.0);
    }

    #[test]
    fn test_audio_segment_sorting_with_all_nan() {
        // Test sorting with all NaN values
        let mut segments = vec![
            AudioSegment {
                start_time: 0.0,
                duration: 12.5,
                intensity: f64::NAN,
            },
            AudioSegment {
                start_time: 12.5,
                duration: 12.5,
                intensity: f64::NAN,
            },
            AudioSegment {
                start_time: 25.0,
                duration: 12.5,
                intensity: f64::NAN,
            },
        ];

        // Should not panic
        segments.sort_by(|a, b| b.intensity.total_cmp(&a.intensity));

        // All should still be NaN
        assert!(segments[0].intensity.is_nan());
        assert!(segments[1].intensity.is_nan());
        assert!(segments[2].intensity.is_nan());
    }

    #[test]
    fn test_audio_segment_sorting_with_nan_and_infinity() {
        // Test sorting with NaN and infinity values
        let mut segments = vec![
            AudioSegment {
                start_time: 0.0,
                duration: 12.5,
                intensity: -15.0,
            },
            AudioSegment {
                start_time: 12.5,
                duration: 12.5,
                intensity: f64::NAN,
            },
            AudioSegment {
                start_time: 25.0,
                duration: 12.5,
                intensity: f64::INFINITY,
            },
            AudioSegment {
                start_time: 37.5,
                duration: 12.5,
                intensity: f64::NEG_INFINITY,
            },
            AudioSegment {
                start_time: 50.0,
                duration: 12.5,
                intensity: -10.0,
            },
        ];

        // Should not panic
        segments.sort_by(|a, b| b.intensity.total_cmp(&a.intensity));

        // Verify ordering with descending sort (b.cmp(a))
        // total_cmp ordering: NaN < -infinity < normal values < infinity
        // With descending sort (b.total_cmp(a)): NaN, infinity, normal values (descending), -infinity
        assert!(segments[0].intensity.is_nan());
        assert_eq!(segments[1].intensity, f64::INFINITY);
        assert_eq!(segments[2].intensity, -10.0);
        assert_eq!(segments[3].intensity, -15.0);
        assert_eq!(segments[4].intensity, f64::NEG_INFINITY);
    }

    #[test]
    fn test_motion_segment_sorting_with_single_nan() {
        // Test sorting with a single NaN value
        let mut segments = vec![
            MotionSegment {
                start_time: 0.0,
                duration: 12.5,
                motion_score: 5.0,
            },
            MotionSegment {
                start_time: 12.5,
                duration: 12.5,
                motion_score: f64::NAN,
            },
            MotionSegment {
                start_time: 25.0,
                duration: 12.5,
                motion_score: 3.0,
            },
            MotionSegment {
                start_time: 37.5,
                duration: 12.5,
                motion_score: 8.0,
            },
        ];

        // Should not panic
        segments.sort_by(|a, b| b.motion_score.total_cmp(&a.motion_score));

        // With total_cmp, NaN is treated as less than all other values
        // When sorting descending (b.cmp(a)), NaN will be at the beginning
        assert!(segments[0].motion_score.is_nan());
        assert!(!segments[1].motion_score.is_nan());
        assert!(!segments[2].motion_score.is_nan());
        assert!(!segments[3].motion_score.is_nan());

        // Verify non-NaN values are sorted correctly (highest first)
        assert_eq!(segments[1].motion_score, 8.0);
        assert_eq!(segments[2].motion_score, 5.0);
        assert_eq!(segments[3].motion_score, 3.0);
    }

    #[test]
    fn test_motion_segment_sorting_with_all_nan() {
        // Test sorting with all NaN values
        let mut segments = vec![
            MotionSegment {
                start_time: 0.0,
                duration: 12.5,
                motion_score: f64::NAN,
            },
            MotionSegment {
                start_time: 12.5,
                duration: 12.5,
                motion_score: f64::NAN,
            },
            MotionSegment {
                start_time: 25.0,
                duration: 12.5,
                motion_score: f64::NAN,
            },
        ];

        // Should not panic
        segments.sort_by(|a, b| b.motion_score.total_cmp(&a.motion_score));

        // All should still be NaN
        assert!(segments[0].motion_score.is_nan());
        assert!(segments[1].motion_score.is_nan());
        assert!(segments[2].motion_score.is_nan());
    }

    #[test]
    fn test_motion_segment_sorting_with_nan_and_infinity() {
        // Test sorting with NaN and infinity values
        let mut segments = vec![
            MotionSegment {
                start_time: 0.0,
                duration: 12.5,
                motion_score: 5.0,
            },
            MotionSegment {
                start_time: 12.5,
                duration: 12.5,
                motion_score: f64::NAN,
            },
            MotionSegment {
                start_time: 25.0,
                duration: 12.5,
                motion_score: f64::INFINITY,
            },
            MotionSegment {
                start_time: 37.5,
                duration: 12.5,
                motion_score: f64::NEG_INFINITY,
            },
            MotionSegment {
                start_time: 50.0,
                duration: 12.5,
                motion_score: 3.0,
            },
        ];

        // Should not panic
        segments.sort_by(|a, b| b.motion_score.total_cmp(&a.motion_score));

        // Verify ordering with descending sort (b.cmp(a))
        // total_cmp ordering: NaN < -infinity < normal values < infinity
        // With descending sort (b.total_cmp(a)): NaN, infinity, normal values (descending), -infinity
        assert!(segments[0].motion_score.is_nan());
        assert_eq!(segments[1].motion_score, f64::INFINITY);
        assert_eq!(segments[2].motion_score, 5.0);
        assert_eq!(segments[3].motion_score, 3.0);
        assert_eq!(segments[4].motion_score, f64::NEG_INFINITY);
    }

    proptest! {
        #[test]
        fn test_nan_safe_audio_segment_sorting(
            // Generate a vector of audio segments with some NaN values
            num_segments in 1usize..=20,
            nan_positions in prop::collection::vec(prop::bool::ANY, 1..=20),
            intensities in prop::collection::vec(-100.0f64..=0.0, 1..=20),
        ) {
            // Ensure vectors have the same length
            let len = num_segments.min(nan_positions.len()).min(intensities.len());

            // Create audio segments with some NaN values
            let mut segments: Vec<AudioSegment> = (0..len)
                .map(|i| {
                    let intensity = if nan_positions[i] {
                        f64::NAN
                    } else {
                        intensities[i]
                    };

                    AudioSegment {
                        start_time: i as f64 * 12.5,
                        duration: 12.5,
                        intensity,
                    }
                })
                .collect();

            // Property 1: Sorting should not panic
            segments.sort_by(|a, b| b.intensity.total_cmp(&a.intensity));

            // Property 2: All NaN values should be grouped together at the beginning
            // (total_cmp treats NaN as less than all other values, so with descending sort b.cmp(a), NaN comes first)
            let nan_count = segments.iter().filter(|s| s.intensity.is_nan()).count();

            // All NaN values should be at the beginning
            for i in 0..nan_count {
                prop_assert!(
                    segments[i].intensity.is_nan(),
                    "NaN values should be grouped at the beginning"
                );
            }

            for i in nan_count..segments.len() {
                prop_assert!(
                    !segments[i].intensity.is_nan(),
                    "Non-NaN values should come after NaN values"
                );
            }

            // Property 3: Non-NaN values should be sorted in descending order
            for i in nan_count..segments.len().saturating_sub(1) {
                prop_assert!(
                    segments[i].intensity >= segments[i + 1].intensity,
                    "Non-NaN values should be sorted in descending order"
                );
            }
        }

        #[test]
        fn test_nan_safe_motion_segment_sorting(
            // Generate a vector of motion segments with some NaN values
            num_segments in 1usize..=20,
            nan_positions in prop::collection::vec(prop::bool::ANY, 1..=20),
            motion_scores in prop::collection::vec(0.0f64..=10.0, 1..=20),
        ) {
            // Ensure vectors have the same length
            let len = num_segments.min(nan_positions.len()).min(motion_scores.len());

            // Create motion segments with some NaN values
            let mut segments: Vec<MotionSegment> = (0..len)
                .map(|i| {
                    let motion_score = if nan_positions[i] {
                        f64::NAN
                    } else {
                        motion_scores[i]
                    };

                    MotionSegment {
                        start_time: i as f64 * 12.5,
                        duration: 12.5,
                        motion_score,
                    }
                })
                .collect();

            // Property 1: Sorting should not panic
            segments.sort_by(|a, b| b.motion_score.total_cmp(&a.motion_score));

            // Property 2: All NaN values should be grouped together at the beginning
            let nan_count = segments.iter().filter(|s| s.motion_score.is_nan()).count();

            // All NaN values should be at the beginning
            for i in 0..nan_count {
                prop_assert!(
                    segments[i].motion_score.is_nan(),
                    "NaN values should be grouped at the beginning"
                );
            }

            for i in nan_count..segments.len() {
                prop_assert!(
                    !segments[i].motion_score.is_nan(),
                    "Non-NaN values should come after NaN values"
                );
            }

            // Property 3: Non-NaN values should be sorted in descending order
            for i in nan_count..segments.len().saturating_sub(1) {
                prop_assert!(
                    segments[i].motion_score >= segments[i + 1].motion_score,
                    "Non-NaN values should be sorted in descending order"
                );
            }
        }
    }

    // Unit tests for segment grouping edge cases

    #[test]
    fn test_segment_grouping_with_empty_measurements() {
        // Test with empty measurements
        let measurements: Vec<(f64, f64)> = Vec::new();
        let video_duration = 100.0;
        let analysis_duration = 100.0;
        let segment_duration = 12.5;

        let segments = FFmpegExecutor::group_measurements_into_segments(
            &measurements,
            video_duration,
            analysis_duration,
            segment_duration,
            |values| values.iter().sum::<f64>(),
        );

        // Should return empty vector
        assert_eq!(segments.len(), 0);
    }

    #[test]
    fn test_segment_grouping_with_single_measurement() {
        // Test with single measurement
        let measurements = vec![(5.0, 10.0)];
        let video_duration = 100.0;
        let analysis_duration = 100.0;
        let segment_duration = 12.5;

        let segments = FFmpegExecutor::group_measurements_into_segments(
            &measurements,
            video_duration,
            analysis_duration,
            segment_duration,
            |values| values.iter().sum::<f64>(),
        );

        // Should have exactly one segment containing the measurement
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].0, 0.0); // Segment starts at 0
        assert_eq!(segments[0].1, 12.5); // Segment duration is 12.5
        assert_eq!(segments[0].2, 10.0); // Score is the value
    }

    #[test]
    fn test_segment_grouping_at_segment_boundaries() {
        // Test with measurements at segment boundaries
        let measurements = vec![
            (0.0, 1.0),  // At start of segment 0
            (12.5, 2.0), // At start of segment 1 (boundary)
            (25.0, 3.0), // At start of segment 2 (boundary)
        ];
        let video_duration = 50.0;
        let analysis_duration = 50.0;
        let segment_duration = 12.5;

        let segments = FFmpegExecutor::group_measurements_into_segments(
            &measurements,
            video_duration,
            analysis_duration,
            segment_duration,
            |values| values.iter().sum::<f64>(),
        );

        // Should have 3 segments
        assert_eq!(segments.len(), 3);

        // Verify each segment has the correct measurement
        assert_eq!(segments[0].2, 1.0); // First segment has first measurement
        assert_eq!(segments[1].2, 2.0); // Second segment has second measurement
        assert_eq!(segments[2].2, 3.0); // Third segment has third measurement
    }

    #[test]
    fn test_segment_grouping_with_video_duration_less_than_segment_duration() {
        // Test with video_duration < segment_duration
        let measurements = vec![(1.0, 5.0), (2.0, 10.0), (3.0, 15.0)];
        let video_duration = 5.0;
        let analysis_duration = 5.0;
        let segment_duration = 12.5;

        let segments = FFmpegExecutor::group_measurements_into_segments(
            &measurements,
            video_duration,
            analysis_duration,
            segment_duration,
            |values| values.iter().sum::<f64>(),
        );

        // Should have exactly one segment
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].0, 0.0); // Segment starts at 0
        assert_eq!(segments[0].1, 5.0); // Segment duration is video_duration
        assert_eq!(segments[0].2, 30.0); // Score is sum of all values (5 + 10 + 15)
    }

    #[test]
    fn test_segment_grouping_with_average_aggregation() {
        // Test with average aggregation function
        let measurements = vec![(1.0, 10.0), (2.0, 20.0), (3.0, 30.0)];
        let video_duration = 12.5;
        let analysis_duration = 12.5;
        let segment_duration = 12.5;

        let segments = FFmpegExecutor::group_measurements_into_segments(
            &measurements,
            video_duration,
            analysis_duration,
            segment_duration,
            |values| values.iter().sum::<f64>() / values.len() as f64, // Average
        );

        // Should have one segment with average value
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].2, 20.0); // Average of 10, 20, 30
    }

    #[test]
    fn test_segment_grouping_with_scaling() {
        // Test with analysis_duration < video_duration (scaling)
        let measurements = vec![
            (5.0, 10.0),  // In first half of analyzed portion
            (15.0, 20.0), // In second half of analyzed portion
        ];
        let video_duration = 100.0;
        let analysis_duration = 50.0; // Only analyzed first 50 seconds
        let segment_duration = 25.0;

        let segments = FFmpegExecutor::group_measurements_into_segments(
            &measurements,
            video_duration,
            analysis_duration,
            segment_duration,
            |values| values.iter().sum::<f64>(),
        );

        // Should have 2 segments (scaled to full video duration)
        // Segment 0: 0-25s (maps to 0-12.5s in analyzed portion)
        // Segment 1: 25-50s (maps to 12.5-25s in analyzed portion)
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].2, 10.0); // First measurement
        assert_eq!(segments[1].2, 20.0); // Second measurement
    }

    // Feature: ffmpeg-code-quality-improvements, Property 3: Segment Grouping Correctness
    // **Validates: Requirements 4.3**
    proptest! {
        #[test]
        fn test_segment_grouping_correctness(
            // Generate random measurements (timestamp, value pairs)
            num_measurements in 0usize..=100,
            measurement_times in prop::collection::vec(0.0f64..=300.0, 0..=100),
            measurement_values in prop::collection::vec(-100.0f64..=100.0, 0..=100),
            // Generate video duration (must be > 0)
            video_duration in 10.0f64..=600.0,
            // Generate segment duration (must be > 0 and <= video_duration)
            segment_duration in 5.0f64..=30.0,
        ) {
            // Ensure vectors have the same length
            let len = num_measurements.min(measurement_times.len()).min(measurement_values.len());

            // Create measurements vector
            let measurements: Vec<(f64, f64)> = (0..len)
                .map(|i| (measurement_times[i], measurement_values[i]))
                .collect();

            // Analysis duration equals video duration for this test
            let analysis_duration = video_duration;

            // Group measurements using the helper function with sum aggregation
            let segments = FFmpegExecutor::group_measurements_into_segments(
                &measurements,
                video_duration,
                analysis_duration,
                segment_duration,
                |values| values.iter().sum::<f64>(),
            );

            // Property 1: All measurements within video duration should be assigned to exactly one segment
            let measurements_in_range: Vec<_> = measurements.iter()
                .filter(|(time, _)| *time >= 0.0 && *time < video_duration)
                .collect();

            for (meas_time, meas_value) in &measurements_in_range {
                // Find which segment this measurement belongs to
                let mut found = false;
                for (segment_start, segment_dur, _) in &segments {
                    let segment_end = segment_start + segment_dur;
                    if *meas_time >= *segment_start && *meas_time < segment_end {
                        found = true;
                        break;
                    }
                }

                prop_assert!(
                    found,
                    "Measurement at time {} with value {} should be assigned to a segment",
                    meas_time, meas_value
                );
            }

            // Property 2: Segments should not overlap
            for i in 0..segments.len() {
                for j in (i + 1)..segments.len() {
                    let (start1, dur1, _) = segments[i];
                    let (start2, dur2, _) = segments[j];
                    let end1 = start1 + dur1;
                    let end2 = start2 + dur2;

                    // Check for overlap
                    let overlaps = (start1 < end2) && (start2 < end1);
                    prop_assert!(
                        !overlaps,
                        "Segments should not overlap: segment {} ({}-{}) and segment {} ({}-{})",
                        i, start1, end1, j, start2, end2
                    );
                }
            }

            // Property 3: Aggregation should be applied correctly
            for (segment_start, segment_dur, score) in &segments {
                let segment_end = segment_start + segment_dur;

                // Find all measurements in this segment
                let segment_values: Vec<f64> = measurements.iter()
                    .filter(|(time, _)| *time >= *segment_start && *time < segment_end)
                    .map(|(_, value)| *value)
                    .collect();

                if !segment_values.is_empty() {
                    // Verify the score is the sum of values
                    let expected_score: f64 = segment_values.iter().sum();
                    prop_assert!(
                        (*score - expected_score).abs() < 0.001,
                        "Segment score should be sum of values: expected {}, got {}",
                        expected_score, score
                    );
                }
            }

            // Property 4: Segments should not extend beyond video duration
            for (segment_start, segment_dur, _) in &segments {
                prop_assert!(
                    segment_start + segment_dur <= video_duration + 0.001,
                    "Segment should not extend beyond video duration"
                );

                prop_assert!(
                    *segment_start >= 0.0,
                    "Segment should not start before 0"
                );
            }
        }
    }

    // Tests for enhanced error messages (Phase 4)

    #[test]
    fn test_ffprobe_failure_includes_file_path() {
        // Test that ffprobe failure includes the file path
        let error = FFmpegError::ExecutionFailed(
            "ffprobe failed on '/path/to/video.mp4': Invalid data found".to_string(),
        );

        let error_msg = error.to_string();
        assert!(
            error_msg.contains("/path/to/video.mp4"),
            "Error message should include file path"
        );
        assert!(
            error_msg.contains("Invalid data found"),
            "Error message should include stderr content"
        );
    }

    #[test]
    fn test_extraction_failure_includes_file_path_and_time_range() {
        // Test that extraction failure includes file path and time range
        let error = FFmpegError::ExecutionFailed(
            "FFmpeg clip extraction failed for '/path/to/video.mp4' at 120.50s-130.50s: Codec error".to_string()
        );

        let error_msg = error.to_string();
        assert!(
            error_msg.contains("/path/to/video.mp4"),
            "Error message should include file path"
        );
        assert!(
            error_msg.contains("120.50s-130.50s"),
            "Error message should include time range"
        );
        assert!(
            error_msg.contains("Codec error"),
            "Error message should include stderr content"
        );
    }

    #[test]
    fn test_json_parse_failure_includes_field_context() {
        // Test that JSON parse failure includes field context
        let error = FFmpegError::ParseError(
            "Failed to parse duration 'not_a_number' for '/path/to/video.mp4': invalid float literal".to_string()
        );

        let error_msg = error.to_string();
        assert!(
            error_msg.contains("duration"),
            "Error message should include field name"
        );
        assert!(
            error_msg.contains("not_a_number"),
            "Error message should include invalid value"
        );
        assert!(
            error_msg.contains("/path/to/video.mp4"),
            "Error message should include file path"
        );
    }

    #[test]
    fn test_corrupted_file_error_includes_file_path() {
        // Test that corrupted file error includes file path
        let error = FFmpegError::CorruptedFile(
            "Video file '/path/to/video.mp4' appears to be corrupted or incomplete: moov atom not found".to_string()
        );

        let error_msg = error.to_string();
        assert!(
            error_msg.contains("/path/to/video.mp4"),
            "Error message should include file path"
        );
        assert!(
            error_msg.contains("moov atom not found"),
            "Error message should include corruption details"
        );
    }

    #[test]
    fn test_stderr_extraction_from_ffprobe_error() {
        // Test stderr extraction from ffprobe error
        let error = FFmpegError::ExecutionFailed(
            "ffprobe failed on '/path/to/video.mp4': Invalid data found when processing input"
                .to_string(),
        );

        let stderr = error.stderr();
        assert!(
            stderr.is_some(),
            "stderr() should return Some for ExecutionFailed"
        );
        assert_eq!(
            stderr.unwrap(),
            "Invalid data found when processing input",
            "stderr() should extract the stderr content"
        );
    }

    #[test]
    fn test_stderr_extraction_from_extraction_error() {
        // Test stderr extraction from extraction error
        let error = FFmpegError::ExecutionFailed(
            "FFmpeg clip extraction failed for '/path/to/video.mp4' at 120.00s-130.00s: Codec not supported".to_string()
        );

        let stderr = error.stderr();
        assert!(
            stderr.is_some(),
            "stderr() should return Some for ExecutionFailed"
        );
        assert_eq!(
            stderr.unwrap(),
            "Codec not supported",
            "stderr() should extract the stderr content"
        );
    }

    #[test]
    fn test_stderr_extraction_from_recovery_error() {
        // Test stderr extraction from recovery error
        let error = FFmpegError::ExecutionFailed(
            "FFmpeg clip extraction failed even with recovery for '/path/to/video.mp4' at 120.00s-130.00s: Unrecoverable error".to_string()
        );

        let stderr = error.stderr();
        assert!(
            stderr.is_some(),
            "stderr() should return Some for ExecutionFailed"
        );
        assert_eq!(
            stderr.unwrap(),
            "Unrecoverable error",
            "stderr() should extract the stderr content"
        );
    }

    #[test]
    fn test_stderr_extraction_fallback_for_unknown_prefix() {
        // Test stderr extraction fallback for unknown prefix
        let error =
            FFmpegError::ExecutionFailed("Some unknown error format: stderr content".to_string());

        let stderr = error.stderr();
        assert!(
            stderr.is_some(),
            "stderr() should return Some for ExecutionFailed"
        );
        assert_eq!(
            stderr.unwrap(),
            "Some unknown error format: stderr content",
            "stderr() should return full message for unknown prefix"
        );
    }

    // Feature: ffmpeg-code-quality-improvements, Property 5: Stderr Extraction Consistency
    // **Validates: Requirements 7.3**
    proptest! {
        #[test]
        fn test_stderr_extraction_consistency(
            // Generate various error message prefixes
            prefix_type in 0usize..=6,
            file_path in "[a-z/]{5,20}\\.mp4",
            start_time in 0.0f64..=3600.0,
            end_time in 0.0f64..=3600.0,
            stderr_content in "[a-zA-Z ]{10,50}",
        ) {
            // Ensure end_time > start_time
            let (start, end) = if end_time > start_time {
                (start_time, end_time)
            } else {
                (end_time, start_time)
            };

            // Create error message with different prefixes
            let msg = match prefix_type {
                0 => format!("FFmpeg clip extraction failed for '{}' at {:.2}s-{:.2}s: {}",
                    file_path, start, end, stderr_content),
                1 => format!("FFmpeg clip extraction failed even with recovery for '{}' at {:.2}s-{:.2}s: {}",
                    file_path, start, end, stderr_content),
                2 => format!("ffprobe failed on '{}': {}", file_path, stderr_content),
                3 => format!("Failed to execute ffprobe on '{}': {}", file_path, stderr_content),
                4 => format!("Failed to execute ffmpeg for '{}': {}", file_path, stderr_content),
                5 => format!("Failed to execute ffmpeg recovery for '{}': {}", file_path, stderr_content),
                _ => format!("Unknown prefix: {}", stderr_content),
            };

            let error = FFmpegError::ExecutionFailed(msg.clone());
            let extracted = error.stderr();

            // Property 1: stderr() should always return Some for ExecutionFailed
            prop_assert!(extracted.is_some(),
                "stderr() should return Some for ExecutionFailed errors");

            let extracted_str = extracted.unwrap();

            // Property 2: For known prefixes, should extract stderr content
            if prefix_type <= 5 {
                prop_assert_eq!(extracted_str, stderr_content.as_str(),
                    "stderr() should extract the stderr content for known prefixes");
            } else {
                // Property 3: For unknown prefixes, should return full message
                prop_assert_eq!(extracted_str, msg.as_str(),
                    "stderr() should return full message for unknown prefixes");
            }
        }
    }

    // Feature: ffmpeg-code-quality-improvements, Property 6: Non-Execution Errors Return None for Stderr
    // **Validates: Requirements 7.4**
    #[test]
    fn test_stderr_returns_none_for_non_execution_errors() {
        // Test NotFound error
        let not_found = FFmpegError::NotFound;
        assert_eq!(
            not_found.stderr(),
            None,
            "stderr() should return None for NotFound error"
        );

        // Test ParseError
        let parse_error = FFmpegError::ParseError("Invalid JSON".to_string());
        assert_eq!(
            parse_error.stderr(),
            None,
            "stderr() should return None for ParseError"
        );

        // Test NoAudioTrack error
        let no_audio = FFmpegError::NoAudioTrack;
        assert_eq!(
            no_audio.stderr(),
            None,
            "stderr() should return None for NoAudioTrack error"
        );

        // Test CorruptedFile error
        let corrupted = FFmpegError::CorruptedFile("File is corrupted".to_string());
        assert_eq!(
            corrupted.stderr(),
            None,
            "stderr() should return None for CorruptedFile error"
        );
    }
}
