use enigo::{Enigo, MouseButton, MouseControllable};
use lazy_static::lazy_static;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;
static MOUSE_EVENT_COUNTER: AtomicU64 = AtomicU64::new(0);
static LAST_MOUSE_LOG_TIME: Mutex<Option<Instant>> = Mutex::new(None);
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseButtonState {
    Up,
    Down,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DragState {
    None,
    LeftDrag,
    RightDrag,
    MiddleDrag,
}
#[derive(Debug)]
pub struct MouseState {
    last_position: (i32, i32),
    left_button: MouseButtonState,
    right_button: MouseButtonState,
    middle_button: MouseButtonState,
    back_button: MouseButtonState,
    forward_button: MouseButtonState,
    drag_state: DragState,
    drag_start_pos: Option<(i32, i32)>,
}
impl Default for MouseState {
    fn default() -> Self {
        Self {
            last_position: (0, 0),
            left_button: MouseButtonState::Up,
            right_button: MouseButtonState::Up,
            middle_button: MouseButtonState::Up,
            back_button: MouseButtonState::Up,
            forward_button: MouseButtonState::Up,
            drag_state: DragState::None,
            drag_start_pos: None,
        }
    }
}
lazy_static! {
    static ref MOUSE_STATE: Mutex<MouseState> = Mutex::new(MouseState::default());
    static ref ENIGO: Mutex<Enigo> = Mutex::new(Enigo::new());
}
pub fn handle_mouse_event_fixed(data: &[u8]) {
    if data.len() < 5 {
        return;
    }
    let count = MOUSE_EVENT_COUNTER.fetch_add(1, Ordering::Relaxed);
    if count % 1000 == 0 {
        let mut last_time = LAST_MOUSE_LOG_TIME.lock().unwrap();
        if let Some(last) = *last_time {
            let elapsed = last.elapsed();
            if elapsed.as_millis() > 0 {
                let events_per_sec = 1000.0 / (elapsed.as_millis() as f64);
                if count % 10000 == 0 {
                    println!("🖱️ Mouse performance: {:.0} events/sec", events_per_sec);
                }
            }
        }
        *last_time = Some(Instant::now());
    }
    let button = data[0];
    let x = (((data[1] as u16) << 8) | (data[2] as u16)) as i32;
    let y = (((data[3] as u16) << 8) | (data[4] as u16)) as i32;
    let (wheel_delta_y, wheel_delta_x) = if data.len() >= 7 {
        let vertical = i16::from_be_bytes([data[5], data[6]]);
        let horizontal = if data.len() >= 9 {
            i16::from_be_bytes([data[7], data[8]])
        } else {
            0
        };
        (vertical, horizontal)
    } else {
        (0, 0)
    };
    let mut state = MOUSE_STATE.lock().unwrap();
    let mut enigo = ENIGO.lock().unwrap();
    enigo.mouse_move_to(x, y);
    state.last_position = (x, y);
    match button {
        0x00 => {
            if wheel_delta_y != 0 || wheel_delta_x != 0 {
                handle_wheel(&mut enigo, wheel_delta_y, wheel_delta_x);
            } else {
                handle_drag_move(&mut state, x, y);
            }
        }
        0x02 => {
            handle_button_down(&mut state, &mut enigo, MouseButton::Left, x, y);
        }
        0x04 => {
            handle_button_up(&mut state, &mut enigo, MouseButton::Left, x, y);
        }
        0x08 => {
            handle_button_down(&mut state, &mut enigo, MouseButton::Right, x, y);
        }
        0x10 => {
            handle_button_up(&mut state, &mut enigo, MouseButton::Right, x, y);
        }
        0x20 => {
            handle_button_down(&mut state, &mut enigo, MouseButton::Middle, x, y);
        }
        0x40 => {
            handle_button_up(&mut state, &mut enigo, MouseButton::Middle, x, y);
        }
        0x05 => {
            handle_button_down(&mut state, &mut enigo, MouseButton::Back, x, y);
        }
        0x0A => {
            handle_button_up(&mut state, &mut enigo, MouseButton::Back, x, y);
        }
        0x06 => {
            handle_button_down(&mut state, &mut enigo, MouseButton::Forward, x, y);
        }
        0x0C => {
            handle_button_up(&mut state, &mut enigo, MouseButton::Forward, x, y);
        }
        0x88 => {
            handle_double_click(&mut state, &mut enigo, x, y);
        }
        _ => {
            println!("Unknown mouse button: 0x{:02X} at ({}, {})", button, x, y);
        }
    }
}
pub fn handle_button_down(
    state: &mut MouseState,
    enigo: &mut Enigo,
    button: MouseButton,
    x: i32,
    y: i32,
) {
    let is_primary = matches!(
        button,
        MouseButton::Left | MouseButton::Right | MouseButton::Middle
    );
    match button {
        MouseButton::Left => {
            state.left_button = MouseButtonState::Down;
            state.drag_state = DragState::LeftDrag;
        }
        MouseButton::Right => {
            state.right_button = MouseButtonState::Down;
            state.drag_state = DragState::RightDrag;
        }
        MouseButton::Middle => {
            state.middle_button = MouseButtonState::Down;
            state.drag_state = DragState::MiddleDrag;
        }
        MouseButton::Back => {
            state.back_button = MouseButtonState::Down;
        }
        MouseButton::Forward => {
            state.forward_button = MouseButtonState::Down;
        }
        _ => {}
    }
    if is_primary {
        state.drag_start_pos = Some((x, y));
    }
    enigo.mouse_down(button);
}
pub fn handle_button_up(
    state: &mut MouseState,
    enigo: &mut Enigo,
    button: MouseButton,
    x: i32,
    y: i32,
) {
    let is_primary = matches!(
        button,
        MouseButton::Left | MouseButton::Right | MouseButton::Middle
    );
    let was_dragging = is_primary && state.drag_state != DragState::None;
    let drag_distance = if let Some((start_x, start_y)) = state.drag_start_pos {
        (((x - start_x).pow(2) + (y - start_y).pow(2)) as f64).sqrt()
    } else {
        0.0
    };
    match button {
        MouseButton::Left => {
            state.left_button = MouseButtonState::Up;
            if state.drag_state == DragState::LeftDrag {
                state.drag_state = DragState::None;
            }
        }
        MouseButton::Right => {
            state.right_button = MouseButtonState::Up;
            if state.drag_state == DragState::RightDrag {
                state.drag_state = DragState::None;
            }
        }
        MouseButton::Middle => {
            state.middle_button = MouseButtonState::Up;
            if state.drag_state == DragState::MiddleDrag {
                state.drag_state = DragState::None;
            }
        }
        MouseButton::Back => {
            state.back_button = MouseButtonState::Up;
        }
        MouseButton::Forward => {
            state.forward_button = MouseButtonState::Up;
        }
        _ => {}
    }
    if is_primary {
        state.drag_start_pos = None;
    }
    enigo.mouse_up(button);
    if was_dragging && drag_distance > 5.0 {
    } else {
    }
}
pub fn handle_drag_move(state: &mut MouseState, x: i32, y: i32) {
    if state.drag_state != DragState::None {
        if let Some((start_x, start_y)) = state.drag_start_pos {
            let distance = (((x - start_x).pow(2) + (y - start_y).pow(2)) as f64).sqrt();
            if distance > 2.0 {}
        }
    }
}
#[cfg(windows)]
const WHEEL_DELTA: i16 = 120;
pub fn handle_wheel(enigo: &mut Enigo, delta_y: i16, delta_x: i16) {
    if delta_y == 0 && delta_x == 0 {
        return;
    }
    #[cfg(windows)]
    {
        if delta_y != 0 {
            let mut y = delta_y as i32;
            y *= WHEEL_DELTA as i32;
            enigo.mouse_scroll_y(y);
        }
        if delta_x != 0 {
            let mut x = delta_x as i32;
            x *= WHEEL_DELTA as i32;
            enigo.mouse_scroll_x(x);
        }
    }
    #[cfg(not(windows))]
    {
        if delta_y != 0 {
            let clicks = (delta_y.abs() / 120).max(1) as i32;
            let step = if delta_y > 0 { -1 } else { 1 };
            for _ in 0..clicks {
                enigo.mouse_scroll_y(step);
            }
        }
        if delta_x != 0 {
            let clicks = (delta_x.abs() / 120).max(1) as i32;
            let step = if delta_x > 0 { 1 } else { -1 };
            for _ in 0..clicks {
                enigo.mouse_scroll_x(step);
            }
        }
    }
}
pub fn handle_double_click(state: &mut MouseState, enigo: &mut Enigo, x: i32, y: i32) {
    enigo.mouse_move_to(x, y);
    enigo.mouse_click(MouseButton::Left);
    enigo.mouse_click(MouseButton::Left);
    state.last_position = (x, y);
}
