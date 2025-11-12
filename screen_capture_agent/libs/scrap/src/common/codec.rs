use crate::common::video::VideoFrame;
use crate::common::vpxcodec::{VpxEncoder, VpxEncoderConfig, VpxVideoCodecId};
use crate::common::{EncodeInput, EncodeYuvFormat};
use anyhow::Result;
use num_cpus;

pub const BR_BEST: f32 = 1.5;
pub const BR_BALANCED: f32 = 0.67;
pub const BR_SPEED: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Quality {
    Best,
    Balanced,
    Low,
    Custom(f32),
}

impl Default for Quality {
    fn default() -> Self {
        Self::Balanced
    }
}

impl Quality {
    pub fn is_custom(&self) -> bool {
        matches!(self, Quality::Custom(_))
    }

    pub fn ratio(&self) -> f32 {
        match self {
            Quality::Best => BR_BEST,
            Quality::Balanced => BR_BALANCED,
            Quality::Low => BR_SPEED,
            Quality::Custom(v) => *v,
        }
    }
}

#[derive(Clone)]
pub enum EncoderCfg {
    VPX(VpxEncoderConfig),
}

pub struct Encoder {
    inner: VpxEncoder,
}

impl Encoder {
    pub fn new(cfg: EncoderCfg, i444: bool) -> Result<Self> {
        let inner = match cfg {
            EncoderCfg::VPX(config) => VpxEncoder::new(config, i444)?,
        };
        Ok(Self { inner })
    }

    pub fn encode_to_message(&mut self, frame: EncodeInput, ms: i64) -> Result<VideoFrame> {
        self.inner.encode_to_message(frame, ms)
    }

    pub fn yuvfmt(&self) -> EncodeYuvFormat {
        self.inner.yuvfmt()
    }

    pub fn support_changing_quality(&self) -> bool {
        true
    }

    pub fn bitrate(&self) -> u32 {
        self.inner.bitrate()
    }

    pub fn set_quality(&mut self, quality: f32) -> Result<()> {
        self.inner.set_quality(quality)
    }

    pub fn codec(&self) -> VpxVideoCodecId {
        self.inner.codec_id()
    }
}

pub fn base_bitrate(width: u32, height: u32) -> u32 {
    const PRESETS: &[(u32, u32, u32)] = &[
        (640, 480, 400),
        (800, 600, 500),
        (1024, 768, 800),
        (1280, 720, 1000),
        (1366, 768, 1100),
        (1440, 900, 1300),
        (1600, 900, 1500),
        (1920, 1080, 2073),
        (2048, 1080, 2200),
        (2560, 1440, 3000),
        (3440, 1440, 4000),
        (3840, 2160, 5000),
        (7680, 4320, 12000),
    ];

    let pixels = width.saturating_mul(height);
    let (preset_pixels, preset_bitrate) = PRESETS
        .iter()
        .map(|(w, h, bitrate)| (w * h, bitrate))
        .min_by_key(|(preset_pixels, _)| {
            if *preset_pixels >= pixels {
                preset_pixels - pixels
            } else {
                pixels - preset_pixels
            }
        })
        .unwrap_or((1920 * 1080, &2073));

    ((*preset_bitrate as f32 * (pixels as f32 / preset_pixels as f32)).round()) as u32
}

pub fn codec_thread_num(limit: usize) -> usize {
    let cpu_count = num_cpus::get().max(1);
    let mut threads = match cpu_count {
        n if n >= 16 => 8,
        n if n >= 8 => 4,
        n if n >= 4 => 2,
        _ => 1,
    };
    threads = threads.min(limit.max(1));
    threads = threads.max(1);
    threads
}
