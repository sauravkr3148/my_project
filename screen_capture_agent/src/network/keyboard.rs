use enigo::{Enigo, Key, KeyboardControllable};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Mutex;
const KEY_DOWN: u8 = 0;
const KEY_UP: u8 = 1;
const KEY_EXTENDED_DOWN: u8 = 2;
const KEY_EXTENDED_UP: u8 = 3;
lazy_static! {
    static ref ENIGO: Mutex<Enigo> = Mutex::new(Enigo::new());
    static ref KEY_MAP: HashMap<u8, Key> = create_key_map();
    static ref MODIFIER_STATE: Mutex<ModifierState> = Mutex::new(ModifierState::default());
}
#[derive(Debug, Default)]
struct ModifierState {
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
}
pub fn handle_keyboard_event_fixed(data: &[u8]) {
    if data.len() < 2 {
        return;
    }
    let action = data[0];
    let keycode = data[1];
    match action {
        KEY_DOWN => {
            handle_key_down(keycode, false);
        }
        KEY_UP => {
            handle_key_up(keycode, false);
        }
        KEY_EXTENDED_DOWN => {
            handle_key_down(keycode, true);
        }
        KEY_EXTENDED_UP => {
            handle_key_up(keycode, true);
        }
        _ => {}
    }
}
pub fn handle_unicode_input_fixed(data: &[u8]) {
    if data.len() < 7 {
        return;
    }
    let action = data[4];
    let unicode_high = data[5] as u16;
    let unicode_low = data[6] as u16;
    let unicode = (unicode_high << 8) | unicode_low;
    if action == KEY_DOWN {
        handle_unicode_input(unicode);
    }
}
pub fn handle_ctrl_alt_del() {
    let mut enigo = ENIGO.lock().unwrap();
    enigo.key_down(Key::Control);
    enigo.key_down(Key::Alt);
    enigo.key_down(Key::Delete);
    enigo.key_up(Key::Delete);
    enigo.key_up(Key::Alt);
    enigo.key_up(Key::Control);
}
fn create_key_map() -> HashMap<u8, Key> {
    let mut map = HashMap::new();
    for i in 0x41..=0x5A {
        let key_char = (i as u8 as char).to_lowercase().next().unwrap();
        map.insert(i, Key::Layout(key_char));
    }
    for i in 0x30..=0x39 {
        let key_char = i as u8 as char;
        map.insert(i, Key::Layout(key_char));
    }
    map.insert(0x70, Key::F1);
    map.insert(0x71, Key::F2);
    map.insert(0x72, Key::F3);
    map.insert(0x73, Key::F4);
    map.insert(0x74, Key::F5);
    map.insert(0x75, Key::F6);
    map.insert(0x76, Key::F7);
    map.insert(0x77, Key::F8);
    map.insert(0x78, Key::F9);
    map.insert(0x79, Key::F10);
    map.insert(0x7A, Key::F11);
    map.insert(0x7B, Key::F12);
    map.insert(0x7C, Key::F13);
    map.insert(0x7D, Key::F14);
    map.insert(0x7E, Key::F15);
    map.insert(0x7F, Key::F16);
    map.insert(0x80, Key::F17);
    map.insert(0x81, Key::F18);
    map.insert(0x82, Key::F19);
    map.insert(0x83, Key::F20);
    map.insert(0x84, Key::F21);
    map.insert(0x85, Key::F22);
    map.insert(0x86, Key::F23);
    map.insert(0x87, Key::F24);
    map.insert(0x08, Key::Backspace);
    map.insert(0x09, Key::Tab);
    map.insert(0x0D, Key::Return);
    map.insert(0x10, Key::Shift);
    map.insert(0x11, Key::Control);
    map.insert(0x12, Key::Alt);
    map.insert(0x13, Key::Pause);
    map.insert(0x14, Key::CapsLock);
    map.insert(0x1B, Key::Escape);
    map.insert(0x20, Key::Space);
    map.insert(0x21, Key::PageUp);
    map.insert(0x22, Key::PageDown);
    map.insert(0x23, Key::End);
    map.insert(0x24, Key::Home);
    map.insert(0x25, Key::LeftArrow);
    map.insert(0x26, Key::UpArrow);
    map.insert(0x27, Key::RightArrow);
    map.insert(0x28, Key::DownArrow);
    map.insert(0x2C, Key::Print);
    map.insert(0x2D, Key::Insert);
    map.insert(0x2E, Key::Delete);
    map.insert(0x5B, Key::Meta);
    map.insert(0x5C, Key::RWin);
    map.insert(0x5D, Key::Apps);
    map.insert(0x60, Key::Layout('0'));
    map.insert(0x61, Key::Layout('1'));
    map.insert(0x62, Key::Layout('2'));
    map.insert(0x63, Key::Layout('3'));
    map.insert(0x64, Key::Layout('4'));
    map.insert(0x65, Key::Layout('5'));
    map.insert(0x66, Key::Layout('6'));
    map.insert(0x67, Key::Layout('7'));
    map.insert(0x68, Key::Layout('8'));
    map.insert(0x69, Key::Layout('9'));
    map.insert(0x6A, Key::Layout('*'));
    map.insert(0x6B, Key::Layout('+'));
    map.insert(0x6D, Key::Layout('-'));
    map.insert(0x6E, Key::Layout('.'));
    map.insert(0x6F, Key::Layout('/'));
    map.insert(0xBA, Key::Layout(';'));
    map.insert(0xBB, Key::Layout('='));
    map.insert(0xBC, Key::Layout(','));
    map.insert(0xBD, Key::Layout('-'));
    map.insert(0xBE, Key::Layout('.'));
    map.insert(0xBF, Key::Layout('/'));
    map.insert(0xC0, Key::Layout('`'));
    map.insert(0xDB, Key::Layout('['));
    map.insert(0xDC, Key::Layout('\\'));
    map.insert(0xDD, Key::Layout(']'));
    map.insert(0xDE, Key::Layout('\''));
    map.insert(0x90, Key::Numlock);
    map.insert(0x91, Key::Scroll);
    map.insert(0xA0, Key::Shift);
    map.insert(0xA1, Key::RShift);
    map.insert(0xA2, Key::Control);
    map.insert(0xA3, Key::RControl);
    map.insert(0xA4, Key::Alt);
    map.insert(0xA5, Key::RMenu);
    map
}
fn handle_key_down(keycode: u8, _extended: bool) {
    let mut enigo = ENIGO.lock().unwrap();
    let mut modifier_state = MODIFIER_STATE.lock().unwrap();
    match keycode {
        0x10 | 0xA0 => {
            modifier_state.shift = true;
            enigo.key_down(Key::Shift);
            return;
        }
        0xA1 => {
            modifier_state.shift = true;
            enigo.key_down(Key::RShift);
            return;
        }
        0x11 | 0xA2 => {
            modifier_state.ctrl = true;
            enigo.key_down(Key::Control);
            return;
        }
        0xA3 => {
            modifier_state.ctrl = true;
            enigo.key_down(Key::RControl);
            return;
        }
        0x12 | 0xA4 => {
            modifier_state.alt = true;
            enigo.key_down(Key::Alt);
            return;
        }
        0xA5 => {
            modifier_state.alt = true;
            enigo.key_down(Key::RMenu);
            return;
        }
        0x5B | 0x5C => {
            modifier_state.win = true;
            if keycode == 0x5C {
                enigo.key_down(Key::RWin);
            } else {
                enigo.key_down(Key::Meta);
            }
            return;
        }
        _ => {}
    }
    if handle_keyboard_shortcuts(keycode, &modifier_state, &mut enigo) {
        return;
    }
    if let Some(key) = KEY_MAP.get(&keycode) {
        enigo.key_down(*key);
    } else {
    }
}
fn handle_key_up(keycode: u8, _extended: bool) {
    let mut enigo = ENIGO.lock().unwrap();
    let mut modifier_state = MODIFIER_STATE.lock().unwrap();
    match keycode {
        0x10 | 0xA0 => {
            modifier_state.shift = false;
            enigo.key_up(Key::Shift);
            return;
        }
        0xA1 => {
            modifier_state.shift = false;
            enigo.key_up(Key::RShift);
            return;
        }
        0x11 | 0xA2 => {
            modifier_state.ctrl = false;
            enigo.key_up(Key::Control);
            return;
        }
        0xA3 => {
            modifier_state.ctrl = false;
            enigo.key_up(Key::RControl);
            return;
        }
        0x12 | 0xA4 => {
            modifier_state.alt = false;
            enigo.key_up(Key::Alt);
            return;
        }
        0xA5 => {
            modifier_state.alt = false;
            enigo.key_up(Key::RMenu);
            return;
        }
        0x5B | 0x5C => {
            modifier_state.win = false;
            if keycode == 0x5C {
                enigo.key_up(Key::RWin);
            } else {
                enigo.key_up(Key::Meta);
            }
            return;
        }
        _ => {}
    }
    if let Some(key) = KEY_MAP.get(&keycode) {
        enigo.key_up(*key);
    } else {
    }
}
fn handle_keyboard_shortcuts(
    keycode: u8,
    modifier_state: &ModifierState,
    enigo: &mut Enigo,
) -> bool {
    if modifier_state.ctrl {
        match keycode {
            0x41 => {
                enigo.key_click(Key::Layout('a'));
                return true;
            }
            0x43 => {
                enigo.key_click(Key::Layout('c'));
                return true;
            }
            0x56 => {
                enigo.key_click(Key::Layout('v'));
                return true;
            }
            0x58 => {
                enigo.key_click(Key::Layout('x'));
                return true;
            }
            0x5A => {
                enigo.key_click(Key::Layout('z'));
                return true;
            }
            0x59 => {
                enigo.key_click(Key::Layout('y'));
                return true;
            }
            0x53 => {
                enigo.key_click(Key::Layout('s'));
                return true;
            }
            0x4F => {
                enigo.key_click(Key::Layout('o'));
                return true;
            }
            0x4E => {
                enigo.key_click(Key::Layout('n'));
                return true;
            }
            0x46 => {
                enigo.key_click(Key::Layout('f'));
                return true;
            }
            0x48 => {
                enigo.key_click(Key::Layout('h'));
                return true;
            }
            0x50 => {
                enigo.key_click(Key::Layout('p'));
                return true;
            }
            _ => {}
        }
    }
    if modifier_state.alt {
        match keycode {
            0x09 => {
                enigo.key_click(Key::Tab);
                return true;
            }
            0x73 => {
                enigo.key_click(Key::F4);
                return true;
            }
            _ => {}
        }
    }
    if modifier_state.ctrl && modifier_state.shift {
        match keycode {
            0x4E => {
                enigo.key_click(Key::Layout('n'));
                return true;
            }
            0x54 => {
                enigo.key_click(Key::Layout('t'));
                return true;
            }
            _ => {}
        }
    }
    false
}
fn handle_unicode_input(unicode: u16) {
    if let Some(character) = char::from_u32(unicode as u32) {
        let mut enigo = ENIGO.lock().unwrap();
        enigo.key_sequence(&character.to_string());
    } else {
    }
}
