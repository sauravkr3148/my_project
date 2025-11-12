use anyhow::bail;
use std::ffi::c_void;

pub mod codec;
pub mod convert;
#[cfg(target_os = "windows")]
pub mod dxgi;
pub mod video;
mod vpx;
pub mod vpxcodec;

pub use codec::{
    base_bitrate, codec_thread_num, Encoder, EncoderCfg, Quality, BR_BALANCED, BR_BEST, BR_SPEED,
};
pub use convert::*;
#[cfg(target_os = "windows")]
pub use dxgi::*;
pub use video::{video_frame, Chroma, EncodedVideoFrame, EncodedVideoFrames, VideoFrame};
pub use vpxcodec::{VpxEncoderConfig, VpxVideoCodecId};

pub type ResultType<T> = anyhow::Result<T>;

pub const STRIDE_ALIGN: usize = 64;
pub const HW_STRIDE_ALIGN: usize = 0;

#[macro_export]
macro_rules! generate_call_macro {
    ($func_name:ident, $allow_err:expr) => {
        macro_rules! $func_name {
            ($x:expr) => {{
                let result = unsafe { $x };
                let result_int = unsafe { std::mem::transmute::<_, i32>(result) };
                if result_int != 0 {
                    let message = format!(
                        "errcode={} {}:{}:{}:{}",
                        result_int,
                        module_path!(),
                        file!(),
                        line!(),
                        column!()
                    );
                    if $allow_err {
                        log::warn!("{}", message);
                    } else {
                        return Err(anyhow::anyhow!(message));
                    }
                }
                result
            }};
        }
    };
}

#[macro_export]
macro_rules! generate_call_ptr_macro {
    ($func_name:ident) => {
        macro_rules! $func_name {
            ($x:expr) => {{
                let result = unsafe { $x };
                if result.is_null() {
                    let message = format!(
                        "null ptr {}:{}:{}:{}",
                        module_path!(),
                        file!(),
                        line!(),
                        column!()
                    );
                    return Err(anyhow::anyhow!(message));
                }
                result
            }};
        }
    };
}

#[repr(usize)]
#[derive(Debug, Copy, Clone)]
pub enum ImageFormat {
    Raw,
    ABGR,
    ARGB,
}

#[repr(C)]
#[derive(Clone)]
pub struct ImageRgb {
    pub raw: Vec<u8>,
    pub w: usize,
    pub h: usize,
    pub fmt: ImageFormat,
    pub align: usize,
}

impl ImageRgb {
    pub fn new(fmt: ImageFormat, align: usize) -> Self {
        Self {
            raw: Vec::new(),
            w: 0,
            h: 0,
            fmt,
            align,
        }
    }

    #[inline]
    pub fn fmt(&self) -> ImageFormat {
        self.fmt
    }

    #[inline]
    pub fn align(&self) -> usize {
        self.align
    }

    #[inline]
    pub fn set_align(&mut self, align: usize) {
        self.align = align;
    }
}

pub struct ImageTexture {
    pub texture: *mut c_void,
    pub w: usize,
    pub h: usize,
}

