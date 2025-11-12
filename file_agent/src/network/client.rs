use crate::{
    config::Config,
    error::{Error, Result},
    network::{handlers::MessageHandler, proxy::ProxyConnector},
};
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::sleep;
use tokio_tungstenite::{
    connect_async_tls_with_config, tungstenite::Message, Connector, MaybeTlsStream, WebSocketStream,
};
use url::Url;

type WebSocketWriter = Arc<
    Mutex<
        futures_util::stream::SplitSink<
            WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
            Message,
        >,
    >,
>;

#[derive(Clone)]
pub struct WebSocketClient {
    config: Arc<Config>,
    message_handler: MessageHandler,
    should_reconnect: Arc<RwLock<bool>>,
}

impl WebSocketClient {
    pub fn new(config: Arc<Config>) -> Self {
        let message_handler = MessageHandler::new();
        Self {
            config,
            message_handler,
            should_reconnect: Arc::new(RwLock::new(true)),
        }
    }

    pub async fn stop_reconnection(&self) {
        let mut should_reconnect = self.should_reconnect.write().await;
        *should_reconnect = false;
    }

    pub async fn connect(&self) -> Result<()> {
        info!("Starting persistent WebSocket connection with automatic reconnection");

        loop {
            // Check if we should continue reconnecting
            {
                let should_reconnect = self.should_reconnect.read().await;
                if !*should_reconnect {
                    info!("Reconnection stopped by user request");
                    break;
                }
            }

            match self.try_connect().await {
                Ok(()) => {
                    info!("WebSocket connection established successfully");
                    // Connection was successful but closed, continue reconnecting
                    warn!("Connection closed, attempting to reconnect...");
                }
                Err(e) => {
                    error!("Connection failed: {}. Retrying in 1 second...", e);
                }
            }

            sleep(Duration::from_secs(1)).await;
        }

        Ok(())
    }

    async fn try_connect(&self) -> Result<()> {
        let url = Url::parse(&self.config.get_websocket_url())
            .map_err(|e| Error::Config(format!("Invalid WebSocket URL: {}", e)))?;

        let ws_stream = if self.config.use_proxy {
            self.connect_through_proxy(&url).await?
        } else {
            self.connect_direct(&url).await?
        };

        self.handle_connection(ws_stream).await
    }

    async fn connect_direct(
        &self,
        url: &Url,
    ) -> Result<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>> {
        let tls_connector = if self.config.use_ssl {
            let mut builder = native_tls::TlsConnector::builder();
            builder.danger_accept_invalid_certs(true);
            builder.danger_accept_invalid_hostnames(true);
            Some(Connector::NativeTls(builder.build().unwrap()))
        } else {
            None
        };

        let (ws_stream, _) = connect_async_tls_with_config(url, None, false, tls_connector)
            .await
            .map_err(|e| Error::Network(format!("WebSocket connection failed: {}", e)))?;

        Ok(ws_stream)
    }

    async fn connect_through_proxy(
        &self,
        url: &Url,
    ) -> Result<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>> {
        let host = url.host_str().unwrap_or("localhost");
        let port = url
            .port()
            .unwrap_or(if self.config.use_ssl { 443 } else { 80 });

        // Parse proxy port
        let proxy_port: u16 = self
            .config
            .proxy_port
            .as_ref()
            .ok_or_else(|| Error::Config("Proxy port not configured".to_string()))?
            .parse()
            .map_err(|e| Error::Config(format!("Invalid proxy port: {}", e)))?;

        // Create proxy connector
        let proxy_connector = ProxyConnector::new(
            self.config.proxy_url.clone().unwrap_or_default(),
            proxy_port,
            host.to_string(),
            port,
            self.config.proxy_auth.clone(),
            self.config.no_auth,
        );

        // Connect through proxy
        let std_stream = proxy_connector.connect()?;

        std_stream.set_nonblocking(true)?;
        let tokio_stream = tokio::net::TcpStream::from_std(std_stream)?;

        // Handle TLS if needed
        let stream = if self.config.use_ssl {
            let mut builder = native_tls::TlsConnector::builder();
            builder.danger_accept_invalid_certs(true);
            builder.danger_accept_invalid_hostnames(true);
            let tls_connector = tokio_native_tls::TlsConnector::from(builder.build().unwrap());

            let tls_stream = tls_connector
                .connect(host, tokio_stream)
                .await
                .map_err(|e| Error::Network(format!("TLS handshake failed: {}", e)))?;

            MaybeTlsStream::NativeTls(tls_stream)
        } else {
            MaybeTlsStream::Plain(tokio_stream)
        };

        // Perform WebSocket handshake
        let (ws_stream, _) = tokio_tungstenite::client_async(url, stream)
            .await
            .map_err(|e| Error::Network(format!("WebSocket handshake failed: {}", e)))?;

        Ok(ws_stream)
    }

    async fn handle_connection(
        &self,
        ws_stream: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    ) -> Result<()> {
        info!("WebSocket connection established, starting message loop");

        let (writer, mut reader) = ws_stream.split();
        let writer = Arc::new(Mutex::new(writer));

        while let Some(msg) = reader.next().await {
            // Check if we should continue processing
            {
                let should_reconnect = self.should_reconnect.read().await;
                if !*should_reconnect {
                    info!("Stopping connection due to shutdown request");
                    break;
                }
            }

            match msg {
                Ok(Message::Binary(data)) => {
                    if let Err(e) = self
                        .message_handler
                        .handle_binary_message(&data, &writer)
                        .await
                    {
                        error!("Error handling binary message: {}", e);
                        // Continue processing other messages
                    }
                }
                Ok(Message::Text(text)) => {
                    if let Err(e) = self
                        .message_handler
                        .handle_text_message(&text, &writer)
                        .await
                    {
                        error!("Error handling text message: {}", e);
                        // Continue processing other messages
                    }
                }
                Ok(Message::Close(_)) => {
                    warn!("WebSocket connection closed by server - will reconnect");
                    break;
                }
                Ok(Message::Ping(data)) => {
                    let mut writer = writer.lock().await;
                    if let Err(e) = writer.send(Message::Pong(data)).await {
                        error!("Error sending pong: {} - connection may be lost", e);
                        break;
                    }
                }
                Ok(Message::Pong(_)) => {
                    // Ignore pong messages
                }
                Ok(Message::Frame(_)) => {
                    // Ignore frame messages
                }
                Err(e) => {
                    error!("Error receiving message: {} - connection lost", e);
                    break;
                }
            }
        }

        info!("Connection loop ended, returning for reconnection");
        Ok(())
    }
}
