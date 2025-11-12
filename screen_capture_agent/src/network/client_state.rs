use crate::network::websocket::should_shutdown;
use scrap::dxgi::gdi::ClientStateProvider;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
#[derive(Debug, Clone)]
pub enum InputEvent {
    Unicode { data: Vec<u8> },
    Mouse { data: Vec<u8>, x: i32, y: i32 },
    MouseMove { x: i32, y: i32 },
    CursorUpdate { cursor_type: u8 },
    Keyboard { data: Vec<u8> },
    CtrlAltDel,
}
#[derive(Debug, Clone)]
pub struct DesktopContext {
    pub desktop_name_hash: u32,
    pub monitor_count: i32,
    pub virtual_screen: (i32, i32, i32, i32),
}
#[derive(Debug, Clone)]
pub struct CursorState {
    pub visible: bool,
    pub x: i32,
    pub y: i32,
    pub cursor_type: u8,
    pub sent_hide_cursor: bool,
    pub remote_mouse_moved: bool,
}
impl Default for CursorState {
    fn default() -> Self {
        Self {
            visible: false,
            x: 0,
            y: 0,
            cursor_type: 0,
            sent_hide_cursor: false,
            remote_mouse_moved: false,
        }
    }
}
pub struct ClientState {
    remote_connected: AtomicBool,
    shutdown_requested: AtomicBool,
    paused: AtomicBool,
    target_fps: AtomicU32,
    frame_count: AtomicU64,
    last_frame_time: RwLock<Instant>,
    error_count: AtomicU32,
    restricted_page_count: AtomicU32,
    restart_count: AtomicU32,
    pending_inputs: Mutex<VecDeque<InputEvent>>,
    desktop_switch_detected: AtomicBool,
    explorer_running: AtomicBool,
    current_desktop_name: Mutex<String>,
    full_refresh_requested: AtomicBool,
    tile_reset_requested: AtomicBool,
    pub resolution_changing: AtomicBool,
    active_client_count: AtomicU32,
    last_client_connection_time: RwLock<Instant>,
    clients_needing_initial_frame: AtomicU32,
    initial_connection_phase: AtomicBool,
    connection_start_time: RwLock<Instant>,
    last_full_refresh_clear: RwLock<Instant>,
    connection_active: AtomicBool,
    disconnect_requested: AtomicBool,
    pub cursor_state: Arc<Mutex<CursorState>>,
    pub desktop_context: Arc<Mutex<Option<DesktopContext>>>,
    restricted_screen_active: AtomicBool,
    session_locked: AtomicBool,
    compression_quality: AtomicU32,
    frame_rate_timer: AtomicU32,
    scaling_factor: AtomicU32,
    selected_display: AtomicU32,
    force_keyframe: AtomicBool,
    clients_needing_keyframe: AtomicU32,
    last_keyframe_time: RwLock<Instant>,
}
impl ClientState {
    pub fn new() -> Self {
        Self {
            remote_connected: AtomicBool::new(false),
            shutdown_requested: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            target_fps: AtomicU32::new(30),
            frame_count: AtomicU64::new(0),
            last_frame_time: RwLock::new(Instant::now()),
            error_count: AtomicU32::new(0),
            restricted_page_count: AtomicU32::new(0),
            restart_count: AtomicU32::new(0),
            pending_inputs: Mutex::new(VecDeque::new()),
            desktop_switch_detected: AtomicBool::new(false),
            explorer_running: AtomicBool::new(true),
            current_desktop_name: Mutex::new(String::new()),
            full_refresh_requested: AtomicBool::new(false),
            tile_reset_requested: AtomicBool::new(false),
            resolution_changing: AtomicBool::new(false),
            active_client_count: AtomicU32::new(0),
            last_client_connection_time: RwLock::new(Instant::now()),
            clients_needing_initial_frame: AtomicU32::new(0),
            initial_connection_phase: AtomicBool::new(true),
            connection_start_time: RwLock::new(Instant::now()),
            last_full_refresh_clear: RwLock::new(Instant::now()),
            connection_active: AtomicBool::new(false),
            disconnect_requested: AtomicBool::new(false),
            cursor_state: Arc::new(Mutex::new(CursorState::default())),
            desktop_context: Arc::new(Mutex::new(None)),
            restricted_screen_active: AtomicBool::new(false),
            session_locked: AtomicBool::new(false),
            compression_quality: AtomicU32::new(70),
            frame_rate_timer: AtomicU32::new(33),
            scaling_factor: AtomicU32::new(1024),
            selected_display: AtomicU32::new(0),
            force_keyframe: AtomicBool::new(false),
            clients_needing_keyframe: AtomicU32::new(0),
            last_keyframe_time: RwLock::new(Instant::now()),
        }
    }
    pub fn set_resolution_changing(&self, val: bool) {
        self.resolution_changing.store(val, Ordering::SeqCst);
    }
    pub fn is_resolution_changing(&self) -> bool {
        self.resolution_changing.load(Ordering::SeqCst)
    }
    pub fn set_remote_connected(&self, connected: bool) {
        self.remote_connected.store(connected, Ordering::Relaxed);
    }
    pub fn should_shutdown(&self) -> bool {
        self.shutdown_requested.load(Ordering::Relaxed) || should_shutdown()
    }
    pub fn set_pause(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }
    pub fn set_force_keyframe(&self, val: bool) {
        self.force_keyframe.store(val, Ordering::Relaxed);
    }
    pub fn set_target_fps(&self, fps: u32) {
        self.target_fps.store(fps.clamp(10, 60), Ordering::Relaxed);
    }
    pub async fn request_full_refresh(&self) {
        let last_clear = self.last_full_refresh_clear.read().await;
        let time_since_clear = Instant::now().duration_since(*last_clear);
        if time_since_clear > Duration::from_secs(3)
            || self.active_client_count.load(Ordering::Relaxed) == 0
        {
            self.full_refresh_requested.store(true, Ordering::Relaxed);
            self.tile_reset_requested.store(true, Ordering::Relaxed);
        } else {
        }
    }
    pub async fn clear_full_refresh_request(&self) {
        self.full_refresh_requested.store(false, Ordering::Relaxed);
        let mut last_clear = self.last_full_refresh_clear.write().await;
        *last_clear = Instant::now();
    }
    pub async fn increment_client_count(&self) -> u32 {
        let new_count = self.active_client_count.fetch_add(1, Ordering::Relaxed) + 1;
        let mut last_time = self.last_client_connection_time.write().await;
        *last_time = Instant::now();
        println!(
            "Client connected - new count: {}, initial phase: {}",
            new_count,
            self.initial_connection_phase.load(Ordering::Relaxed)
        );
        self.clients_needing_initial_frame
            .fetch_add(1, Ordering::Relaxed);
        self.clients_needing_keyframe
            .fetch_add(1, Ordering::Relaxed);
        self.initial_connection_phase.store(true, Ordering::Relaxed);
        let mut conn_start = self.connection_start_time.write().await;
        *conn_start = Instant::now();
        println!(
            "Client {} connected - triggering full refresh and keyframe",
            new_count
        );
        self.full_refresh_requested.store(true, Ordering::Relaxed);
        self.tile_reset_requested.store(true, Ordering::Relaxed);
        self.force_keyframe.store(true, Ordering::Relaxed);
        if new_count == 1 {
            self.set_cursor_visible(true);
        } else {
            self.set_cursor_visible(false);
        }
        new_count
    }

