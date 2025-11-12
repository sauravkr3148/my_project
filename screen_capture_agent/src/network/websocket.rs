use super::client_state::ClientState;
use super::message_handler::MessageHandler;
use super::protocol::{
    create_agent_connected_packet, create_display_list_packet_with_scaling,
    create_resolution_packet_with_scaling, create_video_frame_packet,
};
use crate::network::blocking_capture_thread::CaptureThreadHandle;
use crate::network::input_processor::InputProcessor;
use crate::network::protocol::get_codec_type_id;
use crate::qos::VideoQoS;
use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, StreamExt};
use native_tls::TlsConnector as NativeTlsConnector;
use rust_c1rmm_agent::config::Config as AgentConfig;
use rust_c1rmm_agent::network::handlers::MessageHandler as FileMessageHandler;
use rust_c1rmm_agent::network::proxy::ProxyConnector;
#[cfg(target_os = "windows")]
use scrap::dxgi::gdi;
use scrap::CodecFormat;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tokio_native_tls::TlsConnector as TokioTlsConnector;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{
    connect_async, connect_async_tls_with_config, tungstenite::client::IntoClientRequest,
    Connector, MaybeTlsStream, WebSocketStream,
};
use url::Url;

const AGENT_TYPE: &str = "c_agent";

pub struct WebSocketClient {
    config: AgentConfig,
    running: bool,
    video_codec: CodecFormat,
    frame_send_counter: Arc<AtomicU32>,
    last_frame_time: Arc<Mutex<Instant>>,
    connection_stable: Arc<AtomicBool>,
    consecutive_errors: Arc<AtomicU32>,
    last_keyframe_time: Arc<Mutex<Instant>>,
}

impl WebSocketClient {
    pub fn new(config: AgentConfig) -> Self {
        Self {
            config,
            running: false,
            // CRITICAL FIX: VP8 is 3-5x faster than VP9 for real-time encoding
            video_codec: CodecFormat::VP8,
            frame_send_counter: Arc::new(AtomicU32::new(0)),
            last_frame_time: Arc::new(Mutex::new(Instant::now())),
            connection_stable: Arc::new(AtomicBool::new(false)),
            consecutive_errors: Arc::new(AtomicU32::new(0)),
            last_keyframe_time: Arc::new(Mutex::new(Instant::now())),
        }
    }

    pub fn with_codec(mut self, codec: CodecFormat) -> Self {
        self.video_codec = codec;
        self
    }

    pub async fn connect_and_stream(&mut self) -> Result<()> {
        let mut retry_count: usize = 0;

        loop {
            match self.try_connect_and_stream().await {
                Ok(_) => {
                    self.connection_stable.store(true, Ordering::Relaxed);
                    self.consecutive_errors.store(0, Ordering::Relaxed);

                    if should_shutdown() {
                        println!("Shutdown requested, stopping agent loop");
                        return Ok(());
                    }

                    retry_count = 0;
                    println!("Connection ended, attempting to reconnect...");
                }
                Err(e) => {
                    retry_count += 1;
                    self.connection_stable.store(false, Ordering::Relaxed);
                    self.consecutive_errors.fetch_add(1, Ordering::Relaxed);

                    let backoff_duration =
                        Duration::from_secs(std::cmp::min(retry_count.max(1) as u64 * 2, 30));
                    println!(
                        "Connection error (attempt {}): {}. Retrying in {} seconds...",
                        retry_count,
                        e,
                        backoff_duration.as_secs()
                    );
                    sleep(backoff_duration).await;
                }
            }
        }
    }

    async fn try_connect_and_stream(&mut self) -> Result<()> {
        let url = self.config.get_websocket_url_for(AGENT_TYPE);
        println!("Connecting to: {}", url);

        let url = Url::parse(&url).context("Invalid WebSocket URL")?;
        let ws_stream = self.establish_connection(&url).await?;
        self.handle_connected_stream(ws_stream).await
    }

    async fn establish_connection(
        &self,
        url: &Url,
    ) -> Result<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>> {
        if self.config.use_proxy {
            self.connect_through_proxy(url).await
        } else {
            self.connect_direct(url).await
        }
    }

