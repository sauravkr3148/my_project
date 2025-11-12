use std::fs;
#[derive(Debug, Clone)]
pub struct Config {
    pub server_address: String,
    pub server_port: String,
    pub tenant_id: String,
    pub proxy_ip: Option<String>,
    pub proxy_port: Option<String>,
    pub use_proxy: String,
    pub uuid: String,
    pub proxy_auth: Option<String>,
    pub no_auth: bool,
    pub use_ssl: bool,
}
impl Config {
    pub fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let tokens: Vec<&str> = content.trim().split(' ').collect();
        if tokens.len() < 11 {
            return Err("Config file doesn't have enough tokens".into());
        }
        let server_address = tokens[0].to_string();
        let use_ssl =
            server_address.starts_with("wss://") || server_address.starts_with("https://");
        let server_ip = server_address
            .replace("wss://", "")
            .replace("ws://", "")
            .replace("https://", "")
            .replace("http://", "");
        Ok(Config {
            server_address: server_ip,
            server_port: tokens[1].to_string(),
            tenant_id: tokens[2].to_string(),
            proxy_ip: if tokens[3].is_empty() {
                None
            } else {
                Some(tokens[3].to_string())
            },
            proxy_port: if tokens[4].is_empty() {
                None
            } else {
                Some(tokens[4].to_string())
            },
            use_proxy: tokens[7].to_string(),
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
    #[inline]
    pub fn get_websocket_url(&self) -> String {
        let protocol = if self.use_ssl { "wss" } else { "ws" };
        let port = match self.server_port.as_str() {
            "" => {
                if self.use_ssl {
                    "443"
                } else {
                    "80"
                }
            }
            s => s,
        };
        format!(
            "{}://{}:{}/websocket/{}/{}",
            protocol, self.server_address, port, self.tenant_id, self.uuid
        )
    }
    pub fn has_proxy(&self) -> bool {
        self.proxy_ip.is_some() && self.proxy_port.is_some() && self.use_proxy != "none"
    }
    pub fn get_proxy_url(&self) -> Option<String> {
        if self.has_proxy() {
            if let (Some(ip), Some(port)) = (&self.proxy_ip, &self.proxy_port) {
                return Some(format!("{}:{}", ip, port));
            }
        }
        None
    }
    pub fn get_proxy_auth(&self) -> Option<String> {
        self.proxy_auth.clone()
    }
}
