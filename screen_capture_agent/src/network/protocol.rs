use crate::network::client_state::ClientState;
use crate::video_encoder::align_dimensions;
use byteorder::{BigEndian, WriteBytesExt};
pub const MNG_KVM_SCREEN: u16 = 7;
pub const MNG_KVM_GET_DISPLAYS: u16 = 11;
pub const MNG_KVM_REFRESH: u16 = 6;
pub const MNG_KVM_PICTURE: u16 = 3;
pub const MNG_KVM_AGENT_CONNECTED: u16 = 59;
pub const INPUT_DEBOUNCE_MS: u64 = 10;
pub const MNG_KVM_DISPLAY_INFO: u16 = 82;
pub const MNG_KVM_FRAME_RATE_TIMER: u8 = 13;
pub const MNG_KVM_INIT_TOUCH: u8 = 14;
pub const MNG_KVM_KEYSTATE: u8 = 18;
pub const MNG_KVM_MOUSE_CURSOR: u8 = 88;
pub const MNG_KVM_CURSOR_CONTROL: u8 = 144;
pub const MNG_UPDATE_TEMP_WALLPAPER: u8 = 73;
pub const MNG_RESTORE_ORIGINAL_WALLPAPER: u8 = 74;
pub fn create_resolution_packet_with_scaling(client_state: &ClientState) -> Vec<u8> {
    let mut packet = Vec::new();
    let (raw_width, raw_height) = get_raw_screen_resolution();
    let (aligned_width, aligned_height) = align_dimensions(raw_width as usize, raw_height as usize);
    let scaling_factor = client_state.get_scaling_factor() as u32;
    let scaled_width = (aligned_width as u32 * scaling_factor) / 1024;
    let scaled_height = (aligned_height as u32 * scaling_factor) / 1024;
    packet.write_u16::<BigEndian>(MNG_KVM_SCREEN).unwrap();
    packet.write_u16::<BigEndian>(8).unwrap();
    packet.write_u16::<BigEndian>(scaled_width as u16).unwrap();
    packet.write_u16::<BigEndian>(scaled_height as u16).unwrap();
    println!(
        " Created resolution packet (C agent format): {}x{} (aligned {}x{}, raw {}x{})",
        scaled_width, scaled_height, aligned_width, aligned_height, raw_width, raw_height
    );
    packet
}
pub fn create_display_list_packet_with_scaling(client_state: &ClientState) -> Vec<u8> {
    let mut packet = Vec::new();
    let displays = get_display_list();
    let screen_count = displays.len();
    if screen_count <= 1 {
        packet.write_u16::<BigEndian>(MNG_KVM_GET_DISPLAYS).unwrap();
        packet.write_u16::<BigEndian>(8).unwrap();
        packet.write_u16::<BigEndian>(0).unwrap();
        packet.write_u16::<BigEndian>(0).unwrap();
    } else {
        let packet_size = 10 + (2 * screen_count);
        packet.write_u16::<BigEndian>(MNG_KVM_GET_DISPLAYS).unwrap();
        packet.write_u16::<BigEndian>(packet_size as u16).unwrap();
        packet
            .write_u16::<BigEndian>((screen_count + 1) as u16)
            .unwrap();
        packet.write_u16::<BigEndian>(0xFFFF).unwrap();
        for i in 0..screen_count {
            packet.write_u16::<BigEndian>((i + 1) as u16).unwrap();
        }
        let selected_display = client_state.get_selected_display();
        if selected_display == 0 {
            packet.write_u16::<BigEndian>(0xFFFF).unwrap();
        } else {
            packet
                .write_u16::<BigEndian>(selected_display as u16)
                .unwrap();
        }
    }
    println!(
        " Created display list packet (C agent format): {} displays",
        screen_count
    );
    packet
}
pub fn create_display_info_packet() -> Vec<u8> {
    let mut packet = Vec::new();
    let displays = get_display_list();
    let packet_size = 4 + (displays.len() * 12);
    packet.write_u16::<BigEndian>(MNG_KVM_DISPLAY_INFO).unwrap();
    packet.write_u16::<BigEndian>(packet_size as u16).unwrap();
    packet
        .write_u16::<BigEndian>(displays.len() as u16)
        .unwrap();
    for display in &displays {
        packet.write_u16::<BigEndian>(display.width).unwrap();
        packet.write_u16::<BigEndian>(display.height).unwrap();
        packet.write_i32::<BigEndian>(display.x).unwrap();
        packet.write_i32::<BigEndian>(display.y).unwrap();
    }
    println!(
        "Created display info packet (C agent format): {} displays",
        displays.len()
    );
    packet
}
pub fn create_agent_connected_packet() -> Vec<u8> {
    let mut packet = Vec::new();
    packet
        .write_u16::<BigEndian>(MNG_KVM_AGENT_CONNECTED)
        .unwrap();
    packet.write_u16::<BigEndian>(0).unwrap();
    println!(" Created agent connected packet");
    packet
}