impl Default for ImageTexture {
    fn default() -> Self {
        Self {
            texture: std::ptr::null_mut(),
            w: 0,
            h: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AdapterDevice {
    pub device: *mut c_void,
    pub vendor_id: u32,
    pub luid: i64,
}

#[inline]
pub fn would_block_if_equal(old: &mut Vec<u8>, b: &[u8]) -> std::io::Result<()> {
    if b == &old[..] {
        return Err(std::io::ErrorKind::WouldBlock.into());
    }
    old.resize(b.len(), 0);
    old.copy_from_slice(b);
    Ok(())
}

pub trait TraitCapturer {
    #[cfg(not(any(target_os = "ios")))]
    fn frame<'a>(&'a mut self, timeout: std::time::Duration) -> std::io::Result<Frame<'a>>;

    #[cfg(target_os = "windows")]
    fn is_gdi(&self) -> bool;
    #[cfg(target_os = "windows")]
    fn set_gdi(&mut self) -> bool;
}

pub trait TraitPixelBuffer {
    fn data(&self) -> &[u8];

    fn width(&self) -> usize;

    fn height(&self) -> usize;

    fn stride(&self) -> Vec<usize>;

    fn pixfmt(&self) -> Pixfmt;
}

#[cfg(not(any(target_os = "ios")))]
pub enum Frame<'a> {
    PixelBuffer(PixelBuffer<'a>),
    Texture((*mut c_void, usize)),
}

#[cfg(not(any(target_os = "ios")))]
impl Frame<'_> {
    pub fn valid(&self) -> bool {
        match self {
            Frame::PixelBuffer(buffer) => !buffer.data().is_empty(),
            Frame::Texture((texture, _)) => !texture.is_null(),
        }
    }

    pub fn to<'a>(
        &'a self,
        yuvfmt: EncodeYuvFormat,
        yuv: &'a mut Vec<u8>,
        mid_data: &mut Vec<u8>,
    ) -> ResultType<EncodeInput<'a>> {
        match self {
            Frame::PixelBuffer(pixelbuffer) => {
                convert_to_yuv(pixelbuffer, yuvfmt, yuv, mid_data)?;
                Ok(EncodeInput::YUV(yuv))
            }
            Frame::Texture(texture) => Ok(EncodeInput::Texture(*texture)),
        }
    }
}

pub enum EncodeInput<'a> {
    YUV(&'a [u8]),
    Texture((*mut c_void, usize)),
}

impl<'a> EncodeInput<'a> {
    pub fn yuv(&self) -> ResultType<&'_ [u8]> {
        match self {
            Self::YUV(f) => Ok(f),
            _ => bail!("not pixel buffer frame"),
        }
    }

    pub fn texture(&self) -> ResultType<(*mut c_void, usize)> {
        match self {
            Self::Texture(f) => Ok(*f),
            _ => bail!("not texture frame"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Pixfmt {
    BGRA,
    RGBA,
    RGB565LE,
    I420,
    NV12,
    I444,
}

impl Pixfmt {
    pub fn bpp(&self) -> usize {
        match self {
            Pixfmt::BGRA | Pixfmt::RGBA => 32,
            Pixfmt::RGB565LE => 16,
            Pixfmt::I420 | Pixfmt::NV12 => 12,
            Pixfmt::I444 => 24,
        }
    }

    pub fn bytes_per_pixel(&self) -> usize {
        (self.bpp() + 7) / 8
    }
}

#[derive(Debug, Clone)]
pub struct EncodeYuvFormat {
    pub pixfmt: Pixfmt,
    pub w: usize,
    pub h: usize,
    pub stride: Vec<usize>,
    pub u: usize,
    pub v: usize,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy, Hash)]
pub enum CodecFormat {
    VP8,
    VP9,
    AV1,
    H264,
    H265,
    Unknown,
}

impl From<&VideoFrame> for CodecFormat {
    fn from(frame: &VideoFrame) -> Self {
        match &frame.union {
            Some(video_frame::Union::Vp8s(_)) => CodecFormat::VP8,
            Some(video_frame::Union::Vp9s(_)) => CodecFormat::VP9,
            Some(video_frame::Union::Av1s(_)) => CodecFormat::AV1,
            Some(video_frame::Union::H264s(_)) => CodecFormat::H264,
            Some(video_frame::Union::H265s(_)) => CodecFormat::H265,
            _ => CodecFormat::Unknown,
        }
    }
}

impl From<&video_frame::Union> for CodecFormat {
    fn from(union: &video_frame::Union) -> Self {
        match union {
            video_frame::Union::Vp8s(_) => CodecFormat::VP8,
            video_frame::Union::Vp9s(_) => CodecFormat::VP9,
            video_frame::Union::Av1s(_) => CodecFormat::AV1,
            video_frame::Union::H264s(_) => CodecFormat::H264,
            video_frame::Union::H265s(_) => CodecFormat::H265,
        }
    }
}
