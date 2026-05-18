use std::path::PathBuf;

use tokio::sync::oneshot;

use crate::transport::{TransportCmd, TransportReply};

/// All input events the daemon event loop handles.
/// Sent over a bounded tokio::sync::mpsc channel from producers (watcher, timeline, transport).
pub enum DaemonEvent {
    /// A watched file changed. Path is the absolute path to the changed file.
    FileChanged(PathBuf),
    /// Timeline tick. Payload is current playback position in seconds.
    Tick(f64),
    /// Transport command from CLI client, with a oneshot reply channel.
    Transport(TransportCmd, oneshot::Sender<TransportReply>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_changed_carries_path() {
        let event = DaemonEvent::FileChanged(PathBuf::from("/tmp/piece.hum"));
        match event {
            DaemonEvent::FileChanged(p) => assert_eq!(p, PathBuf::from("/tmp/piece.hum")),
            _ => panic!("expected FileChanged"),
        }
    }

    #[test]
    fn tick_carries_position() {
        let event = DaemonEvent::Tick(10.5);
        match event {
            DaemonEvent::Tick(pos) => assert!((pos - 10.5).abs() < f64::EPSILON),
            _ => panic!("expected Tick"),
        }
    }
}
