use crate::network::client_state::{ClientState, InputEvent};
use crate::network::keyboard::{
    handle_ctrl_alt_del, handle_keyboard_event_fixed, handle_unicode_input_fixed,
};
use crate::network::mouse::handle_mouse_event_fixed;
use crate::network::protocol::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
pub struct InputProcessor {
    client_state: Arc<ClientState>,
    last_process_time: Instant,
    input_rate_limiter: HashMap<String, Instant>,
    consecutive_inputs: u32,
    last_mouse_pos: (i32, i32),
    mouse_move_threshold: u32,
}
impl InputProcessor {
    pub fn new(client_state: Arc<ClientState>) -> Self {
        Self {
            client_state,
            last_process_time: Instant::now(),
            input_rate_limiter: HashMap::new(),
            consecutive_inputs: 0,
            last_mouse_pos: (0, 0),
            mouse_move_threshold: 0,
        }
    }
    pub async fn process_pending_inputs(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_process_time) < Duration::from_millis(INPUT_DEBOUNCE_MS) {
            return;
        }
        let inputs = self.client_state.get_pending_inputs().await;
        if inputs.is_empty() {
            self.consecutive_inputs = 0;
            return;
        }
        self.consecutive_inputs += inputs.len() as u32;
        let delay_ms = if self.consecutive_inputs > 100 {
            50
        } else if self.consecutive_inputs > 50 {
            25
        } else if self.consecutive_inputs > 20 {
            15
        } else {
            INPUT_DEBOUNCE_MS
        };
        let mut significant_input_processed = false;
        for (index, input) in inputs.into_iter().enumerate() {
            if self.client_state.should_shutdown() {
                break;
            }
            if index > 0 && delay_ms > INPUT_DEBOUNCE_MS {
                sleep(Duration::from_millis(2)).await;
            }
            match input {
                InputEvent::Mouse { data, x, y } => {
                    let is_button_event = if data.len() > 0 {
                        let button = data[0];
                        matches!(
                            button,
                            0x02 | 0x04
                                | 0x05
                                | 0x06
                                | 0x08
                                | 0x0A
                                | 0x0C
                                | 0x10
                                | 0x20
                                | 0x40
                                | 0x88
                        )
                    } else {
                        false
                    };
                    let should_process = if is_button_event {
                        true
                    } else {
                        self.should_process_mouse_input(x, y, now)
                    };
                    if should_process {
                        handle_mouse_event_fixed(&data);
                        self.last_mouse_pos = (x, y);
                        if is_button_event {
                            significant_input_processed = true;
                        }
                    }
                }
                InputEvent::Keyboard { data } => {
                    if self.should_process_input("keyboard", now) {
                        handle_keyboard_event_fixed(&data);
                        significant_input_processed = true;
                    }
                }
                InputEvent::Unicode { data } => {
                    if self.should_process_input("unicode", now) {
                        handle_unicode_input_fixed(&data);
                        significant_input_processed = true;
                    }
                }
                InputEvent::CtrlAltDel => {
                    handle_ctrl_alt_del();
                    significant_input_processed = true;
                }
                InputEvent::MouseMove { x, y } => {
                    if self.should_process_mouse_move(x, y) {
                        self.last_mouse_pos = (x, y);
                    }
                }
                InputEvent::CursorUpdate { cursor_type: _ } => {}
            }
        }
        if significant_input_processed {
            self.client_state.request_full_refresh().await;
        }
        if now.duration_since(self.last_process_time) > Duration::from_millis(200) {
            self.consecutive_inputs = self.consecutive_inputs.saturating_sub(10);
        }
        self.last_process_time = now;
    }
    fn should_process_input(&mut self, input_type: &str, now: Instant) -> bool {
        let key = input_type.to_string();
        if let Some(&last_time) = self.input_rate_limiter.get(&key) {
            let min_interval = match input_type {
                "keyboard" => Duration::from_millis(8),
                "mouse" => Duration::from_millis(2),
                "unicode" => Duration::from_millis(15),
                _ => Duration::from_millis(INPUT_DEBOUNCE_MS),
            };
            if now.duration_since(last_time) < min_interval {
                return false;
            }
        }
        self.input_rate_limiter.insert(key, now);
        true
    }
    fn should_process_mouse_input(&mut self, x: i32, y: i32, now: Instant) -> bool {
        if !self.should_process_input("mouse", now) {
            return false;
        }
        let dx = (x - self.last_mouse_pos.0).abs();
        let dy = (y - self.last_mouse_pos.1).abs();
        dx > 0 || dy > 0 || now.duration_since(self.last_process_time) > Duration::from_millis(30)
    }
    fn should_process_mouse_move(&mut self, x: i32, y: i32) -> bool {
        let dx = (x - self.last_mouse_pos.0).abs();
        let dy = (y - self.last_mouse_pos.1).abs();
        dx > 1 || dy > 1
    }
}
