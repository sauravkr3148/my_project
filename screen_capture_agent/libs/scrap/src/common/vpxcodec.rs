// https://github.com/astraw/vpx-encode
// https://github.com/astraw/env-libvpx-sys
// https://github.com/rust-av/vpx-rs/blob/master/src/decoder.rs
// https://github.com/chromium/chromium/blob/e7b24573bc2e06fed4749dd6b6abfce67f29052f/media/video/vpx_video_encoder.cc#L522

use anyhow::{anyhow, Context, Result};
use bytes::Bytes;
use log;

use crate::common::codec::{base_bitrate, codec_thread_num};
use crate::common::video::{EncodedVideoFrame, EncodedVideoFrames, VideoFrame};
use crate::common::{EncodeInput, EncodeYuvFormat, Pixfmt, STRIDE_ALIGN};

use super::vpx::{vp8e_enc_control_id::*, vpx_codec_err_t::*, *};
use crate::{generate_call_macro, generate_call_ptr_macro};
use std::os::raw::{c_int, c_uint};
use std::{ptr, slice};

generate_call_macro!(call_vpx, false);
generate_call_ptr_macro!(call_vpx_ptr);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum VpxVideoCodecId {
    VP8,
    VP9,
}

impl Default for VpxVideoCodecId {
    fn default() -> VpxVideoCodecId {
        // VP8 is default for real-time screen sharing (3-5x faster than VP9)
        // RustDesk uses VP8 for systems with <=4GB RAM
        VpxVideoCodecId::VP8
    }
}

pub struct VpxEncoder {
    ctx: vpx_codec_ctx_t,
    width: usize,
    height: usize,
    id: VpxVideoCodecId,
    i444: bool,
    yuvfmt: EncodeYuvFormat,
}

pub struct VpxDecoder {
    ctx: vpx_codec_ctx_t,
}

impl VpxEncoder {
    pub fn new(config: VpxEncoderConfig, i444: bool) -> Result<Self> {
        let codec = match config.codec {
            VpxVideoCodecId::VP8 => call_vpx_ptr!(vpx_codec_vp8_cx()),
            VpxVideoCodecId::VP9 => call_vpx_ptr!(vpx_codec_vp9_cx()),
        };
        let mut c = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };
        call_vpx!(vpx_codec_enc_config_default(codec, &mut c, 0));

        c.g_w = config.width;
        c.g_h = config.height;
        c.g_timebase.num = 1;
        c.g_timebase.den = 1000;
        c.rc_undershoot_pct = 95;
        c.rc_dropframe_thresh = 25;
        c.g_threads = codec_thread_num(64) as _;
        c.g_error_resilient = VPX_ERROR_RESILIENT_DEFAULT;
        c.rc_end_usage = vpx_rc_mode::VPX_CBR;
        if let Some(keyframe_interval) = config.keyframe_interval {
            c.kf_min_dist = 0;
            c.kf_max_dist = keyframe_interval as _;
        } else {
            c.kf_mode = vpx_kf_mode::VPX_KF_DISABLED;
        }

        let (q_min, q_max) = Self::calc_q_values(config.quality);
        c.rc_min_quantizer = q_min;
        c.rc_max_quantizer = q_max;
        c.rc_target_bitrate =
            Self::calc_bitrate(config.width as _, config.height as _, config.quality);
        c.g_profile = if i444 && config.codec == VpxVideoCodecId::VP9 {
            1
        } else {
            0
        };

        let mut ctx = Default::default();
        call_vpx!(vpx_codec_enc_init_ver(
            &mut ctx,
            codec,
            &c,
            0,
            VPX_ENCODER_ABI_VERSION as _,
        ));

        if config.codec == VpxVideoCodecId::VP9 {
            call_vpx!(vpx_codec_control_(&mut ctx, VP8E_SET_CPUUSED as _, 7,));
            call_vpx!(vpx_codec_control_(
                &mut ctx,
                VP9E_SET_ROW_MT as _,
                1 as c_int
            ));
            call_vpx!(vpx_codec_control_(
                &mut ctx,
                VP9E_SET_TILE_COLUMNS as _,
                4 as c_int
            ));
        } else {
            call_vpx!(vpx_codec_control_(&mut ctx, VP8E_SET_CPUUSED as _, 12,));
        }

