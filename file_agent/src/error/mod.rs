use thiserror::Error;

/// Application-specific error types
#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("File system error: {0}")]
    FileSystem(String),

    #[error("System error: {0}")]
    System(String),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("URL parsing error: {0}")]
    Url(#[from] url::ParseError),

    #[error("Base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

/// Application result type
pub type Result<T> = std::result::Result<T, Error>;

/// Convert string errors to our error type
impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::System(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::System(s.to_string())
    }
}