    async fn connect_direct(
        &self,
        url: &Url,
    ) -> Result<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>> {
        if self.config.use_ssl {
            let req = url.clone().into_client_request()?;
            let tls_connector = NativeTlsConnector::builder()
                .danger_accept_invalid_certs(true)
                .danger_accept_invalid_hostnames(true)
                .build()
                .context("failed to build TLS connector")?;
            let connector = Connector::NativeTls(tls_connector);
            let (ws_stream, response) =
                connect_async_tls_with_config(req, None, false, Some(connector))
                    .await
                    .context("TLS WebSocket handshake failed")?;
            println!(
                "Secure WebSocket handshake successful! Response: {:?}",
                response.status()
            );
            Ok(ws_stream)
        } else {
            let (ws_stream, response) = connect_async(url.clone())
                .await
                .context("WebSocket handshake failed")?;
            println!(
                "WebSocket handshake successful! Response: {:?}",
                response.status()
            );
            Ok(ws_stream)
        }
    }

    async fn connect_through_proxy(
        &self,
        url: &Url,
    ) -> Result<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>> {
        let host = url.host_str().unwrap_or("localhost").to_string();
        let port =
            url.port_or_known_default()
                .unwrap_or_else(|| if self.config.use_ssl { 443 } else { 80 });

        let proxy_port: u16 = self
            .config
            .proxy_port
            .as_ref()
            .ok_or_else(|| anyhow!("Proxy port not configured"))?
            .parse()
            .context("Invalid proxy port")?;

        let proxy_host = self
            .config
            .proxy_url
            .clone()
            .ok_or_else(|| anyhow!("Proxy host not configured"))?;

        let proxy_connector = ProxyConnector::new(
            proxy_host,
            proxy_port,
            host.clone(),
            port,
            self.config.proxy_auth.clone(),
            self.config.no_auth,
        );

        let std_stream = proxy_connector
            .connect()
            .map_err(|e| anyhow!(e.to_string()))
            .context("Failed to establish proxy tunnel")?;

        std_stream
            .set_nonblocking(true)
            .context("Failed to configure proxy stream")?;

        let tokio_stream =
            tokio::net::TcpStream::from_std(std_stream).context("Failed to wrap proxy stream")?;

        let stream = if self.config.use_ssl {
            let mut builder = native_tls::TlsConnector::builder();
            builder.danger_accept_invalid_certs(true);
            builder.danger_accept_invalid_hostnames(true);
            let tls_connector = builder
                .build()
                .context("Failed to build native TLS for proxy")?;
            let tls_connector = TokioTlsConnector::from(tls_connector);
            let tls_stream = tls_connector
                .connect(&host, tokio_stream)
                .await
                .context("TLS handshake over proxy failed")?;
            MaybeTlsStream::NativeTls(tls_stream)
        } else {
            MaybeTlsStream::Plain(tokio_stream)
        };

        let (ws_stream, _) = tokio_tungstenite::client_async(url, stream)
            .await
            .context("WebSocket handshake through proxy failed")?;

        println!("WebSocket handshake through proxy successful!");
        Ok(ws_stream)
    }

