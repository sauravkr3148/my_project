use crate::error::{Error, Result};
use std::fs;

/// Application configuration
#[derive(Debug, Clone)]
pub struct Config {
    pub server_address: String,
    pub server_port: String,
    pub tenant_id: String,
    pub uuid: String,
    pub proxy_url: Option<String>,
    pub proxy_port: Option<String>,
    pub proxy_auth: Option<String>,
    pub use_proxy: bool,
    pub use_ssl: bool,
    pub no_auth: bool,
}

impl Config {
    /// Load configuration from file
    pub fn load_from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("Failed to read config file '{}': {}", path, e)))?;

        let tokens: Vec<&str> = content.trim().split(' ').collect();

        if tokens.len() < 11 {
            return Err(Error::Config(format!(
                "Config file '{}' doesn't have enough tokens (expected 11, got {})",
                path,
                tokens.len()
            )));
        }

        let server_address = tokens[0].to_string();
        let use_ssl =
            server_address.starts_with("wss://") || server_address.starts_with("https://");

        // Extract hostname/IP from server address
        let clean_server = server_address
            .replace("wss://", "")
            .replace("ws://", "")
            .replace("https://", "")
            .replace("http://", "");

        Ok(Config {
            server_address: clean_server,
            server_port: tokens[1].to_string(),
            tenant_id: tokens[2].to_string(),
            proxy_url: if tokens[3].is_empty() {
                None
            } else {
                Some(tokens[3].to_string())
            },
            proxy_port: if tokens[4].is_empty() {
                None
            } else {
                Some(tokens[4].to_string())
            },
            //Check if proxy_url and proxy_port are present AND not empty
            use_proxy: tokens[7] == "proxy",
            uuid: tokens[8].to_string(),
            proxy_auth: if tokens[9].is_empty() {
                None
            } else {
                Some(tokens[9].to_string())
            },
            no_auth: tokens[10] == "isNoAuth",
            use_ssl,
        })
    }

    /// Build a WebSocket URL for the given agent type
    pub fn get_websocket_url_for(&self, agent_type: &str) -> String {
        let protocol = if self.use_ssl { "wss" } else { "ws" };
        let port = if self.server_port.is_empty() {
            if self.use_ssl {
                "443"
            } else {
                "80"
            }
        } else {
            &self.server_port
        };

        format!(
            "{}://{}:{}/ws/rev/{}/{}/{}",
            protocol, self.server_address, port, agent_type, self.tenant_id, self.uuid
        )
    }

    /// Default WebSocket URL for the file agent
    pub fn get_websocket_url(&self) -> String {
        self.get_websocket_url_for("file_agent")
    }
}