    pub fn decrement_client_count(&self) -> u32 {
        let mut current = self.active_client_count.load(Ordering::Relaxed);
        loop {
            if current == 0 {
                return 0;
            }
            match self.active_client_count.compare_exchange(
                current,
                current - 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    let remaining = current - 1;
                    if remaining == 0 {
                        self.initial_connection_phase
                            .store(false, Ordering::Relaxed);
                        self.full_refresh_requested.store(false, Ordering::Relaxed);
                        self.tile_reset_requested.store(false, Ordering::Relaxed);
                        self.set_cursor_visible(false);
                    } else if remaining == 1 {
                        self.set_cursor_visible(true);
                    }
                    println!("Client disconnected - remaining count: {}", remaining);
                    return remaining;
                }
                Err(actual) => {
                    current = actual;
                }
            }
        }
    }
    pub fn has_clients_needing_initial_frame(&self) -> bool {
        self.clients_needing_initial_frame.load(Ordering::Relaxed) > 0
    }
    pub fn mark_client_initial_frame_sent(&self) {
        let remaining = self
            .clients_needing_initial_frame
            .fetch_sub(1, Ordering::Relaxed);
        self.clients_needing_keyframe
            .fetch_sub(1, Ordering::Relaxed);
        if let Ok(mut last_time) = self.last_keyframe_time.try_write() {
            *last_time = Instant::now();
        }
        if remaining <= 1 {
            self.initial_connection_phase
                .store(false, Ordering::Relaxed);
            println!(" All clients have received initial frame - exiting initial connection phase");
        } else {
            println!(
                " Client received initial frame - {} clients still waiting",
                remaining - 1
            );
        }
    }
    pub fn get_active_client_count(&self) -> u32 {
        self.active_client_count.load(Ordering::Relaxed)
    }
    pub async fn add_pending_input(&self, input: InputEvent) {
        let mut inputs = self.pending_inputs.lock().await;
        if inputs.len() >= 100 {
            inputs.pop_front();
        }
        inputs.push_back(input);
    }
    pub async fn get_pending_inputs(&self) -> Vec<InputEvent> {
        let mut inputs = self.pending_inputs.lock().await;
        let result = inputs.drain(..).collect();
        result
    }
    pub fn set_cursor_visible(&self, visible: bool) {
        if let Ok(mut cursor_state) = self.cursor_state.try_lock() {
            cursor_state.visible = visible;
        }
    }
    pub fn is_cursor_visible(&self) -> bool {
        if let Ok(cursor_state) = self.cursor_state.try_lock() {
            cursor_state.visible
        } else {
            false
        }
    }
    pub fn set_compression_quality(&self, quality: u32) {
        self.compression_quality.store(quality, Ordering::Relaxed);
    }
    pub fn set_frame_rate_timer(&self, timer: u32) {
        self.frame_rate_timer.store(timer, Ordering::Relaxed);
    }
    pub fn set_scaling_factor(&self, factor: u32) {
        self.scaling_factor.store(factor, Ordering::Relaxed);
    }
    pub fn get_scaling_factor(&self) -> u32 {
        self.scaling_factor.load(Ordering::Relaxed)
    }
    pub fn set_compression_settings(&self, compression_type: u8, compression_level: u8) {
        let quality = match compression_type {
            0 => 100,
            1 => match compression_level {
                0 => 60,
                1 => 70,
                2 => 80,
                3 => 90,
                _ => 50,
            },
            _ => 50,
        };
        self.set_compression_quality(quality);
        println!(
            " Compression settings updated: type={}, level={}, quality={}",
            compression_type, compression_level, quality
        );
    }
    pub fn set_selected_display(&self, display_id: u32) {
        self.selected_display.store(display_id, Ordering::Relaxed);
    }
    pub fn get_selected_display(&self) -> u32 {
        self.selected_display.load(Ordering::Relaxed)
    }
    pub fn set_connection_active(&self, active: bool) {
        self.connection_active.store(active, Ordering::Relaxed);
    }
    pub fn request_disconnect(&self) {
        self.disconnect_requested.store(true, Ordering::Relaxed);
    }
    pub fn is_force_keyframe(&self) -> bool {
        self.force_keyframe.load(Ordering::Relaxed)
    }
}
unsafe impl Send for ClientState {}
unsafe impl Sync for ClientState {}
impl ClientStateProvider for ClientState {
    fn is_cursor_visible(&self) -> bool {
        if let Ok(cursor_state) = self.cursor_state.try_lock() {
            cursor_state.visible
        } else {
            false
        }
    }
}
