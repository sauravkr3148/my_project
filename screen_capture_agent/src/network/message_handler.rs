use super::client_state::{ClientState, InputEvent};
use super::protocol::*;
use anyhow::Result;
use flume::Sender;
use futures_util::stream::SplitSink;
use futures_util::SinkExt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
static FRAME_RATE_TIMER_TX: once_cell::sync::OnceCell<tokio::sync::mpsc::Sender<u32>> =
    once_cell::sync::OnceCell::new();
pub struct MessageHandler {
    pub client_state: Arc<ClientState>,
}
impl MessageHandler {
    pub fn new(client_state: Arc<ClientState>) -> Self {
        Self { client_state }
    }
    pub async fn handle_message(
        &self,
        data: Vec<u8>,
        ws_sender: &Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
        handshake_tx: &Sender<bool>,
    ) -> Result<()> {
        if data.len() < 4 {
            println!(" Received message too short: {} bytes", data.len());
            return Ok(());
        }
        match data.as_slice() {
            [0, 73, 0, 6, 0, 1] => {
                println!("NEW CLIENT CONNECTED - Initializing full screen refresh");
                let active_clients = self.client_state.increment_client_count().await;
                println!("Active client count: {}", active_clients);
                if active_clients == 1 {
                    println!(" Single client connected - cursor capture enabled by default");
                } else {
                    println!(
                        " Multiple clients connected ({}) - cursor capture requires explicit enable",
                        active_clients
                    );
                }
                println!("Client connected - will need full screen before tiles");
                self.handle_client_connect(ws_sender, handshake_tx).await?;
            }
            [0, 74, 0, 6, 0, 1] => {
                println!("Client disconnected - cleaning up");
                self.client_state.set_connection_active(false);
                self.client_state.set_remote_connected(false);
                self.client_state.request_disconnect();
                self.client_state.set_pause(true);
                let remaining = self.client_state.decrement_client_count();
                if remaining == 0 {
                    println!(" No active clients remaining - cursor capture disabled");
                } else if remaining == 1 {
                    println!(" Single client remaining - cursor capture enabled by default");
                }
            }
            [0, 14, 0, 4] => {
                println!(
                    " Received client ACK (0x0E) from client - client has processed initial screen"
                );
                self.client_state.set_remote_connected(true);
                let _ = handshake_tx.try_send(true);
                println!(
                    " Client fully connected and ready - tile updates now enabled for this client"
                );
            }
            _ => {
                if data.len() >= 1 {
                    self.handle_input_message(data, ws_sender).await?;
                } else {
                    println!(" Unknown message format received (too short): {:?}", data);
                }
            }
        }
        Ok(())
    }
    async fn handle_client_connect(
        &self,
        ws_sender: &Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
        _handshake_tx: &Sender<bool>,
    ) -> Result<()> {
        println!("⏳ Waiting for client to initialize loading state...");
        let mut sender = ws_sender.lock().await;
        let resolution_packet = create_resolution_packet_with_scaling(&self.client_state);
        println!(
            " AGENT SENDING Resolution Packet: Length={}, Data={:?}",
            resolution_packet.len(),
            &resolution_packet[..std::cmp::min(resolution_packet.len(), 20)]
        );
        sender
            .send(Message::Binary(resolution_packet))
            .await
            .map_err(|e| {
                println!("Failed to send resolution packet: {}", e);
                e
            })?;
        println!(" Resolution packet sent to new client");
        let display_info_packet = create_display_info_packet();
        sender
            .send(Message::Binary(display_info_packet))
            .await
            .map_err(|e| {
                println!("Failed to send display info packet: {}", e);
                e
            })?;
        println!(" Display info packet sent to new client");
        let display_list_packet = create_display_list_packet_with_scaling(&self.client_state);
        sender
            .send(Message::Binary(display_list_packet))
            .await
            .map_err(|e| {
                println!("Failed to send display list packet: {}", e);
                e
            })?;
        println!(" Display list packet sent to new client");
        let keystate_packet = create_keystate_packet();
        println!(
            " AGENT SENDING Keystate Packet: Length={}, Data={:?}",
            keystate_packet.len(),
            &keystate_packet[..std::cmp::min(keystate_packet.len(), 20)]
        );
        sender
            .send(Message::Binary(keystate_packet))
            .await
            .map_err(|e| {
                println!("Failed to send keystate packet: {}", e);
                e
            })?;
        println!(" Keystate packet sent");
        let mouse_cursor_packet = create_mouse_cursor_packet();
        println!(
            " AGENT SENDING Mouse Cursor Packet: Length={}, Data={:?}",
            mouse_cursor_packet.len(),
            &mouse_cursor_packet[..std::cmp::min(mouse_cursor_packet.len(), 20)]
        );
        sender
            .send(Message::Binary(mouse_cursor_packet))
            .await
            .map_err(|e| {
                println!("Failed to send mouse cursor packet: {}", e);
                e
            })?;
        println!(" Mouse cursor packet sent");
        // RustDesk approach: No delay - send packets immediately for instant connection
        let refresh_packet = create_refresh_packet();
        sender
            .send(Message::Binary(refresh_packet))
            .await
            .map_err(|e| {
                println!("Failed to send refresh packet: {}", e);
                e
            })?;
        println!(" Sent refresh packet (cmd=6) to client");
        println!(" Touch injection not supported - reporting failure to client");
        let mut touch_response = Vec::new();
        touch_response.push(MNG_KVM_INIT_TOUCH);
        touch_response.push(0);
        touch_response.push(0);
        touch_response.push(4);
        touch_response.push(2); // 2 = failure
        sender
            .send(Message::Binary(touch_response))
            .await
            .map_err(|e| {
                println!(" Failed to send auto touch init response: {}", e);
                e
            })?;
        self.client_state.set_force_keyframe(true);
        println!(" Forced keyframe for new client");
        // RustDesk approach: No delay - start streaming immediately
        tokio::spawn({
            async move {
                println!("Is it running??");
                println!(
                    " Client connection setup complete - waiting for client ack (0x0E) before joining delta stream"
                );
            }
        });
        Ok(())
    }
    async fn handle_input_message(
        &self,
        data: Vec<u8>,
        ws_sender: &Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    ) -> Result<()> {
        let input_type = data[1];
        match input_type {
            2 => {
                if data.len() >= 10 {
                    let mouse_data = &data[5..];
                    let data_clone = mouse_data.to_vec();
                    let x = if mouse_data.len() >= 3 {
                        u16::from_be_bytes([mouse_data[1], mouse_data[2]]) as i32
                    } else {
                        0
                    };
                    let y = if mouse_data.len() >= 5 {
                        u16::from_be_bytes([mouse_data[3], mouse_data[4]]) as i32
                    } else {
                        0
                    };
                    self.client_state
                        .add_pending_input(InputEvent::Mouse {
                            data: data_clone,
                            x,
                            y,
                        })
                        .await;
                }
            }
            1 => {
                if data.len() >= 6 {
                    let key_data = &data[4..];
                    let data_clone = key_data.to_vec();
                    self.client_state
                        .add_pending_input(InputEvent::Keyboard { data: data_clone })
                        .await;
                }
            }
            85 => {
                if data.len() >= 7 {
                    let unicode_data = &data[0..];
                    let data_clone = unicode_data.to_vec();
                    self.client_state
                        .add_pending_input(InputEvent::Unicode { data: data_clone })
                        .await;
                }
            }
            10 => {
                println!(" Ctrl+Alt+Del received");
                self.client_state
                    .add_pending_input(InputEvent::CtrlAltDel)
                    .await;
            }
            15 => {
                println!(" Received compression packet");
            }
            5 => {
                if data.len() >= 10 {
                    let quality = data[4];
                    let fps = data[5];
                    let scaling_factor_new = u16::from_be_bytes([data[6], data[7]]) as u32;
                    let _height_param = u16::from_be_bytes([data[8], data[9]]) as u32;
                    println!(
                        " Received resize command: scaling_factor={} (quality={}, fps={})",
                        scaling_factor_new, quality, fps
                    );
                    if scaling_factor_new >= 64 && scaling_factor_new <= 4096 {
                        self.client_state.set_scaling_factor(scaling_factor_new);
                        println!(" Updated scaling factor to {}/1024", scaling_factor_new);
                    }
                    self.client_state.set_compression_quality(quality as u32);
                    self.client_state.set_target_fps(fps as u32);
                    let resolution_packet =
                        create_resolution_packet_with_scaling(&self.client_state);
                    {
                        let mut sender = ws_sender.lock().await;
                        sender.send(Message::Binary(resolution_packet)).await?;
                    }
                    let display_packet =
                        create_display_list_packet_with_scaling(&self.client_state);
                    {
                        let mut sender = ws_sender.lock().await;
                        sender.send(Message::Binary(display_packet)).await?;
                    }
                    self.client_state.set_force_keyframe(true);
                    self.client_state.request_full_refresh().await;
                    println!(
                        " Resize processed: sent resolution packets and requested full refresh"
                    );
                }
            }
            6 => {
                static mut LAST_REFRESH_REQUEST_TIME: Option<Instant> = None;
                unsafe {
                    let now = Instant::now();
                    if let Some(last_time) = LAST_REFRESH_REQUEST_TIME {
                        let elapsed = now.duration_since(last_time);
                        println!(" Refresh request received - time since last: {:?}", elapsed);
                        if elapsed < Duration::from_secs(1) {
                            return Ok(());
                        }
                    }
                    LAST_REFRESH_REQUEST_TIME = Some(now);
                }
                let resolution_packet = create_resolution_packet_with_scaling(&self.client_state);
                {
                    let mut sender = ws_sender.lock().await;
                    sender.send(Message::Binary(resolution_packet)).await?;
                }
                let display_list_packet =
                    create_display_list_packet_with_scaling(&self.client_state);
                {
                    let mut sender = ws_sender.lock().await;
                    sender.send(Message::Binary(display_list_packet)).await?;
                }
                if !self.client_state.is_resolution_changing() {
                    self.client_state.request_full_refresh().await;
                    println!(" Sent resolution and display list packets, requested full refresh");
                } else {
                    self.client_state.set_resolution_changing(false);
                    self.client_state.clear_full_refresh_request().await;
                    println!(" Skipped refresh because resolution is changing");
                }
            }
            8 => {
                let pause_state = data.len() > 5 && data[5] == 1;
                println!(" Pause state: {}", pause_state);
                self.client_state.set_pause(pause_state);
            }
            9 => {
                if data.len() >= 6 {
                    let compression_type = data[4];
                    let compression_level = data[5];
                    println!(
                        " Compression settings received: type={}, level={}",
                        compression_type, compression_level
                    );
                    self.client_state
                        .set_compression_settings(compression_type, compression_level);
                    if data.len() >= 8 {
                        let frame_timer = u16::from_be_bytes([data[6], data[7]]) as u32;
                        if frame_timer >= 20 && frame_timer <= 5000 {
                            self.client_state.set_frame_rate_timer(frame_timer);
                            println!("Frame rate timer updated: {}ms", frame_timer);
                        }
                    }
                    if data.len() >= 10 {
                        let scaling = u16::from_be_bytes([data[8], data[9]]) as u32;
                        if scaling > 0 {
                            self.client_state.set_scaling_factor(scaling);
                            println!(" Scaling factor updated: {}", scaling);
                        }
                    }
                } else {
                    println!(" Invalid compression packet length: {}", data.len());
                }
            }
            12 => {
                if data.len() >= 6 {
                    let display_id = u16::from_be_bytes([data[4], data[5]]) as u32;
                    let current_display = self.client_state.get_selected_display();
                    println!(
                        " Display selection received: {} (current: {})",
                        display_id, current_display
                    );
                    self.client_state.set_selected_display(display_id);
                    if display_id != current_display {
                        println!("Display changed from {} to {}", current_display, display_id);
                    }
                } else {
                    println!(" Invalid display selection packet length: {}", data.len());
                }
            }
            MNG_KVM_FRAME_RATE_TIMER => {
                if data.len() >= 6 {
                    let timer_value = u16::from_be_bytes([data[4], data[5]]) as u32;
                    if timer_value >= 20 && timer_value <= 5000 {
                        println!("Received frame rate timer update: {}ms", timer_value);
                        if let Some(tx) = FRAME_RATE_TIMER_TX.get() {
                            let _ = tx.try_send(timer_value);
                        }
                    }
                }
            }
            MNG_KVM_CURSOR_CONTROL => {
                if data.len() >= 5 {
                    let cursor_enabled = data[4] != 0;
                    println!(
                        "Received cursor control command: {}",
                        if cursor_enabled {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    );
                    self.client_state.set_cursor_visible(cursor_enabled);
                }
            }
            MNG_KVM_INIT_TOUCH => {
                println!(" Received touch init command (type 14) - feature disabled");
                let mut response = Vec::new();
                response.push(MNG_KVM_INIT_TOUCH);
                response.push(0);
                response.push(0);
                response.push(4);
                response.push(2); // 2 = failure / unsupported
                let mut sender = ws_sender.lock().await;
                if let Err(e) = sender.send(Message::Binary(response)).await {
                    println!(" Failed to send touch init response: {}", e);
                } else {
                    println!(" Touch init response sent with failure status");
                }
            }
            MNG_UPDATE_TEMP_WALLPAPER => {
                println!("Received temp wallpaper command (type 73) - wallpaper feature disabled");
            }
            MNG_RESTORE_ORIGINAL_WALLPAPER => {
                println!(
                    "Received restore wallpaper command (type 74) - wallpaper feature disabled"
                );
            }
            144 => {
                if data.len() >= 5 {
                    let cursor_enabled = data[4] != 0;
                    println!(
                        "Received cursor control command (type 144): {}",
                        if cursor_enabled {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    );
                    self.client_state.set_cursor_visible(cursor_enabled);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

pub fn create_keystate_packet() -> Vec<u8> {
    let mut packet = Vec::new();
    packet.extend_from_slice(&(MNG_KVM_KEYSTATE as u16).to_be_bytes());
    packet.extend_from_slice(&(6u16).to_be_bytes());
    packet.push(0);
    packet.push(0);
    packet.push(0);
    packet
}
fn create_mouse_cursor_packet() -> Vec<u8> {
    let mut packet = Vec::new();
    packet.extend_from_slice(&(MNG_KVM_MOUSE_CURSOR as u16).to_be_bytes());
    packet.extend_from_slice(&(8u16).to_be_bytes());
    packet.extend_from_slice(&(0u32).to_be_bytes());
    packet
}
fn create_refresh_packet() -> Vec<u8> {
    let mut refresh_msg = Vec::with_capacity(4);
    refresh_msg.extend_from_slice(&MNG_KVM_REFRESH.to_be_bytes());
    refresh_msg.extend_from_slice(&(4u16).to_be_bytes());
    println!("[DEBUG] Created refresh packet: {:?}", refresh_msg);
    refresh_msg
}
