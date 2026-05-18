use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, oneshot};

use crate::events::DaemonEvent;

pub const SOCKET_PATH: &str = "/tmp/hum.sock";

// ---------------------------------------------------------------------------
// Protocol types (JSON newline-delimited)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum TransportCmd {
    Play,
    Stop,
    Status,
    Seek { pos: f64 },
    PlayFrom { pos: f64 },
    Loop { start: f64, end: f64 },
    Solo { thing: String },
    Mute { thing: String },
    DictList,
    DictShow { term: String },
    DictAdd { thing: String, term: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "ok", rename_all = "snake_case")]
pub enum TransportReply {
    Ack,
    Status {
        playing: bool,
        pos: f64,
        active: Vec<String>,
        solo: Vec<String>,
        mute: Vec<String>,
        amplitudes: HashMap<String, f32>,
    },
    Error {
        message: String,
    },
    DictVocab {
        terms: Vec<String>,
    },
    DictEntry {
        term: String,
        synth: String,
        context: Option<String>,
    },
    DictAdded {
        term: String,
    },
}

// ---------------------------------------------------------------------------
// Server (daemon side)
// ---------------------------------------------------------------------------

/// Start the Unix socket server. Removes stale socket, binds, and spawns
/// a task that accepts connections. Each connection reads one JSON command,
/// forwards it through the event channel, waits for a reply, and responds.
pub async fn start_socket_server(tx: mpsc::Sender<DaemonEvent>) {
    // Remove stale socket file
    let _ = std::fs::remove_file(SOCKET_PATH);

    let listener = match UnixListener::bind(SOCKET_PATH) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("failed to bind unix socket at {}: {}", SOCKET_PATH, e);
            return;
        }
    };

    tracing::info!("transport: listening on {}", SOCKET_PATH);

    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let tx = tx.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, tx).await {
                        tracing::warn!("transport connection error: {}", e);
                    }
                });
            }
            Err(e) => {
                tracing::error!("transport accept error: {}", e);
            }
        }
    }
}

/// Handle a single client connection: read one JSON line, dispatch, reply.
async fn handle_connection(
    stream: UnixStream,
    tx: mpsc::Sender<DaemonEvent>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    buf_reader.read_line(&mut line).await?;
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }

    let cmd: TransportCmd = serde_json::from_str(line)?;

    // Create oneshot channel for the reply
    let (reply_tx, reply_rx) = oneshot::channel();

    // Send through event channel
    tx.send(DaemonEvent::Transport(cmd, reply_tx)).await?;

    // Wait for reply from the event loop
    let reply = reply_rx.await.unwrap_or(TransportReply::Error {
        message: "event loop dropped reply channel".to_string(),
    });

    let reply_json = serde_json::to_string(&reply)?;
    writer.write_all(reply_json.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Client (CLI side)
// ---------------------------------------------------------------------------

/// Connect to the daemon socket, send a command, and read the reply.
/// Exits with a clear message if the daemon is not running.
pub async fn send_cmd(cmd: TransportCmd) -> Result<TransportReply> {
    let stream = match UnixStream::connect(SOCKET_PATH).await {
        Ok(s) => s,
        Err(_) => {
            eprintln!("hum-rt is not running");
            std::process::exit(1);
        }
    };

    let (reader, mut writer) = stream.into_split();

    let cmd_json = serde_json::to_string(&cmd)?;
    writer.write_all(cmd_json.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;

    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();
    buf_reader.read_line(&mut line).await?;

    let reply: TransportReply = serde_json::from_str(line.trim())?;
    Ok(reply)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dict_list_serializes_correctly() {
        let cmd = TransportCmd::DictList;
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"cmd\":\"dict_list\""), "got: {}", json);
    }

    #[test]
    fn dict_show_serializes_correctly() {
        let cmd = TransportCmd::DictShow { term: "laser".to_string() };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"cmd\":\"dict_show\""), "got: {}", json);
        assert!(json.contains("\"term\":\"laser\""), "got: {}", json);
    }

    #[test]
    fn dict_vocab_reply_roundtrips() {
        let reply = TransportReply::DictVocab {
            terms: vec!["laser".to_string(), "warm".to_string()],
        };
        let json = serde_json::to_string(&reply).unwrap();
        let parsed: TransportReply = serde_json::from_str(&json).unwrap();
        match parsed {
            TransportReply::DictVocab { terms } => {
                assert_eq!(terms, vec!["laser", "warm"]);
            }
            _ => panic!("expected DictVocab, got {:?}", json),
        }
    }

    #[test]
    fn dict_entry_reply_roundtrips() {
        let reply = TransportReply::DictEntry {
            term: "laser".to_string(),
            synth: "Sine".to_string(),
            context: Some("bright, cutting, sci-fi".to_string()),
        };
        let json = serde_json::to_string(&reply).unwrap();
        let parsed: TransportReply = serde_json::from_str(&json).unwrap();
        match parsed {
            TransportReply::DictEntry { term, synth, context } => {
                assert_eq!(term, "laser");
                assert_eq!(synth, "Sine");
                assert_eq!(context, Some("bright, cutting, sci-fi".to_string()));
            }
            _ => panic!("expected DictEntry, got {:?}", json),
        }
    }
}
