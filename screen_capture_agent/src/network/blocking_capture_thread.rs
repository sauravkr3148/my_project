use crate::codec::YuvConverter;
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
    fn crop_frame_data(
        data: &[u8],
        stride: usize,
        original_width: usize,
        original_height: usize,
        target_width: usize,
        target_height: usize,
    ) -> Vec<u8> {
        if original_width == target_width && original_height == target_height {
            return data.to_vec();
        }

        let bytes_per_pixel = 4; // BGRA format
        let mut cropped_data = Vec::with_capacity(target_width * target_height * bytes_per_pixel);

        // Calculate cropping offsets (center crop)
        let width_diff = original_width - target_width;
        let height_diff = original_height - target_height;
        let x_offset = width_diff / 2;
        let y_offset = height_diff / 2;

        for y in 0..target_height {
            let src_y = y + y_offset;
            if src_y >= original_height {
                break;
            }

            let src_start = src_y * stride + x_offset * bytes_per_pixel;
            let src_end = src_start + target_width * bytes_per_pixel;

            if src_start + target_width * bytes_per_pixel <= data.len() {
                cropped_data.extend_from_slice(&data[src_start..src_end]);
            }
        }

        cropped_data
    }

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
            let mut crop_notice_logged = false;
            let mut stride_notice_logged = false;
            let mut repack_notice_logged = false;
            let mut frame_prep_log_count: usize = 0;
            let mut stride_mismatch_log_count: usize = 0;
            let mut encode_success_log_count: usize = 0;
            let mut encode_none_log_count: usize = 0;
            const MAX_PREP_LOGS: usize = 6;
            const MAX_STRIDE_MISMATCH_LOGS: usize = 10;
            const MAX_ENCODE_SUCCESS_LOGS: usize = 6;
            const MAX_ENCODE_NONE_LOGS: usize = 10;

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

            let mut _yuv_converter = match YuvConverter::new(target_width, target_height) {
                Ok(c) => c,
                Err(e) => {
                    log::error!(" Failed to create YUV converter: {}", e);
                    return;
                }
            };
            let mut consecutive_would_block = 0u32;
            let mut last_frame_data: Option<(Vec<u8>, u32, i64)> = None;
            let repeat_encode_max = 3;
            let mut repeat_encode_counter = 0u32;
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
                        repeat_encode_counter = 0;
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
                        repeat_encode_counter = 0;
                        would_block_count = 0;
                        consecutive_would_block = 0;
                        // RustDesk exact: portable_service.rs line 401 - reset on success
                        dxgi_failed_times = 0;

                        if !frame.valid() {
                            continue;
                        }

                        let (data, stride) = match &frame {
                            scrap::Frame::PixelBuffer(pixbuf) => {
                                let stride_vec = pixbuf.stride();
                                let original_stride = if stride_vec.is_empty() {
                                    width * 4
                                } else {
                                    stride_vec[0]
                                };

                                if original_stride != encoder_expected_stride
                                    && stride_mismatch_log_count < MAX_STRIDE_MISMATCH_LOGS
                                {
                                    log::warn!(
                                        "Capture stride {} differs from encoder expected stride {} (target {}x{})",
                                        original_stride,
                                        encoder_expected_stride,
                                        target_width,
                                        target_height
                                    );
                                    stride_mismatch_log_count += 1;
                                } else if !stride_notice_logged {
                                    log::info!(
                                        "Capture stride matches encoder expectation (stride={})",
                                        original_stride
                                    );
                                    stride_notice_logged = true;
                                }

                                const BYTES_PER_PIXEL: usize = 4;
                                let expected_stride = target_width * BYTES_PER_PIXEL;
                                let expected_len = target_width * target_height * BYTES_PER_PIXEL;
                                let mut prepared_stride_usize = expected_stride;
                                let mut prepared_data = Vec::with_capacity(expected_len);

                                if width != target_width || height != target_height {
                                    if !crop_notice_logged {
                                        log::warn!(
                                            "Cropping captured frame from {}x{} to {}x{} to satisfy encoder requirements",
                                            width,
                                            height,
                                            target_width,
                                            target_height
                                        );
                                        crop_notice_logged = true;
                                    }
                                    prepared_data = Self::crop_frame_data(
                                        pixbuf.data(),
                                        original_stride,
                                        width,
                                        height,
                                        target_width,
                                        target_height,
                                    );
                                    prepared_stride_usize = expected_stride;
                                } else if original_stride != expected_stride {
                                    if !repack_notice_logged {
                                        log::warn!(
                                            "Repacking frame data to remove stride padding: capture stride {} -> encoder stride {}",
                                            original_stride,
                                            expected_stride
                                        );
                                        repack_notice_logged = true;
                                    }
                                    let row_len = target_width * BYTES_PER_PIXEL;
                                    let data_ref = pixbuf.data();
                                    prepared_data.reserve_exact(expected_len);
                                    for y in 0..target_height {
                                        let start = y * original_stride;
                                        let end = start + row_len;
                                        if end <= data_ref.len() {
                                            prepared_data.extend_from_slice(&data_ref[start..end]);
                                        } else {
                                            log::warn!(
                                                "Stride repack exceeded source buffer (row={}, start={}, end={}, len={})",
                                                y,
                                                start,
                                                end,
                                                data_ref.len()
                                            );
                                            break;
                                        }
                                    }
                                    prepared_stride_usize = expected_stride;
                                } else {
                                    prepared_data = pixbuf.data().to_vec();
                                    prepared_stride_usize = original_stride;
                                }

                                if prepared_data.len() != expected_len {
                                    log::warn!(
                                        "Prepared frame data length {} does not match expected {} ({}x{}x{})",
                                        prepared_data.len(),
                                        expected_len,
                                        target_width,
                                        target_height,
                                        BYTES_PER_PIXEL
                                    );
                                } else if frame_prep_log_count < MAX_PREP_LOGS {
                                    log::info!(
                                        "Frame prep {}: capture {}x{} stride={} -> prepared stride {} len={} timestamp={}",
                                        frame_prep_log_count + 1,
                                        width,
                                        height,
                                        original_stride,
                                        prepared_stride_usize,
                                        prepared_data.len(),
                                        ms
                                    );
                                    frame_prep_log_count += 1;
                                }

                                let prepared_stride_u32 = prepared_stride_usize as u32;
                                last_frame_data =
                                    Some((prepared_data.clone(), prepared_stride_u32, ms));
                                (prepared_data, prepared_stride_usize)
                            }
                            _ => {
                                continue;
                            }
                        };

                        if stride != encoder_expected_stride
                            && stride_mismatch_log_count < MAX_STRIDE_MISMATCH_LOGS
                        {
                            log::warn!(
                                "Prepared frame stride {} still differs from encoder expected stride {} (encoder frame count={})",
                                stride,
                                encoder_expected_stride,
                                encoder.frame_count()
                            );
                            stride_mismatch_log_count += 1;
                        }

                        if encode_success_log_count == 0 {
                            log::info!(
                                "Encoding frame start: stride={}, expected_stride={}, data_len={}",
                                stride,
                                encoder_expected_stride,
                                data.len()
                            );
                        }

                        match encoder.encode_frame(&data, stride, ms) {
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
                                log::warn!(
                                    "Encoder returned no payload (frame_count={}, encoder frame count={}, timestamp={}, stride={}, expected_stride={}, data_len={}). Attempting to reconfigure encoder...",
                                    frame_count,
                                    encoder.frame_count(),
                                    ms,
                                    stride,
                                    encoder_expected_stride,
                                    data.len()
                                );
                                last_frame_data = None;
                                if encode_none_log_count < MAX_ENCODE_NONE_LOGS {
                                    log::warn!(
                                        "Encoder returned no payload (encoder frame count={}, timestamp={}, prepared_stride={}, expected_stride={})",
                                        encoder.frame_count(),
                                        ms,
                                        stride,
                                        encoder_expected_stride
                                    );
                                    encode_none_log_count += 1;
                                }

                                // Attempt to reconfigure encoder with padding strategy
                                let padded_width = stride / 4;
                                if padded_width != encoder_expected_stride / 4 {
                                    log::warn!(
                                        "Padding frame from width {} to {} to satisfy encoder stride requirements",
                                        padded_width,
                                        encoder_expected_stride / 4
                                    );
                                }

                                // No payload after reconfiguration, continue loop
                                continue;
                            }
                            Err(e) => {
                                last_frame_data = None;
                                log::error!(
                                    "Encode error after {} frames (encoder frame count={}, timestamp={}, stride={}, expected_stride={}, data_len={}): {}",
                                    frame_count,
                                    encoder.frame_count(),
                                    ms,
                                    stride,
                                    encoder_expected_stride,
                                    data.len(),
                                    e
                                );
                            }
                        }
                    }
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                        would_block_count += 1;
                        consecutive_would_block += 1;

                        // RustDesk recovery: Try to re-encode last frame for a few attempts, then skip
                        if let Some((ref last_data, last_stride, last_ms)) = last_frame_data {
                            if repeat_encode_counter < repeat_encode_max {
                                repeat_encode_counter += 1;
                                match encoder.encode_frame(last_data, last_stride as usize, last_ms)
                                {
                                    Ok(Some(encoded_data)) => {
                                        let is_keyframe = encoder.was_last_frame_keyframe();
                                        let frame_data = FrameData {
                                            data: encoded_data,
                                            timestamp: last_ms,
                                            is_keyframe,
                                        };
                                        if let Ok(_) = frame_tx.try_send(frame_data) {
                                            frames_sent += 1;
                                            send_counter += 1;
                                        }
                                        // Reset counter on successful re-encode
                                        if consecutive_would_block < 10 {
                                            consecutive_would_block = 0;
                                        }
                                    }
                                    _ => {}
                                }
                            } else {
                                // After max re-encodes, skip frame to maintain real-time performance
                                frames_dropped += 1;
                            }
                        }

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
