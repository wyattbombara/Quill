use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error("Local database: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Network request: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("System keychain: {0}")]
    Keychain(#[from] keyring::Error),
    #[error("Desktop operation: {0}")]
    Desktop(#[from] tauri::Error),
    #[error("Background task: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("Reply stopped")]
    Cancelled,
}
impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
pub type Result<T> = std::result::Result<T, Error>;
pub fn invalid(message: impl Into<String>) -> Error {
    Error::Message(message.into())
}
