use bytes::Bytes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chroma {
    I420,
    I444,
}

#[derive(Default, Clone)]
pub struct EncodedVideoFrame {
    pub data: Bytes,
    pub key: bool,
    pub pts: i64,
}

#[derive(Default, Clone)]
pub struct EncodedVideoFrames {
    pub frames: Vec<EncodedVideoFrame>,
}

pub mod video_frame {
    use super::EncodedVideoFrames;

    #[derive(Clone)]
    pub enum Union {
        Vp8s(EncodedVideoFrames),
        Vp9s(EncodedVideoFrames),
        H264s(EncodedVideoFrames),
        H265s(EncodedVideoFrames),
        Av1s(EncodedVideoFrames),
    }
}

#[derive(Default, Clone)]
pub struct VideoFrame {
    pub union: Option<video_frame::Union>,
    pub display: i32,
}

impl VideoFrame {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_vp8s(&mut self, frames: EncodedVideoFrames) {
        self.union = Some(video_frame::Union::Vp8s(frames));
    }

    pub fn set_vp9s(&mut self, frames: EncodedVideoFrames) {
        self.union = Some(video_frame::Union::Vp9s(frames));
    }

    pub fn set_h264s(&mut self, frames: EncodedVideoFrames) {
        self.union = Some(video_frame::Union::H264s(frames));
    }

    pub fn set_h265s(&mut self, frames: EncodedVideoFrames) {
        self.union = Some(video_frame::Union::H265s(frames));
    }

    pub fn set_av1s(&mut self, frames: EncodedVideoFrames) {
        self.union = Some(video_frame::Union::Av1s(frames));
    }
}