pub fn create_video_frame_packet(frame_data: &[u8], codec_type: u8, is_keyframe: bool) -> Vec<u8> {
    let mut packet = Vec::new();
    let frame_len = frame_data.len();
    let total_packet_size = 6u32.saturating_add(frame_len as u32);

    let size_field = if total_packet_size > 65535 {
        0xFFFFu16
    } else {
        total_packet_size as u16
    };

    packet.write_u16::<BigEndian>(MNG_KVM_PICTURE).unwrap();
    packet.write_u16::<BigEndian>(size_field).unwrap();
    packet.write_u8(codec_type).unwrap();
    packet.write_u8(if is_keyframe { 1 } else { 0 }).unwrap();
    packet
        .write_u32::<BigEndian>(frame_data.len() as u32)
        .unwrap();
    packet.extend_from_slice(frame_data);
    packet
}
#[derive(Debug, Clone)]
pub struct DisplayInfo {
    pub width: u16,
    pub height: u16,
    pub x: i32,
    pub y: i32,
}
pub fn get_screen_resolution() -> (u16, u16) {
    let (raw_width, raw_height) = get_raw_screen_resolution();
    let (aligned_width, aligned_height) = align_dimensions(raw_width as usize, raw_height as usize);
    (aligned_width as u16, aligned_height as u16)
}

fn get_raw_screen_resolution() -> (u16, u16) {
    #[cfg(target_os = "windows")]
    {
        use winapi::um::winuser::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
        unsafe {
            let width = GetSystemMetrics(SM_CXSCREEN) as u16;
            let height = GetSystemMetrics(SM_CYSCREEN) as u16;
            (width, height)
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        (1920, 1080)
    }
}
pub fn get_display_list() -> Vec<DisplayInfo> {
    #[cfg(target_os = "windows")]
    {
        use std::mem;
        use std::ptr;
        use winapi::shared::minwindef::{BOOL, DWORD, LPARAM};
        use winapi::shared::windef::{HDC, HMONITOR, LPRECT, RECT};
        use winapi::um::winuser::{EnumDisplayMonitors, GetMonitorInfoW, MONITORINFO};
        let mut displays = Vec::new();
        unsafe extern "system" fn enum_proc(
            hmonitor: HMONITOR,
            _hdc: HDC,
            _lprect: LPRECT,
            lparam: LPARAM,
        ) -> BOOL {
            unsafe {
                let displays = &mut *(lparam as *mut Vec<DisplayInfo>);
                let mut monitor_info: MONITORINFO = mem::zeroed();
                monitor_info.cbSize = mem::size_of::<MONITORINFO>() as DWORD;
                if GetMonitorInfoW(hmonitor, &mut monitor_info) != 0 {
                    let rect = monitor_info.rcMonitor;
                    let raw_width = (rect.right - rect.left) as u16;
                    let raw_height = (rect.bottom - rect.top) as u16;
                    let (aligned_width, aligned_height) =
                        align_dimensions(raw_width as usize, raw_height as usize);
                    displays.push(DisplayInfo {
                        width: aligned_width as u16,
                        height: aligned_height as u16,
                        x: rect.left,
                        y: rect.top,
                    });
                }
                1
            }
        }
        unsafe {
            EnumDisplayMonitors(
                ptr::null_mut(),
                ptr::null_mut(),
                Some(enum_proc),
                &mut displays as *mut Vec<DisplayInfo> as LPARAM,
            );
        }
        if displays.is_empty() {
            let (width, height) = get_screen_resolution();
            displays.push(DisplayInfo {
                width,
                height,
                x: 0,
                y: 0,
            });
        }
        displays
    }
    #[cfg(not(target_os = "windows"))]
    {
        let (width, height) = get_screen_resolution();
        vec![DisplayInfo {
            width,
            height,
            x: 0,
            y: 0,
        }]
    }
}
pub fn get_codec_type_id(codec: &scrap::CodecFormat) -> u8 {
    match codec {
        scrap::CodecFormat::VP8 => 1,
        scrap::CodecFormat::VP9 => 2,
        scrap::CodecFormat::H264 => 3,
        scrap::CodecFormat::H265 => 4,
        _ => 2,
    }
}