        Ok(Self {
            ctx,
            width: config.width as _,
            height: config.height as _,
            id: config.codec,
            i444,
            yuvfmt: Self::get_yuvfmt(config.width, config.height, i444),
        })
    }

    pub fn encode_to_message(&mut self, input: EncodeInput, ms: i64) -> Result<VideoFrame> {
        let mut frames = Vec::new();
        for frame in self
            .encode(ms, input.yuv()?, STRIDE_ALIGN)
            .with_context(|| "Failed to encode")?
        {
            frames.push(VpxEncoder::create_frame(&frame));
        }
        for frame in self.flush().with_context(|| "Failed to flush")? {
            frames.push(VpxEncoder::create_frame(&frame));
        }

        if !frames.is_empty() {
            Ok(VpxEncoder::create_video_frame(self.id, frames))
        } else {
            Err(anyhow!("no valid frame"))
        }
    }

    pub fn yuvfmt(&self) -> EncodeYuvFormat {
        self.yuvfmt.clone()
    }

    pub fn set_quality(&mut self, ratio: f32) -> Result<()> {
        let mut c = unsafe { *self.ctx.config.enc.to_owned() };
        let (q_min, q_max) = Self::calc_q_values(ratio);
        c.rc_min_quantizer = q_min;
        c.rc_max_quantizer = q_max;
        c.rc_target_bitrate = Self::calc_bitrate(self.width as _, self.height as _, ratio);
        call_vpx!(vpx_codec_enc_config_set(&mut self.ctx, &c));
        Ok(())
    }

    pub fn bitrate(&self) -> u32 {
        let c = unsafe { *self.ctx.config.enc.to_owned() };
        c.rc_target_bitrate
    }

    pub fn codec_id(&self) -> VpxVideoCodecId {
        self.id
    }

    pub fn encode<'a>(
        &'a mut self,
        pts: i64,
        data: &[u8],
        stride_align: usize,
    ) -> Result<EncodeFrames<'a>> {
        let bpp = if self.i444 { 24 } else { 12 };
        if data.len() < self.width * self.height * bpp / 8 {
            return Err(anyhow!("len not enough"));
        }
        let fmt = if self.i444 {
            vpx_img_fmt::VPX_IMG_FMT_I444
        } else {
            vpx_img_fmt::VPX_IMG_FMT_I420
        };

        let mut image = Default::default();
        call_vpx_ptr!(vpx_img_wrap(
            &mut image,
            fmt,
            self.width as _,
            self.height as _,
            stride_align as _,
            data.as_ptr() as _,
        ));

        call_vpx!(vpx_codec_encode(
            &mut self.ctx,
            &image,
            pts as _,
            1,
            0,
            VPX_DL_REALTIME as _,
        ));

        Ok(EncodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }

    pub fn flush<'a>(&'a mut self) -> Result<EncodeFrames<'a>> {
        call_vpx!(vpx_codec_encode(
            &mut self.ctx,
            ptr::null(),
            -1,
            1,
            0,
            VPX_DL_REALTIME as _,
        ));

        Ok(EncodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }

    #[inline]
    fn create_video_frame(codec_id: VpxVideoCodecId, frames: Vec<EncodedVideoFrame>) -> VideoFrame {
        let mut vf = VideoFrame::new();
        let vpxs = EncodedVideoFrames { frames };
        match codec_id {
            VpxVideoCodecId::VP8 => vf.set_vp8s(vpxs),
            VpxVideoCodecId::VP9 => vf.set_vp9s(vpxs),
        }
        vf
    }

    #[inline]
    fn create_frame(frame: &EncodeFrame) -> EncodedVideoFrame {
        EncodedVideoFrame {
            data: Bytes::from(frame.data.to_vec()),
            key: frame.key,
            pts: frame.pts,
        }
    }

    fn calc_bitrate(width: u32, height: u32, ratio: f32) -> u32 {
        let bitrate = base_bitrate(width, height) as f32;
        (bitrate * ratio) as u32
    }

    #[inline]
    fn calc_q_values(ratio: f32) -> (u32, u32) {
        let b = (ratio * 100.0) as u32;
        let b = std::cmp::min(b, 200);
        let q_min1 = 36;
        let q_min2 = 0;
        let q_max1 = 56;
        let q_max2 = 37;

        let t = b as f32 / 200.0;

        let mut q_min: u32 = ((1.0 - t) * q_min1 as f32 + t * q_min2 as f32).round() as u32;
        let mut q_max = ((1.0 - t) * q_max1 as f32 + t * q_max2 as f32).round() as u32;

        q_min = q_min.clamp(q_min2, q_min1);
        q_max = q_max.clamp(q_max2, q_max1);

        (q_min, q_max)
    }

    fn get_yuvfmt(width: u32, height: u32, i444: bool) -> EncodeYuvFormat {
        let mut img = Default::default();
        let fmt = if i444 {
            vpx_img_fmt::VPX_IMG_FMT_I444
        } else {
            vpx_img_fmt::VPX_IMG_FMT_I420
        };
        unsafe {
            vpx_img_wrap(
                &mut img,
                fmt,
                width as _,
                height as _,
                STRIDE_ALIGN as _,
                0x1 as _,
            );
        }
        let pixfmt = if i444 { Pixfmt::I444 } else { Pixfmt::I420 };
        EncodeYuvFormat {
            pixfmt,
            w: img.w as _,
            h: img.h as _,
            stride: img.stride.iter().map(|s| *s as usize).collect(),
            u: img.planes[1] as usize - img.planes[0] as usize,
            v: img.planes[2] as usize - img.planes[0] as usize,
        }
    }
}

