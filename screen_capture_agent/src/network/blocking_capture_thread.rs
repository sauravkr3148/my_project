use crate::qos::VideoQoS;
use crate::video_encoder::{EnhancedVideoEncoder, VideoEncoderConfig};
use scrap::CodecFormat;
use scrap::{Capturer, Display, TraitCapturer, TraitPixelBuffer};
use std::io::ErrorKind;
use std::sync::mpsc::{sync_channel, Receiver};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};

const FRAME_CHANNEL_SIZE: usize = 30;
// RustDesk exact: portable_service.rs line 51
const MAX_DXGI_FAIL_TIME: usize = 5;

pub struct FrameData {
    pub data: Vec<u8>,
    pub timestamp: i64,
    pub is_keyframe: bool,
}

pub struct CaptureThreadHandle {
    thread_handle: Option<thread::JoinHandle<()>>,
    running: Arc<AtomicBool>,
    force_keyframe: Arc<AtomicBool>,
    active_clients: Arc<AtomicUsize>,
}

impl CaptureThreadHandle {
    pub fn spawn(
        codec: CodecFormat,
        video_qos: Arc<Mutex<VideoQoS>>,
    ) -> (Self, Receiver<FrameData>) {
        let (frame_tx, frame_rx) = sync_channel::<FrameData>(FRAME_CHANNEL_SIZE);
        let running = Arc::new(AtomicBool::new(true));
        let force_keyframe = Arc::new(AtomicBool::new(false));
        let active_clients = Arc::new(AtomicUsize::new(0));

        let running_clone = running.clone();
        let force_keyframe_clone = force_keyframe.clone();
        let active_clients_clone = active_clients.clone();
        let video_qos_clone = video_qos.clone();

        let thread_handle = thread::spawn(move || {
            log::debug!(" RustDesk-style BLOCKING capture thread started (no async)");

            let display = match Display::primary() {
                Ok(d) => d,
                Err(e) => {
                    log::error!(" Failed to get primary display: {}", e);
                    return;
                }
            };

            let mut capturer = match Capturer::new(display) {
                Ok(mut c) => {
                    log::info!(" DXGI Capturer initialized in blocking thread");
                    c
                }
                Err(e) => {
                    log::error!(" Failed to create capturer: {}", e);
                    return;
                }
            };

            // RustDesk exact: portable_service.rs - track DXGI failures for GDI fallback
            let mut dxgi_failed_times = 0usize;

            let width = capturer.width();
            let height = capturer.height();

            // RustDesk approach: VP8/VP9 encoders require EVEN dimensions (divisible by 2)
            // Round down to nearest even number
            let adjusted_width = width & !1; // Clear last bit to make even
            let adjusted_height = height & !1; // Clear last bit to make even

            log::info!(
                " Capture resolution: {}x{} (adjusted to {}x{} for encoder)",
                width,
                height,
                adjusted_width,
                adjusted_height
            );

            // CRITICAL FIX: Windows DXGI needs initialization delay in release mode
            // Due to compiler optimizations, DXGI compositor needs time to set up
            log::info!(" Initializing DXGI (Windows requires delay in release mode)...");

            // STEP 1: Give DXGI compositor time to initialize (critical for release mode)
            log::info!(" Allowing DXGI compositor to initialize...");
            thread::sleep(Duration::from_millis(2000)); // 2 seconds for Windows 10/11 in release mode

            // STEP 2: Now try to get first frame
            log::info!(" Attempting first DXGI frame capture...");
            let mut dxgi_ready = false;
            for attempt in 0..100 {
                match capturer.frame(Duration::from_millis(100)) {
                    // 100ms timeout per attempt (faster retries)
                    Ok(frame) if frame.valid() => {
                        log::info!(" DXGI ready after {} attempts!", attempt + 1);
                        dxgi_ready = true;
                        break;
                    }
                    Ok(_) => {
                        // Got frame but not valid, try again
                        if attempt % 10 == 9 {
                            log::info!("⏳ Waiting for valid frame... attempt {}/100", attempt + 1);
                        }
                        thread::sleep(Duration::from_millis(50)); // Shorter sleep, faster retries
                    }
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                        if attempt % 10 == 9 {
                            log::info!("⏳ DXGI initializing... attempt {}/100", attempt + 1);
                        }
                        thread::sleep(Duration::from_millis(50)); // Shorter sleep between attempts for faster convergence
                    }
                    Err(e) => {
                        log::error!(" DXGI error: {}, continuing anyway", e);
                        break;
                    }
                }
            }

            if dxgi_ready {
                log::info!(" DXGI successfully initialized!");
                thread::sleep(Duration::from_millis(500)); // Longer stabilization period for release mode
            } else {
                log::error!(
                    " DXGI failed to initialize after 100 attempts - frames may not capture!"
                );
                log::error!(
                    " This is a known Windows + release mode issue with certain GPU drivers."
                );
                log::error!(" Workarounds: 1) Use debug mode, 2) Update GPU drivers, 3) Run as administrator");
                log::error!(
                    " Continuing anyway - will retry frame capture with re-encoding last frame..."
                );
            }

            let initial_quality = {
                let mut qos = video_qos_clone.lock().unwrap();
                qos.ratio()
            };

            let config = VideoEncoderConfig::new(adjusted_width, adjusted_height)
                .with_codec(codec)
                .with_quality(initial_quality)
                .with_fps(30);

            let mut encoder = match EnhancedVideoEncoder::new(config) {
                Ok(e) => e,
                Err(e) => {
                    log::error!(" Failed to create encoder: {}", e);
                    return;
                }
            };

            let (encoder_width, encoder_height) = encoder.target_dimensions();
            let encoder_expected_stride = encoder.expected_stride();
            log::info!(
                "Encoder ready: target={}x{}, codec={:?}, dummy_mode={}",
                encoder_width,
                encoder_height,
                encoder.codec_format(),
                encoder.is_dummy()
            );
            if encoder_width != adjusted_width || encoder_height != adjusted_height {
                log::warn!(
                    "Encoder dimensions {}x{} differ from even-aligned capture {}x{} - frame data will be adjusted",
                    encoder_width,
                    encoder_height,
                    adjusted_width,
                    adjusted_height
                );
            }

            let target_width = encoder_width;
            let target_height = encoder_height;
            let mut stride_notice_logged = false;
            let mut encode_success_log_count: usize = 0;
            let mut encode_none_log_count: usize = 0;
            const MAX_ENCODE_SUCCESS_LOGS: usize = 6;
            const MAX_ENCODE_NONE_LOGS: usize = 10;

            let encoder_yuv_format = match encoder.yuv_format() {
                Some(fmt) => fmt,
                None => {
                    log::error!("Failed to get encoder YUV format");
                    return;
                }
            };
            let mut yuv_buffer: Vec<u8> = Vec::new();
            let mut mid_data: Vec<u8> = Vec::new();

            let debug_force_capture_frames: u64 = std::env::var("AGENT_FORCE_CAPTURE_FRAMES")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            if debug_force_capture_frames > 0 {
                log::info!(
                    "Debug mode: forcing capture of {} frames even without active clients",
                    debug_force_capture_frames
                );
            }
            let mut debug_frames_remaining = debug_force_capture_frames;
            let mut debug_force_notice_logged = false;

            let mut spf = {
                let mut qos = video_qos_clone.lock().unwrap();
                qos.spf()
            };
            let mut quality = {
                let mut qos = video_qos_clone.lock().unwrap();
                qos.ratio()
            };
            let mut frame_count = 0u64;
            let mut last_stats = Instant::now();
            let mut second_instant = Instant::now();
            let mut send_counter = 0usize;
            let mut frames_sent = 0u64;
            let mut frames_dropped = 0u64;
            let mut would_block_count = 0u32;
            let video_service_name = "blocking_capture".to_string();
            let start = Instant::now();

            {
                let mut qos = video_qos_clone.lock().unwrap();
                qos.new_display(video_service_name.clone());
                qos.set_support_changing_quality(
                    &video_service_name,
                    encoder.support_changing_quality(),
                );
                qos.store_bitrate(encoder.bitrate());
            }

            let mut consecutive_would_block = 0u32;
            while running_clone.load(Ordering::Relaxed) {
                let loop_start = Instant::now();
                let clients = active_clients_clone.load(Ordering::Relaxed);
                let has_active_clients = clients > 0;

                if !has_active_clients {
                    if debug_frames_remaining > 0 {
                        if !debug_force_notice_logged {
                            log::info!(
                                "Debug mode: continuing capture without clients ({} frames remaining)",
                                debug_frames_remaining
                            );
                            debug_force_notice_logged = true;
                        }
                        debug_frames_remaining = debug_frames_remaining.saturating_sub(1);
                    } else {
                        thread::sleep(Duration::from_millis(100));
                        consecutive_would_block = 0;
                        continue;
                    }
                }

                {
                    let mut video_qos = video_qos_clone.lock().unwrap();
                    spf = video_qos.spf();
                    if quality != video_qos.ratio() {
                        quality = video_qos.ratio();
                        if encoder.support_changing_quality() {
                            if let Err(_) = encoder.set_quality(quality) {
                                log::debug!("Failed to set encoder quality");
                            }
                            video_qos.store_bitrate(encoder.bitrate());
                        }
                    }
                    if second_instant.elapsed() > Duration::from_secs(1) {
                        second_instant = Instant::now();
                        video_qos.update_display_data(&video_service_name, send_counter);
                        send_counter = 0;
                    }
                }

                if force_keyframe_clone.swap(false, Ordering::Relaxed) {
                    encoder.force_keyframe();
                }

                let time = loop_start.duration_since(start);
                let ms = (time.as_secs() * 1000 + time.subsec_millis() as u64) as i64;
                let res = capturer.frame(spf);

                match res {
                    Ok(frame) => {
                        would_block_count = 0;
                        consecutive_would_block = 0;
                        // RustDesk exact: portable_service.rs line 401 - reset on success
                        dxgi_failed_times = 0;

                        if !frame.valid() {
                            continue;
                        }

                        if !frame.valid() {
                            continue;
                        }

                        let stride = match &frame {
                            scrap::Frame::PixelBuffer(pixbuf) => {
                                let stride_vec = pixbuf.stride();
                                let original_stride = if stride_vec.is_empty() {
                                    width * 4
                                } else {
                                    stride_vec[0]
                                };

                                if !stride_notice_logged {
                                    log::info!(
                                        "Capture stride {} (encoder expected stride {})",
                                        original_stride,
                                        encoder_expected_stride
                                    );
                                    stride_notice_logged = true;
                                }
                                original_stride
                            }
                            _ => {
                                continue;
                            }
                        };

                        if encode_success_log_count == 0 {
                            log::info!(
                                "Encoding frame start: stride={}, expected_stride={}",
                                stride,
                                encoder_expected_stride,
                            );
                        }

                        let encode_input = match frame.to(
                            encoder_yuv_format.clone(),
                            &mut yuv_buffer,
                            &mut mid_data,
                        ) {
                            Ok(input) => input,
                            Err(e) => {
                                log::warn!(
                                    "Failed to convert frame to YUV ({}x{}): {}",
                                    width,
                                    height,
                                    e
                                );
                                continue;
                            }
                        };

                        match encoder.encode_input(encode_input, ms) {
                            Ok(Some(encoded_data)) => {
                                frame_count += 1;
                                let is_keyframe = encoder.was_last_frame_keyframe();

                                if encode_success_log_count < MAX_ENCODE_SUCCESS_LOGS {
                                    log::info!(
                                        "Encoded frame {} (timestamp {}) -> payload {} bytes, keyframe={}, stride={}",
                                        frame_count,
                                        ms,
                                        encoded_data.len(),
                                        is_keyframe,
                                        stride
                                    );
                                    encode_success_log_count += 1;
                                }

                                if has_active_clients {
                                    let frame_data = FrameData {
                                        data: encoded_data,
                                        timestamp: ms,
                                        is_keyframe,
                                    };

                                    match frame_tx.try_send(frame_data) {
                                        Ok(_) => {
                                            frames_sent += 1;
                                            send_counter += 1;
                                        }
                                        Err(_) => {
                                            frames_dropped += 1;
                                            if frames_dropped % 10 == 1 {
                                                log::warn!(
                                                    " Dropped {} frames (client too slow)",
                                                    frames_dropped
                                                );
                                            }
                                            {
                                                let mut qos = video_qos_clone.lock().unwrap();
                                                qos.update_display_data(&video_service_name, 0);
                                            }
                                        }
                                    }
                                } else {
                                    log::info!(
                                        "Debug mode: encoded frame {} ({} bytes) discarded because there are no active clients",
                                        frame_count,
                                        encoded_data.len()
                                    );
                                }
                            }
                            Ok(None) => {
                                if encode_none_log_count == 0 {
                                    log::info!(
                                        "Encoder produced no payload (frame_count={}, encoder frame count={}, timestamp={}, stride={}, expected_stride={}). This is normal when the frame has no changes.",
                                        frame_count,
                                        encoder.frame_count(),
                                        ms,
                                        stride,
                                        encoder_expected_stride
                                    );
                                } else if encode_none_log_count < MAX_ENCODE_NONE_LOGS {
                                    log::debug!(
                                        "Encoder produced no payload (encoder frame count={}, timestamp={}, stride={}, expected_stride={})",
                                        encoder.frame_count(),
                                        ms,
                                        stride,
                                        encoder_expected_stride
                                    );
                                }
                                encode_none_log_count += 1;
                                continue;
                            }
                            Err(e) => {
                                log::error!(
                                    "Encode error after {} frames (encoder frame count={}, timestamp={}, stride={}, expected_stride={}, data_len={}): {}",
                                    frame_count,
                                    encoder.frame_count(),
                                    ms,
                                    stride,
                                    encoder_expected_stride,
                                    yuv_buffer.len(),
                                    e
                                );
                            }
                        }
                    }
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                        would_block_count += 1;
                        consecutive_would_block += 1;

                        // RustDesk recovery: Try to re-encode last frame for a few attempts, then skip
                        // RustDesk recovery: After many consecutive failures, try shorter timeout
                        // This helps recover from temporary DXGI issues
                        if consecutive_would_block > 50 && consecutive_would_block % 10 == 0 {
                            // Try with very short timeout to quickly recover
                            thread::sleep(Duration::from_millis(1));
                        }

                        // Log less frequently to avoid spam
                        if consecutive_would_block == 100 {
                            log::error!(" DXGI not capturing frames after 100 attempts!");
                            log::error!(" This is a Windows 8 + release mode bug.");
                            log::error!(" Recommended: Use debug mode or upgrade to Windows 10+");
                        } else if consecutive_would_block % 1000 == 0 && consecutive_would_block > 0
                        {
                            log::error!(
                                " DXGI still failing ({} WouldBlock errors)",
                                consecutive_would_block
                            );
                        }
                    }
                    Err(e) => {
                        // RustDesk exact: portable_service.rs lines 407-425
                        // Handle non-WouldBlock errors with GDI fallback
                        if e.kind() != ErrorKind::WouldBlock {
                            // DXGI_ERROR_INVALID_CALL after each success on Microsoft GPU driver
                            if !capturer.is_gdi() {
                                // not gdi
                                dxgi_failed_times += 1;
                            }
                            if dxgi_failed_times > MAX_DXGI_FAIL_TIME {
                                // Recreate capturer with GDI fallback
                                log::info!(
                                    " DXGI failed {} times, falling back to GDI",
                                    dxgi_failed_times
                                );
                                let display = match Display::primary() {
                                    Ok(d) => d,
                                    Err(err) => {
                                        log::error!(
                                            " Failed to get primary display for GDI fallback: {}",
                                            err
                                        );
                                        thread::sleep(spf);
                                        continue;
                                    }
                                };
                                match Capturer::new(display) {
                                    Ok(mut v) => {
                                        dxgi_failed_times = 0;
                                        v.set_gdi();
                                        capturer = v;
                                        log::info!(" Successfully switched to GDI capturer");
                                    }
                                    Err(err) => {
                                        log::error!(" Failed to create GDI capturer: {:?}", err);
                                        thread::sleep(spf);
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                }

                if last_stats.elapsed().as_secs() >= 5 {
                    let fps = frames_sent as f32 / last_stats.elapsed().as_secs_f32();
                    log::info!(
                        " BLOCKING THREAD: {:.1} FPS, {} sent, {} dropped",
                        fps,
                        frames_sent,
                        frames_dropped
                    );
                    frames_sent = 0;
                    frames_dropped = 0;
                    would_block_count = 0;
                    last_stats = Instant::now();
                }

                let elapsed = loop_start.elapsed();
                if elapsed < spf {
                    thread::sleep(spf - elapsed);
                }
            }

            log::info!(" Blocking capture thread exited cleanly");
        });

        (
            Self {
                thread_handle: Some(thread_handle),
                running,
                force_keyframe,
                active_clients,
            },
            frame_rx,
        )
    }

    pub fn set_force_keyframe(&self) {
        self.force_keyframe.store(true, Ordering::Relaxed);
    }

    pub fn set_active_clients(&self, count: usize) {
        self.active_clients.store(count, Ordering::Relaxed);
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for CaptureThreadHandle {
    fn drop(&mut self) {
        self.stop();
    }
}