    async fn handle_connected_stream(
        &mut self,
        ws_stream: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    ) -> Result<()> {
        let (mut write, read) = ws_stream.split();
        println!("WebSocket connection established - waiting for client connections");
        let client_state = Arc::new(ClientState::new());
        #[cfg(target_os = "windows")]
        {
            gdi::set_client_state_provider(client_state.clone());
        }

        let screen_message_handler = MessageHandler::new(client_state.clone());
        let file_message_handler = FileMessageHandler::new();
        let (handshake_tx, _handshake_rx) = flume::unbounded::<bool>();

        println!(
            " WebSocket connection ready - blocking capture thread will handle all capture/encode"
        );

        let agent_connected_packet = create_agent_connected_packet();
        write
            .send(Message::Binary(agent_connected_packet))
            .await
            .context("Failed to send agent_connected packet")?;

        let resolution_packet = create_resolution_packet_with_scaling(&client_state);
        write
            .send(Message::Binary(resolution_packet))
            .await
            .context("Failed to send resolution packet")?;

        let display_list_packet = create_display_list_packet_with_scaling(&client_state);
        write
            .send(Message::Binary(display_list_packet))
            .await
            .context("Failed to send display list packet")?;

        let write_half = Arc::new(Mutex::new(write));
        let write_half_for_messages = Arc::clone(&write_half);
        let write_half_for_video = Arc::clone(&write_half);

        let (video_tx, mut video_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(30);

        let video_qos = Arc::new(std::sync::Mutex::new(VideoQoS::new()));
        let (capture_thread, frame_receiver) =
            CaptureThreadHandle::spawn(self.video_codec, video_qos.clone());

        self.running = true;

        let writer_for_messages = Arc::clone(&write_half_for_messages);
        let file_handler_for_messages = file_message_handler.clone();
        let screen_handler = screen_message_handler;
        let handshake_sender = handshake_tx.clone();

        let message_task = tokio::spawn(async move {
            let mut read_stream = read;
            while let Some(msg) = read_stream.next().await {
                match msg {
                    Ok(Message::Binary(data)) => {
                        if let Err(e) = file_handler_for_messages
                            .handle_binary_message(&data, &writer_for_messages)
                            .await
                        {
                            println!("Error handling file-agent binary message: {}", e);
                        }

                        if let Err(e) = screen_handler
                            .handle_message(data, &writer_for_messages, &handshake_sender)
                            .await
                        {
                            println!("Error handling screen message: {}", e);
                        }
                    }
                    Ok(Message::Text(text)) => {
                        if let Err(e) = file_handler_for_messages
                            .handle_text_message(&text, &writer_for_messages)
                            .await
                        {
                            println!("Error handling file-agent text message: {}", e);
                        }
                    }
                    Ok(Message::Ping(data)) => {
                        let mut writer = writer_for_messages.lock().await;
                        if let Err(e) = writer.send(Message::Pong(data)).await {
                            println!("Error sending pong: {}", e);
                            break;
                        }
                    }
                    Ok(Message::Pong(_)) => {}
                    Ok(Message::Close(_)) => {
                        println!("Connection closed by remote");
                        break;
                    }
                    Err(e) => {
                        println!("Error receiving message: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
            println!("Message handling loop exited");
        });

        let frame_counter = Arc::clone(&self.frame_send_counter);
        let write_half_for_video_clone = Arc::clone(&write_half_for_video);
        let video_send_task = tokio::spawn(async move {
            let mut send_errors = 0u32;
            let mut dropped_by_timeout = 0u64;
            while let Some(packet) = video_rx.recv().await {
                let mut write_guard = write_half_for_video_clone.lock().await;
                let send_start = Instant::now();

                match tokio::time::timeout(
                    Duration::from_millis(10),
                    write_guard.send(Message::Binary(packet)),
                )
                .await
                {
                    Ok(Ok(_)) => {
                        frame_counter.fetch_add(1, Ordering::Relaxed);
                        send_errors = 0;
                        let delay = send_start.elapsed();
                        if delay.as_millis() > 50 {
                            println!(" SLOW WEBSOCKET SEND: {}ms", delay.as_millis());
                        }
                    }
                    Ok(Err(e)) => {
                        send_errors += 1;
                        if send_errors % 10 == 1 {
                            println!(" Video send error: {}", e);
                        }
                        if send_errors > 50 {
                            println!(" Too many video send errors, exiting");
                            break;
                        }
                    }
                    Err(_) => {
                        dropped_by_timeout += 1;
                        if dropped_by_timeout % 10 == 1 {
                            println!(
                                "Dropped {} frames by timeout (client receiving too slow)",
                                dropped_by_timeout
                            );
                        }
                    }
                }
            }
            println!("Video send loop exited");
        });

        let client_state_for_input = client_state.clone();

        tokio::spawn(async move {
            let mut input_processor = InputProcessor::new(client_state_for_input);
            loop {
                input_processor.process_pending_inputs().await;
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        });

        let client_state_for_bridge = client_state.clone();
        let video_tx_for_bridge = video_tx.clone();
        let capture_thread_handle = Arc::new(parking_lot::Mutex::new(capture_thread));
        let capture_thread_clone = capture_thread_handle.clone();
        let codec_for_bridge = self.video_codec.clone();

        let bridge_task = tokio::task::spawn_blocking(move || {
            let mut frame_counter = 0u64;
            let mut dropped_frames = 0u64;
            let mut last_stats = Instant::now();
            let codec_id = get_codec_type_id(&codec_for_bridge);

            println!(" Bridge task started - receiving from blocking thread");

            while let Ok(frame_data) = frame_receiver.recv() {
                let clients = client_state_for_bridge.get_active_client_count();

                if let Some(handle) = capture_thread_clone.try_lock() {
                    handle.set_active_clients(clients as usize);
                }

                if client_state_for_bridge.has_clients_needing_initial_frame() {
                    if !frame_data.is_keyframe {
                        if let Some(handle) = capture_thread_clone.try_lock() {
                            handle.set_force_keyframe();
                        }
                        continue;
                    } else {
                        println!(" FIRST KEYFRAME: {} bytes", frame_data.data.len());
                        client_state_for_bridge.mark_client_initial_frame_sent();
                    }
                }

                if client_state_for_bridge.is_force_keyframe() {
                    if let Some(handle) = capture_thread_clone.try_lock() {
                        handle.set_force_keyframe();
                    }
                    client_state_for_bridge.set_force_keyframe(false);
                }

                let packet =
                    create_video_frame_packet(&frame_data.data, codec_id, frame_data.is_keyframe);

                match video_tx_for_bridge.try_send(packet) {
                    Ok(_) => {
                        frame_counter += 1;
                    }
                    Err(_) => {
                        dropped_frames += 1;
                        if dropped_frames % 30 == 0 {
                            println!(" Dropped {} frames (client too slow)", dropped_frames);
                        }
                    }
                }

                if last_stats.elapsed().as_secs() >= 5 {
                    let fps = frame_counter as f32 / last_stats.elapsed().as_secs_f32();
                    println!(
                        " BRIDGE: {:.1} FPS sent to client, {} dropped",
                        fps, dropped_frames
                    );
                    frame_counter = 0;
                    dropped_frames = 0;
                    last_stats = Instant::now();
                }
            }

            println!(" Bridge task exited");
        });

        let mut last_check = Instant::now();
        while self.running {
            tokio::time::sleep(Duration::from_millis(100)).await;

            let clients = client_state.get_active_client_count();
            if let Some(handle) = capture_thread_handle.try_lock() {
                handle.set_active_clients(clients as usize);
            }

            if should_shutdown() {
                println!(" Shutdown signal received");
                self.running = false;
                break;
            }

            if last_check.elapsed().as_secs() >= 10 {
                println!("Connection alive, {} active clients", clients);
                last_check = Instant::now();
            }
        }
        println!("Streaming loop ended");

        if let Some(mut handle) = capture_thread_handle.try_lock() {
            handle.stop();
        }

        let _ = bridge_task.await;

        let total_frames = self.frame_send_counter.load(Ordering::Relaxed);
        let total_errors = self.consecutive_errors.load(Ordering::Relaxed);
        println!(
            "Final stats: {} frames sent, {} errors encountered",
            total_frames, total_errors
        );
        message_task.abort();
        video_send_task.abort();
        self.running = false;
        Ok(())
    }
}

pub async fn connect_and_stream_with_encoder(codec: CodecFormat) -> Result<()> {
    let config = AgentConfig::load_from_file("config.txt")
        .map_err(|e| anyhow!(e.to_string()))
        .context("Failed to load configuration")?;
    let mut client = WebSocketClient::new(config).with_codec(codec);
    client.connect_and_stream().await
}

pub async fn connect_and_stream_default() -> Result<()> {
    // CRITICAL FIX: Use VP8 instead of VP9 for 3-5x better performance
    // VP8 with cpuused=12 achieves 30fps on 4-core systems
    // VP9 with cpuused=7 only achieves 10-15fps on same hardware
    connect_and_stream_with_encoder(CodecFormat::VP8).await
}

pub static SHUTDOWN_FLAG: AtomicBool = AtomicBool::new(false);

pub fn should_shutdown() -> bool {
    SHUTDOWN_FLAG.load(Ordering::Relaxed)
}
