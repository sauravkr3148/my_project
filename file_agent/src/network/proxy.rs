use crate::error::{Error, Result};
use log::info;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

pub struct ProxyConnector {
    proxy_host: String,
    proxy_port: u16,
    target_host: String,
    target_port: u16,
    auth: Option<String>,
    no_auth: bool,
}

impl ProxyConnector {
    pub fn new(
        proxy_host: String,
        proxy_port: u16,
        target_host: String,
        target_port: u16,
        auth: Option<String>,
        no_auth: bool,
    ) -> Self {
        Self {
            proxy_host,
            proxy_port,
            target_host,
            target_port,
            auth,
            no_auth,
        }
    }

    pub fn connect(&self) -> Result<TcpStream> {
        info!(
            "Connecting to proxy {}:{}",
            self.proxy_host, self.proxy_port
        );

        let mut stream = TcpStream::connect(format!("{}:{}", self.proxy_host, self.proxy_port))
            .map_err(|e| Error::Network(format!("Failed to connect to proxy: {}", e)))?;

        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;

        // Send CONNECT request
        let connect_request = if self.no_auth {
            format!(
                "CONNECT {}:{} HTTP/1.1\r\n\
                 Host: {}:{}\r\n\
                 Connection: keep-alive\r\n\r\n",
                self.target_host, self.target_port, self.target_host, self.target_port
            )
        } else if let Some(ref auth) = self.auth {
            format!(
                "CONNECT {}:{} HTTP/1.1\r\n\
                 Host: {}:{}\r\n\
                 Proxy-Authorization: Basic {}\r\n\
                 Connection: keep-alive\r\n\r\n",
                self.target_host, self.target_port, self.target_host, self.target_port, auth
            )
        } else {
            return Err(Error::Config(
                "Proxy authentication required but not provided".to_string(),
            ));
        };

        stream
            .write_all(connect_request.as_bytes())
            .map_err(|e| Error::Network(format!("Failed to send CONNECT request: {}", e)))?;

        // Read response
        let mut response = [0u8; 4096];
        let bytes_read = stream
            .read(&mut response)
            .map_err(|e| Error::Network(format!("Failed to read proxy response: {}", e)))?;

        let response_str = String::from_utf8_lossy(&response[..bytes_read]);
        info!(
            "Proxy response: {}",
            response_str.lines().next().unwrap_or("")
        );

        if !response_str.contains("200") {
            return Err(Error::Network(format!(
                "Proxy CONNECT failed: {}",
                response_str.lines().next().unwrap_or("Unknown error")
            )));
        }

        println!("Successfully connected through proxy");
        Ok(stream)
    }
}
