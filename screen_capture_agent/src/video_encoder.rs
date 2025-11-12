use crate::qos::VideoQoS;
use log::{debug, info, warn};
use scrap::codec::{Encoder, EncoderCfg, BR_BALANCED};
use scrap::vpxcodec::{VpxEncoderConfig, VpxVideoCodecId};
use scrap::{video_frame, CodecFormat, EncodeInput, EncodeYuvFormat, VideoFrame};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
const BR_MAX: f32 = 4.0;
const BR_MIN_HIGH_RESOLUTION: f32 = 0.1;
const BR_DEFAULT: f32 = 1.0;
const MAX_FRAME_SIZE_1080P: usize = 1_200_000;
const TARGET_FRAME_SIZE_1080P: usize = 200_000;
const QUALITY_ADJUST_INTERVAL: Duration = Duration::from_secs(5);
const FRAME_SIZE_HISTORY_LEN: usize = 20;

pub fn align_dimensions(width: usize, height: usize) -> (usize, usize) {
    let aligned_width = if width % 2 == 0 {
        width
    } else {
        width.saturating_sub(1)
    };
    let aligned_height = if height % 2 == 0 {
        height
    } else {
        height.saturating_sub(1)
    };
    (aligned_width, aligned_height)
}

#[derive(Debug, Clone)]
pub struct VideoQuality {
    pub ratio: f32,
    pub bitrate: u32,
    pub keyframe_interval: Option<usize>,
}
impl Default for VideoQuality {
    fn default() -> Self {
        Self {
            ratio: 0.5,
            bitrate: 2_000_000,
            keyframe_interval: Some(60), // 60 frames = 2 seconds at 30fps - needed for new clients to decode
        }
    }
}
#[derive(Clone)]
pub struct VideoEncoderConfig {
    pub width: usize,
    pub height: usize,
    pub quality: VideoQuality,
    pub codec_format: CodecFormat,
    pub fps: u32,
}
impl VideoEncoderConfig {
    pub fn new(width: usize, height: usize) -> Self {
        let (aligned_width, aligned_height) = align_dimensions(width, height);

        // RustDesk approach: VP8 is default for real-time, VP9 only for high-end systems
        // VP8 with CPUUSED=12 is 3-5x faster than VP9 with similar quality
        // CRITICAL: For systems with 4 cores and typical load, VP8 is the ONLY viable option
        // VP9 is too slow for real-time at 30fps on most hardware
        let codec_format = CodecFormat::VP8;

        // RustDesk-style bitrate calculation
        let bitrate = match (aligned_width, aligned_height) {
            (1920, 1080) => 2_500_000, // Lower for VP8
            (1280, 720) => 1_500_000,
            (1366, 768) => 1_500_000,
            _ => 800_000,
        };

        // RustDesk uses 0.5-0.7 quality, but starts at 0.5 for VP8
        // VP8 needs less quality ratio than VP9 for similar visual quality
        Self {
            width: aligned_width,
            height: aligned_height,
            quality: VideoQuality {
                ratio: BR_BALANCED,
                bitrate,
                keyframe_interval: Some(60), // 60 frames = 2 seconds at 30fps - needed for new clients to decode
            },
            codec_format,
            fps: 30,
        }
    }
    pub fn with_quality(mut self, quality: f32) -> Self {
        self.quality.ratio = quality.clamp(BR_MIN_HIGH_RESOLUTION, BR_MAX);
        self
    }
    pub fn with_codec(mut self, codec: CodecFormat) -> Self {
        self.codec_format = codec;
        self
    }
    pub fn with_fps(mut self, fps: u32) -> Self {
        self.fps = fps;
        self
    }
}
#[derive(Debug)]
struct FrameSizeTracker {
    frame_sizes: VecDeque<usize>,
    total_size: usize,
    large_frame_count: usize,
    last_adjustment: Instant,
}
impl Default for FrameSizeTracker {
    fn default() -> Self {
        Self {
            frame_sizes: VecDeque::new(),
            total_size: 0,
            large_frame_count: 0,
            last_adjustment: Instant::now(),
        }
    }
}
impl FrameSizeTracker {
    fn add_frame_size(&mut self, size: usize) {
        if self.frame_sizes.len() >= FRAME_SIZE_HISTORY_LEN {
            if let Some(old_size) = self.frame_sizes.pop_front() {
                self.total_size -= old_size;
            }
        }
        self.frame_sizes.push_back(size);
        self.total_size += size;
        if size > TARGET_FRAME_SIZE_1080P * 2 {
            self.large_frame_count += 1;
        }
    }
    fn average_frame_size(&self) -> usize {
        if self.frame_sizes.is_empty() {
            0
        } else {
            self.total_size / self.frame_sizes.len()
        }
    }
    fn should_adjust_quality(&mut self) -> Option<f32> {
        if self.last_adjustment.elapsed() < QUALITY_ADJUST_INTERVAL {
            return None;
        }
        let avg_size = self.average_frame_size();
        let large_frame_ratio = self.large_frame_count as f32 / self.frame_sizes.len() as f32;
        let adjustment = if avg_size > TARGET_FRAME_SIZE_1080P * 3 || large_frame_ratio > 0.3 {
            -0.3
        } else if avg_size > TARGET_FRAME_SIZE_1080P * 2 || large_frame_ratio > 0.2 {
            -0.2
        } else if avg_size > (TARGET_FRAME_SIZE_1080P as f32 * 1.5) as usize {
            -0.1
        } else if avg_size < TARGET_FRAME_SIZE_1080P / 2 && large_frame_ratio < 0.05 {
            0.1
        } else {
            return None;
        };
        self.last_adjustment = Instant::now();
        self.large_frame_count = 0;
        Some(adjustment)
    }
}
pub struct EnhancedVideoEncoder {
    encoder: Option<Encoder>,
    config: VideoEncoderConfig,
    frame_count: u64,
    last_keyframe: u64,
    last_encode_time: Instant,
    encode_stats: EncodeStats,
    frame_size_tracker: FrameSizeTracker,
    current_quality: f32,
    force_next_frame_keyframe: bool,
    last_frame_was_keyframe: bool,
    dummy_mode: bool,
    qos: VideoQoS,
    retry_encode_counter: usize,
    would_block_count: u32,
}
#[derive(Debug, Default)]
struct EncodeStats {
    total_frames: u64,
    total_size: u64,
    avg_encode_time: f32,
    keyframes: u64,
    skipped_frames: u64,
    large_frames: u64,
}
impl EnhancedVideoEncoder {
    pub fn new(config: VideoEncoderConfig) -> Result<Self, Box<dyn std::error::Error>> {
        if config.width == 0 || config.height == 0 {
            return Err("Invalid encoder dimensions: width and height must be > 0".into());
        }
        if config.width > 4096 || config.height > 4096 {
            return Err("Encoder dimensions too large: maximum 4096x4096".into());
        }
        if config.width % 2 != 0 || config.height % 2 != 0 {
            return Err(format!(
                "Encoder dimensions must be even: {}x{}",
                config.width, config.height
            )
            .into());
        }
        info!(
            "Creating encoder with validated config: {}x{} (requested codec: {:?})",
            config.width, config.height, config.codec_format
        );
        let (encoder, actual_config, dummy_mode) = match Self::create_safe_encoder(&config) {
            Ok((enc, adj_config)) => (Some(enc), adj_config, false),
            Err(e) => {
                warn!(
                    "All encoder creation attempts failed: {} - falling back to dummy mode",
                    e
                );
                (None, config, true)
            }
        };
        if dummy_mode {
            warn!(
                "Enhanced video encoder initialized in DUMMY MODE: {}x{} (no actual encoding)",
                actual_config.width, actual_config.height
            );
        } else {
            info!(
                "Enhanced video encoder initialized: {}x{} @ {}fps, quality: {:.2}, codec: {:?}",
                actual_config.width,
                actual_config.height,
                actual_config.fps,
                actual_config.quality.ratio,
                actual_config.codec_format
            );
        }
        let current_quality = actual_config.quality.ratio;
        Ok(Self {
            encoder,
            config: actual_config,
            frame_count: 0,
            last_keyframe: 0,
            last_encode_time: Instant::now(),
            encode_stats: EncodeStats::default(),
            frame_size_tracker: FrameSizeTracker::default(),
            current_quality,
            force_next_frame_keyframe: false,
            last_frame_was_keyframe: false,
            dummy_mode,
            qos: VideoQoS::new(),
            retry_encode_counter: 0,
            would_block_count: 0,
        })
    }