impl Drop for VpxEncoder {
    fn drop(&mut self) {
        unsafe {
            let result = vpx_codec_destroy(&mut self.ctx);
            if result != VPX_CODEC_OK {
                panic!("failed to destroy vpx codec");
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EncodeFrame<'a> {
    /// Compressed data.
    pub data: &'a [u8],
    /// Whether the frame is a keyframe.
    pub key: bool,
    /// Presentation timestamp (in timebase units).
    pub pts: i64,
}

#[derive(Clone, Copy, Debug)]
pub struct VpxEncoderConfig {
    /// The width (in pixels).
    pub width: c_uint,
    /// The height (in pixels).
    pub height: c_uint,
    /// The bitrate ratio
    pub quality: f32,
    /// The codec
    pub codec: VpxVideoCodecId,
    /// keyframe interval
    pub keyframe_interval: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub struct VpxDecoderConfig {
    pub codec: VpxVideoCodecId,
}

pub struct EncodeFrames<'a> {
    ctx: &'a mut vpx_codec_ctx_t,
    iter: vpx_codec_iter_t,
}

impl<'a> Iterator for EncodeFrames<'a> {
    type Item = EncodeFrame<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            unsafe {
                let pkt = vpx_codec_get_cx_data(self.ctx, &mut self.iter);
                if pkt.is_null() {
                    return None;
                } else if (*pkt).kind == vpx_codec_cx_pkt_kind::VPX_CODEC_CX_FRAME_PKT {
                    let f = &(*pkt).data.frame;
                    return Some(Self::Item {
                        data: slice::from_raw_parts(f.buf as _, f.sz as _),
                        key: (f.flags & VPX_FRAME_IS_KEY) != 0,
                        pts: f.pts,
                    });
                } else {
                    // Ignore the packet.
                }
            }
        }
    }
}

impl VpxDecoder {
    /// Create a new decoder
    ///
    /// # Errors
    ///
    /// The function may fail if the underlying libvpx does not provide
    /// the VP9 decoder.
    pub fn new(config: VpxDecoderConfig) -> Result<Self> {
        // This is sound because `vpx_codec_ctx` is a repr(C) struct without any field that can
        // cause UB if uninitialized.
        let i = match config.codec {
            VpxVideoCodecId::VP8 => call_vpx_ptr!(vpx_codec_vp8_dx()),
            VpxVideoCodecId::VP9 => call_vpx_ptr!(vpx_codec_vp9_dx()),
        };
        let mut ctx = Default::default();
        let cfg = vpx_codec_dec_cfg_t {
            threads: codec_thread_num(64) as _,
            w: 0,
            h: 0,
        };
        /*
        unsafe {
            println!("{}", vpx_codec_get_caps(i));
        }
        */
        call_vpx!(vpx_codec_dec_init_ver(
            &mut ctx,
            i,
            &cfg,
            0,
            VPX_DECODER_ABI_VERSION as _,
        ));
        Ok(Self { ctx })
    }

    /// Feed some compressed data to the encoder
    ///
    /// The `data` slice is sent to the decoder
    ///
    /// It matches a call to `vpx_codec_decode`.
    pub fn decode(&mut self, data: &[u8]) -> Result<DecodeFrames> {
        call_vpx!(vpx_codec_decode(
            &mut self.ctx,
            data.as_ptr(),
            data.len() as _,
            ptr::null_mut(),
            0,
        ));

        Ok(DecodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }

    /// Notify the decoder to return any pending frame
    pub fn flush(&mut self) -> Result<DecodeFrames> {
        call_vpx!(vpx_codec_decode(
            &mut self.ctx,
            ptr::null(),
            0,
            ptr::null_mut(),
            0
        ));
        Ok(DecodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }
}

impl Drop for VpxDecoder {
    fn drop(&mut self) {
        unsafe {
            let result = vpx_codec_destroy(&mut self.ctx);
            if result != VPX_CODEC_OK {
                panic!("failed to destroy vpx codec");
            }
        }
    }
}

pub struct DecodeFrames<'a> {
    ctx: &'a mut vpx_codec_ctx_t,
    iter: vpx_codec_iter_t,
}

impl<'a> Iterator for DecodeFrames<'a> {
    type Item = Image;
    fn next(&mut self) -> Option<Self::Item> {
        let img = unsafe { vpx_codec_get_frame(self.ctx, &mut self.iter) };
        if img.is_null() {
            return None;
        } else {
            return Some(Image(img));
        }
    }
}

// https://chromium.googlesource.com/webm/libvpx/+/bali/vpx/src/vpx_image.c
pub struct Image(*mut vpx_image_t);
impl Image {
    #[inline]
    pub fn new() -> Self {
        Self(std::ptr::null_mut())
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    #[inline]
    pub fn format(&self) -> vpx_img_fmt_t {
        // VPX_IMG_FMT_I420
        self.inner().fmt
    }

    #[inline]
    pub fn inner(&self) -> &vpx_image_t {
        unsafe { &*self.0 }
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { vpx_img_free(self.0) };
        }
    }
}

unsafe impl Send for vpx_codec_ctx_t {}
