use thiserror::Error;

#[derive(Debug, Error)]
pub enum OscBridgeError {
    #[error("sync timeout waiting for /synced {0}")]
    SyncTimeout(i32),
    #[error("OSC encode error: {0}")]
    EncodeError(String),
    #[error("socket error: {0}")]
    SocketError(#[from] std::io::Error),
    #[error("scsynth unreachable at configured host (timeout 2s)")]
    Unreachable,
    #[error("no node registered for thing: {0}")]
    UnknownThing(String),
}