    fn create_safe_encoder(
        config: &VideoEncoderConfig,
    ) -> Result<(Encoder, VideoEncoderConfig), Box<dyn std::error::Error>> {
        let (aligned_width, aligned_height) = align_dimensions(config.width, config.height);
        if aligned_width != config.width || aligned_height != config.height {
            info!(
                "Aligning encoder dimensions from {}x{} to {}x{} using even-dimension requirement",
                config.width, config.height, aligned_width, aligned_height
            );
        }
        let adjusted_config = VideoEncoderConfig {
            width: aligned_width,
            height: aligned_height,
            quality: config.quality.clone(),
            codec_format: config.codec_format,
            fps: config.fps,
        };

        if let Ok(encoder) =
            Self::try_create_encoder_with_config(&adjusted_config, adjusted_config.codec_format)
        {
            info!(
                "Encoder created with requested codec: {:?}",
                adjusted_config.codec_format
            );
            return Ok((encoder, adjusted_config));
        }

        warn!("Requested codec failed, trying VP8 fallback (RustDesk primary fallback)");
        let mut fallback_config = adjusted_config.clone();
        fallback_config.quality.ratio =
            (adjusted_config.quality.ratio * 0.8).max(BR_MIN_HIGH_RESOLUTION);
        if let Ok(encoder) =
            Self::try_create_encoder_with_config(&fallback_config, CodecFormat::VP8)
        {
            info!("Encoder created with VP8 fallback");
            return Ok((encoder, fallback_config));
        }

        warn!("VP8 failed, trying VP9 fallback");
        fallback_config.quality.ratio = BR_DEFAULT;
        if let Ok(encoder) =
            Self::try_create_encoder_with_config(&fallback_config, CodecFormat::VP9)
        {
            info!("Encoder created with VP9 fallback");
            return Ok((encoder, fallback_config));
        }

        warn!("VP8 failed, trying minimal quality VP8 (last resort)");
        let mut minimal_config = adjusted_config.clone();
        minimal_config.quality.ratio = BR_MIN_HIGH_RESOLUTION;
        minimal_config.quality.bitrate = 1_000_000;
        minimal_config.fps = 15;
        if let Ok(encoder) = Self::try_create_encoder_with_config(&minimal_config, CodecFormat::VP8)
        {
            info!("Encoder created with minimal VP8 configuration");
            return Ok((encoder, minimal_config));
        }

        warn!("All standard encoders failed - attempting software-only encoder");
        Self::create_software_only_encoder(&adjusted_config)
    }
    fn create_software_only_encoder(
        config: &VideoEncoderConfig,
    ) -> Result<(Encoder, VideoEncoderConfig), Box<dyn std::error::Error>> {
        info!("Creating software-only encoder for maximum safety");
        let safe_config = VideoEncoderConfig {
            width: config.width & !3,   // Align to 4-pixel boundary
            height: config.height & !3, // Align to 4-pixel boundary
            quality: VideoQuality {
                ratio: 0.1,
                bitrate: 500_000,
                keyframe_interval: Some(300),
            },
            codec_format: CodecFormat::VP8,
            fps: 15,
        };
        let encoder_cfg = EncoderCfg::VPX(VpxEncoderConfig {
            width: safe_config.width as u32,
            height: safe_config.height as u32,
            quality: safe_config.quality.ratio,
            codec: VpxVideoCodecId::VP8,
            keyframe_interval: safe_config.quality.keyframe_interval,
        });
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Encoder::new(encoder_cfg, false)
        })) {
            Ok(Ok(encoder)) => {
                info!("Software-only encoder created successfully");
                Ok((encoder, safe_config))
            }
            Ok(Err(e)) => {
                warn!("Software encoder creation failed: {}", e);
                Err(format!("Software encoder failed: {}", e).into())
            }
            Err(panic_info) => {
                warn!("Even software encoder panicked: {:?}", panic_info);
                Err("Software encoder panicked - system may be unstable".into())
            }
        }
    }
    fn try_create_encoder_with_config(
        config: &VideoEncoderConfig,
        codec: CodecFormat,
    ) -> Result<Encoder, Box<dyn std::error::Error>> {
        let mut safe_config = config.clone();
        safe_config.codec_format = codec;
        let encoder_cfg = Self::create_encoder_config(&safe_config)?;
        let use_i444 = false;
        let encoder_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Encoder::new(encoder_cfg, use_i444)
        }));
        match encoder_result {
            Ok(Ok(encoder)) => {
                info!("Encoder created successfully with codec: {:?}", codec);
                Ok(encoder)
            }
            Ok(Err(e)) => {
                warn!("Encoder creation failed: {}", e);
                Err(format!("Encoder creation failed: {}", e).into())
            }
            Err(panic_info) => {
                warn!("Encoder creation panicked: {:?}", panic_info);
                Err("Encoder creation panicked - system may be unstable".into())
            }
        }
    }
    pub fn was_last_frame_keyframe(&self) -> bool {
        self.last_frame_was_keyframe
    }
    fn is_keyframe_from_video_frame(&self, video_frame: &VideoFrame) -> bool {
        match &video_frame.union {
            Some(video_frame::Union::Vp9s(encoded_frames)) => {
                encoded_frames.frames.iter().any(|frame| {
                    if frame.key {
                        return true;
                    }
                    if frame.data.len() >= 3 {
                        let first_bytes = &frame.data[0..3];
                        (first_bytes[0] & 0x01) == 0
                    } else {
                        false
                    }
                })
            }
            Some(video_frame::Union::Vp8s(encoded_frames)) => {
                encoded_frames.frames.iter().any(|frame| {
                    if frame.key {
                        return true;
                    }
                    if frame.data.len() >= 3 {
                        let first_bytes = &frame.data[0..3];
                        (first_bytes[0] & 0x01) == 0
                    } else {
                        false
                    }
                })
            }
            Some(video_frame::Union::H264s(encoded_frames)) => {
                encoded_frames.frames.iter().any(|frame| {
                    if frame.key {
                        return true;
                    }
                    if frame.data.len() >= 4 {
                        for i in 0..frame.data.len().saturating_sub(3) {
                            if frame.data[i] == 0x00
                                && frame.data[i + 1] == 0x00
                                && frame.data[i + 2] == 0x00
                                && frame.data[i + 3] == 0x01
                            {
                                if i + 4 < frame.data.len() {
                                    let nal_type = frame.data[i + 4] & 0x1F;
                                    if nal_type == 5 {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                    false
                })
            }
            Some(video_frame::Union::H265s(encoded_frames)) => {
                encoded_frames.frames.iter().any(|frame| {
                    if frame.key {
                        return true;
                    }
                    if frame.data.len() >= 4 {
                        for i in 0..frame.data.len().saturating_sub(3) {
                            if frame.data[i] == 0x00
                                && frame.data[i + 1] == 0x00
                                && frame.data[i + 2] == 0x00
                                && frame.data[i + 3] == 0x01
                            {
                                if i + 4 < frame.data.len() {
                                    let nal_type = (frame.data[i + 4] >> 1) & 0x3F;
                                    if nal_type >= 16 && nal_type <= 23 {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                    false
                })
            }
            Some(video_frame::Union::Av1s(encoded_frames)) => {
                encoded_frames.frames.iter().any(|frame| frame.key)
            }
            _ => false,
        }
    }
    fn create_encoder_config(
        config: &VideoEncoderConfig,
    ) -> Result<EncoderCfg, Box<dyn std::error::Error>> {
        if config.width == 0 || config.height == 0 {
            return Err("Invalid encoder config: width and height must be > 0".into());
        }
        if config.width > 4096 || config.height > 4096 {
            return Err("Encoder config dimensions too large: maximum 4096x4096".into());
        }
        let safe_width = config.width as u32;
        let safe_height = config.height as u32;
        let adjusted_quality = match (config.width, config.height) {
            (1366, 768) => {
                debug!("Applying special quality adjustment for 1366x768 resolution");
                (config.quality.ratio * 0.8).max(BR_MIN_HIGH_RESOLUTION)
            }
            _ => config.quality.ratio,
        };
        let keyframe_interval = config.quality.keyframe_interval;
        debug!(
            "Creating encoder config: {}x{}, quality: {:.2}",
            safe_width, safe_height, adjusted_quality
        );
        let (codec, desc) = match config.codec_format {
            CodecFormat::VP9 => (VpxVideoCodecId::VP9, "VP9"),
            CodecFormat::VP8 => (VpxVideoCodecId::VP8, "VP8"),
            other => {
                warn!(
                    "Codec {:?} not supported, falling back to VP8 configuration",
                    other
                );
                (VpxVideoCodecId::VP8, "VP8")
            }
        };
        debug!("Using {desc} codec configuration");
        let encoder_cfg = EncoderCfg::VPX(VpxEncoderConfig {
            width: safe_width,
            height: safe_height,
            quality: adjusted_quality,
            codec,
            keyframe_interval,
        });
        Ok(encoder_cfg)
    }
    pub fn force_keyframe(&mut self) {
        // Note: RustDesk doesn't support forcing keyframes through the encoder API.
        // Keyframes are generated automatically based on keyframe_interval configuration.
        self.force_next_frame_keyframe = true;
        debug!("Keyframe requested - will be generated based on keyframe_interval");
    }

    pub fn support_changing_quality(&self) -> bool {
        if let Some(ref encoder) = self.encoder {
            encoder.support_changing_quality()
        } else {
            false
        }
    }

    pub fn bitrate(&self) -> u32 {
        if let Some(ref encoder) = self.encoder {
            encoder.bitrate()
        } else {
            0
        }
    }

    pub fn target_dimensions(&self) -> (usize, usize) {
        (self.config.width, self.config.height)
    }

    pub fn codec_format(&self) -> CodecFormat {
        self.config.codec_format
    }

    pub fn is_dummy(&self) -> bool {
        self.dummy_mode
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    pub fn expected_stride(&self) -> usize {
        self.config.width * 4
    }

    pub fn yuv_format(&self) -> Option<EncodeYuvFormat> {
        self.encoder.as_ref().map(|enc| enc.yuvfmt())
    }

    pub fn set_quality(&mut self, quality: f32) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut encoder) = self.encoder {
            encoder.set_quality(quality)?;
            self.current_quality = quality;
        }
        Ok(())
    }
    pub fn encode_input(
        &mut self,
        encode_input: EncodeInput,
        timestamp_ms: i64,
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        let encode_start = Instant::now();
        let width = self.config.width;

        if self.dummy_mode || self.encoder.is_none() {
            debug!("Dummy mode: returning fake encoded data");
            self.frame_count += 1;
            let fake_data = vec![0u8; 100];
            self.update_stats(0.001, fake_data.len());
            return Ok(Some(fake_data));
        }

        let encode_start_time = Instant::now();
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.encoder
                .as_mut()
                .unwrap()
                .encode_to_message(encode_input, timestamp_ms)
        })) {
            Ok(Ok(video_frame)) => {
                let encode_time = encode_start_time.elapsed().as_millis();
                if encode_time > 50 && self.frame_count % 10 == 0 {
                    debug!(
                        "VPX encode took {}ms (target: <33ms for 30fps)",
                        encode_time
                    );
                }

                self.frame_count += 1;
                let is_actual_keyframe = self.is_keyframe_from_video_frame(&video_frame);
                self.last_frame_was_keyframe = is_actual_keyframe;
                if is_actual_keyframe {
                    self.last_keyframe = self.frame_count;
                    self.encode_stats.keyframes += 1;
                    if self.force_next_frame_keyframe {
                        self.force_next_frame_keyframe = false;
                        debug!(
                            "Force keyframe request fulfilled at frame {}",
                            self.frame_count
                        );
                    }
                    debug!("Actual keyframe produced at frame {}", self.frame_count);
                }
                let encode_time = encode_start.elapsed().as_secs_f32();

                self.log_video_frame_details(&video_frame);

                let encoded_data = match self.video_frame_to_bytes(&video_frame) {
                    Ok(data) => data,
                    Err(e) => {
                        if self.frame_count % 100 == 0 {
                            warn!("Failed to extract frame data: {}, skipping frame", e);
                        }
                        return Ok(None);
                    }
                };
                if encoded_data.is_empty() {
                    warn!(
                        "Encoded video frame {} returned empty payload (variant logged above) - skipping",
                        self.frame_count
                    );
                    return Ok(None);
                }
                let frame_size = encoded_data.len();
                let max_size = if width >= 1920 {
                    MAX_FRAME_SIZE_1080P
                } else {
                    MAX_FRAME_SIZE_1080P / 2
                };
                if frame_size > max_size {
                    self.encode_stats.large_frames += 1;
                    if self.encode_stats.large_frames % 10 == 0 {
                        warn!(
                            "Frame too large ({} bytes > {} max), attempting quality reduction",
                            frame_size, max_size
                        );
                    }
                    if self.current_quality > BR_MIN_HIGH_RESOLUTION + 0.1 {
                        self.current_quality =
                            (self.current_quality - 0.1).max(BR_MIN_HIGH_RESOLUTION);
                        if let Some(ref mut encoder) = self.encoder {
                            if let Err(_) = encoder.set_quality(self.current_quality) {}
                        }
                    }
                    if frame_size > max_size * 2 {
                        self.encode_stats.skipped_frames += 1;
                        return Ok(None);
                    }
                }
                self.frame_size_tracker.add_frame_size(frame_size);
                if let Some(adjustment) = self.frame_size_tracker.should_adjust_quality() {
                    let new_quality =
                        (self.current_quality + adjustment).clamp(BR_MIN_HIGH_RESOLUTION, BR_MAX);
                    if (new_quality - self.current_quality).abs() > 0.05 {
                        self.current_quality = new_quality;
                        if let Some(ref mut encoder) = self.encoder {
                            if let Err(_) = encoder.set_quality(self.current_quality) {}
                        }
                    }
                }
                self.update_stats(encode_time, frame_size);
                Ok(Some(encoded_data))
            }
            Ok(Err(e)) => {
                if self.frame_count % 100 == 0 {
                    warn!("Encoding error: {:?}", e);
                }
                Ok(None)
            }
            Err(panic_info) => {
                if self.frame_count % 100 == 0 {
                    warn!("Encoder panicked: {:?}", panic_info);
                }
                Ok(None)
            }
        }
    }
    fn should_force_keyframe(&mut self) -> bool {
        let frames_since_keyframe = self.frame_count - self.last_keyframe;
        let keyframe_interval = self.config.quality.keyframe_interval.unwrap_or(60) as u64;
        let force = self.force_next_frame_keyframe
            || frames_since_keyframe >= keyframe_interval
            || self.frame_count == 0;
        if self.force_next_frame_keyframe && force {
            debug!(
                "Attempting to force keyframe at frame {}",
                self.frame_count + 1
            );
        }
        force
    }
    fn video_frame_to_bytes(
        &self,
        video_frame: &VideoFrame,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        match &video_frame.union {
            Some(video_frame::Union::Vp9s(encoded_frames)) => {
                if !encoded_frames.frames.is_empty() {
                    Ok(encoded_frames.frames[0].data.to_vec())
                } else {
                    warn!("VP9 encoded_frames list empty");
                    Ok(Vec::new())
                }
            }
            Some(video_frame::Union::Vp8s(encoded_frames)) => {
                if !encoded_frames.frames.is_empty() {
                    Ok(encoded_frames.frames[0].data.to_vec())
                } else {
                    warn!("VP8 encoded_frames list empty");
                    Ok(Vec::new())
                }
            }
            Some(video_frame::Union::Av1s(encoded_frames)) => {
                if !encoded_frames.frames.is_empty() {
                    Ok(encoded_frames.frames[0].data.to_vec())
                } else {
                    warn!("AV1 encoded_frames list empty");
                    Ok(Vec::new())
                }
            }
            Some(video_frame::Union::H264s(encoded_frames)) => {
                if !encoded_frames.frames.is_empty() {
                    Ok(encoded_frames.frames[0].data.to_vec())
                } else {
                    warn!("H264 encoded_frames list empty");
                    Ok(Vec::new())
                }
            }
            Some(video_frame::Union::H265s(encoded_frames)) => {
                if !encoded_frames.frames.is_empty() {
                    Ok(encoded_frames.frames[0].data.to_vec())
                } else {
                    warn!("H265 encoded_frames list empty");
                    Ok(Vec::new())
                }
            }
            Some(_) => {
                warn!("Unhandled video frame variant (not VP8/VP9/H264/H265/AV1)");
                Ok(Vec::new())
            }
            None => {
                warn!("Video frame union missing, no encoded data available");
                Ok(Vec::new())
            }
        }
    }

    fn log_video_frame_details(&self, video_frame: &VideoFrame) {
        match &video_frame.union {
            Some(video_frame::Union::Vp8s(encoded_frames)) => {
                if let Some(first) = encoded_frames.frames.first() {
                    debug!(
                        "VP8 frame details: count={}, first.len={}, key={}, pts={}",
                        encoded_frames.frames.len(),
                        first.data.len(),
                        first.key,
                        first.pts
                    );
                } else {
                    warn!("VP8 frame details: frames list empty");
                }
            }
            Some(video_frame::Union::Vp9s(encoded_frames)) => {
                if let Some(first) = encoded_frames.frames.first() {
                    debug!(
                        "VP9 frame details: count={}, first.len={}, key={}, pts={}",
                        encoded_frames.frames.len(),
                        first.data.len(),
                        first.key,
                        first.pts
                    );
                } else {
                    warn!("VP9 frame details: frames list empty");
                }
            }
            Some(video_frame::Union::H264s(encoded_frames)) => {
                debug!(
                    "H264 frame details: count={}, first_len={}",
                    encoded_frames.frames.len(),
                    encoded_frames
                        .frames
                        .first()
                        .map(|f| f.data.len())
                        .unwrap_or(0)
                );
            }
            Some(video_frame::Union::H265s(encoded_frames)) => {
                debug!(
                    "H265 frame details: count={}, first_len={}",
                    encoded_frames.frames.len(),
                    encoded_frames
                        .frames
                        .first()
                        .map(|f| f.data.len())
                        .unwrap_or(0)
                );
            }
            Some(video_frame::Union::Av1s(encoded_frames)) => {
                debug!(
                    "AV1 frame details: count={}, first_len={}",
                    encoded_frames.frames.len(),
                    encoded_frames
                        .frames
                        .first()
                        .map(|f| f.data.len())
                        .unwrap_or(0)
                );
            }
            Some(_) => {
                warn!("Unknown video frame variant encountered (non VPx/H26x/AV1)");
            }
            None => {
                warn!("No video frame union present");
            }
        }
    }
    fn update_stats(&mut self, encode_time: f32, frame_size: usize) {
        self.encode_stats.total_frames += 1;
        self.encode_stats.total_size += frame_size as u64;
        let alpha = 0.1;
        if self.encode_stats.total_frames == 1 {
            self.encode_stats.avg_encode_time = encode_time;
        } else {
            self.encode_stats.avg_encode_time =
                alpha * encode_time + (1.0 - alpha) * self.encode_stats.avg_encode_time;
        }
        self.last_encode_time = Instant::now();
    }
}
