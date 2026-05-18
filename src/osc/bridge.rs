use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use rosc::{decoder, encoder, OscMessage, OscPacket, OscType};
use tokio::net::UdpSocket;
use tokio::time::timeout;

use super::error::OscBridgeError;

pub struct ScsynthClient {
    socket: UdpSocket,
    nodes: HashMap<String, i32>,
    next_node_id: i32,
    next_sync_id: i32,
}

impl ScsynthClient {
    /// Connect to scsynth at the given address (e.g. "127.0.0.1:57110").
    /// Binds a local UDP socket on an ephemeral port.
    pub async fn connect(addr: &str) -> Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        socket.connect(addr).await?;
        Ok(Self {
            socket,
            nodes: HashMap::new(),
            next_node_id: 1000, // 0 and 1 are reserved scsynth groups
            next_sync_id: 1,
        })
    }

    /// Send /status and await /status.reply within 2 seconds.
    /// Returns OscBridgeError::Unreachable on timeout.
    pub async fn check_alive(&self) -> Result<()> {
        self.send_message("/status", vec![]).await?;
        let mut buf = vec![0u8; 4096];
        match timeout(Duration::from_secs(2), self.socket.recv(&mut buf)).await {
            Ok(Ok(n)) => {
                if let Ok((_, OscPacket::Message(msg))) = decoder::decode_udp(&buf[..n]) {
                    if msg.addr == "/status.reply" {
                        tracing::info!("scsynth alive: {:?}", msg.args);
                        return Ok(());
                    }
                }
                Err(OscBridgeError::Unreachable.into())
            }
            Ok(Err(e)) => Err(OscBridgeError::SocketError(e).into()),
            Err(_) => Err(OscBridgeError::Unreachable.into()),
        }
    }

    /// Create the default group (node 1) if it doesn't exist.
    /// Headless scsynth doesn't create this automatically — SC IDE does.
    pub async fn ensure_default_group(&self) -> Result<()> {
        // /g_new groupID=1 addAction=0(addToHead) targetID=0(root)
        self.send_message("/g_new", vec![
            OscType::Int(1),
            OscType::Int(0),
            OscType::Int(0),
        ]).await?;
        tracing::debug!("ensured default group (node 1)");
        Ok(())
    }

    /// Load a SynthDef binary via /d_recv + /sync + await /synced handshake.
    /// CRITICAL: Never send /s_new before /synced arrives.
    /// CRITICAL: Never treat /done from /d_recv as success (SC bug #4411).
    pub async fn load_synthdef(&mut self, synthdef_bytes: Vec<u8>) -> Result<()> {
        // 1. Send /d_recv with SynthDef bytes as OSC Blob
        self.send_message("/d_recv", vec![OscType::Blob(synthdef_bytes)])
            .await?;

        // 2. Send /sync <id> immediately after
        let sync_id = self.alloc_sync_id();
        self.send_message("/sync", vec![OscType::Int(sync_id)])
            .await?;

        // 3. Await /synced <id> with 5s timeout
        self.await_synced(sync_id, Duration::from_secs(5)).await?;

        Ok(())
    }

    /// Recv loop waiting for /synced with the expected ID.
    /// Discards all other messages (e.g. /done, /status.reply) with debug logging.
    /// No concurrent recv — this is the only recv caller during sync-wait.
    async fn await_synced(&self, expected_id: i32, deadline: Duration) -> Result<()> {
        let mut buf = vec![0u8; 4096];
        timeout(deadline, async {
            loop {
                let n = self.socket.recv(&mut buf).await?;
                // Use decode_udp (not decode) for UDP datagrams
                if let Ok((_, OscPacket::Message(msg))) = decoder::decode_udp(&buf[..n]) {
                    if msg.addr == "/synced" {
                        if let Some(OscType::Int(id)) = msg.args.first() {
                            if *id == expected_id {
                                tracing::debug!("received /synced {}", expected_id);
                                return Ok(());
                            }
                        }
                    }
                    // Log and discard other messages
                    tracing::debug!(
                        "osc recv (awaiting /synced {}): {} {:?}",
                        expected_id,
                        msg.addr,
                        msg.args
                    );
                }
            }
        })
        .await
        .map_err(|_| OscBridgeError::SyncTimeout(expected_id))?
    }

    /// Create a new synth node. Returns the allocated node ID.
    /// If a node already exists for this thing_name, it is freed first (no orphans).
    pub async fn new_synth(&mut self, thing_name: &str, synthdef_name: &str) -> Result<i32> {
        // Free existing node for this thing if present
        if let Some(&old_id) = self.nodes.get(thing_name) {
            self.free_node_by_id(old_id).await?;
        }

        let node_id = self.alloc_node_id();

        // /s_new <defName:String> <nodeID:Int> <addAction:Int=0 head> <target:Int=1 default group>
        self.send_message(
            "/s_new",
            vec![
                OscType::String(synthdef_name.to_string()),
                OscType::Int(node_id),
                OscType::Int(0), // addAction: head
                OscType::Int(1), // target: default group
            ],
        )
        .await?;

        self.nodes.insert(thing_name.to_string(), node_id);
        tracing::info!("s_new: {} -> node {}", thing_name, node_id);
        Ok(node_id)
    }

    /// Set a named control parameter on a running synth.
    pub async fn set_param(&self, thing_name: &str, param: &str, value: f32) -> Result<()> {
        let &node_id = self
            .nodes
            .get(thing_name)
            .ok_or_else(|| OscBridgeError::UnknownThing(thing_name.to_string()))?;

        // /n_set <nodeID:Int> <paramName:String> <value:Float>
        self.send_message(
            "/n_set",
            vec![
                OscType::Int(node_id),
                OscType::String(param.to_string()),
                OscType::Float(value),
            ],
        )
        .await?;

        tracing::debug!("n_set: {} {param}={value}", thing_name);
        Ok(())
    }

    /// Free a synth node by thing name. Removes from registry.
    pub async fn free_node(&mut self, thing_name: &str) -> Result<()> {
        if let Some(node_id) = self.nodes.remove(thing_name) {
            self.free_node_by_id(node_id).await?;
            tracing::info!("n_free: {} (node {})", thing_name, node_id);
        }
        Ok(())
    }

    /// Free ALL tracked nodes. Call on daemon shutdown to prevent orphans.
    pub async fn free_all_nodes(&mut self) -> Result<()> {
        let entries: Vec<(String, i32)> = self
            .nodes
            .drain()
            .collect();
        for (name, id) in &entries {
            self.free_node_by_id(*id).await?;
            tracing::info!("n_free: {} (node {})", name, id);
        }
        Ok(())
    }

    /// Query amplitude of a running synth node.
    /// Sends /n_get for "amp" param; scsynth replies with /n_set.
    /// Returns None if no reply within 20ms (non-blocking best-effort).
    /// NOTE: placeholder — returns 0.0 for all known nodes until /s_get wiring.
    pub fn get_node_amplitude(&self, _node_id: i32) -> Option<f32> {
        // Placeholder: real implementation will send /n_get and parse /n_set reply.
        // For now return 0.0 so the protocol field is populated without blocking.
        Some(0.0)
    }

    /// Create a scsynth Group node. Sends /g_new [id, 0, 1] (head of default group).
    /// Returns the allocated group node ID.
    pub async fn create_group(&mut self) -> Result<i32> {
        let id = self.alloc_node_id();
        self.send_message(
            "/g_new",
            vec![
                OscType::Int(id),
                OscType::Int(0), // add_action: add to head
                OscType::Int(1), // target: default group
            ],
        )
        .await?;
        tracing::info!("g_new: group node {}", id);
        Ok(id)
    }

    /// Spawn a synth node inside a specific group (add to head of group).
    /// Used for routing source things into a stage group.
    pub async fn start_synth_in_group(
        &mut self,
        thing_name: &str,
        synthdef_name: &str,
        group_id: i32,
    ) -> Result<i32> {
        // Free existing node for this thing if present
        if let Some(&old_id) = self.nodes.get(thing_name) {
            self.free_node_by_id(old_id).await?;
        }

        let node_id = self.alloc_node_id();
        self.send_message(
            "/s_new",
            vec![
                OscType::String(synthdef_name.to_string()),
                OscType::Int(node_id),
                OscType::Int(0), // addAction: add to head of group
                OscType::Int(group_id),
            ],
        )
        .await?;

        self.nodes.insert(thing_name.to_string(), node_id);
        tracing::info!(
            "s_new (in group {}): {} -> node {}",
            group_id,
            thing_name,
            node_id
        );
        Ok(node_id)
    }

    /// Spawn an effect synth at the tail of a group (processes group output).
    /// Returns the allocated node ID for the effect node.
    pub async fn start_effect_at_tail(
        &mut self,
        effect_name: &str,
        synthdef_name: &str,
        group_id: i32,
    ) -> Result<i32> {
        let node_id = self.alloc_node_id();
        self.send_message(
            "/s_new",
            vec![
                OscType::String(synthdef_name.to_string()),
                OscType::Int(node_id),
                OscType::Int(1), // addAction: add to tail of group
                OscType::Int(group_id),
            ],
        )
        .await?;

        tracing::info!(
            "s_new (effect at tail of group {}): {} -> node {}",
            group_id,
            effect_name,
            node_id
        );
        Ok(node_id)
    }

    // -- Buffer management (sample playback) --

    /// Load an audio file into a scsynth buffer via /b_allocRead.
    /// Returns Ok(()) after /synced confirmation.
    /// The buffer is auto-sized to match the file's frame count and channels.
    pub async fn load_buffer(&mut self, buf_id: i32, path: &str) -> Result<()> {
        // /b_allocRead bufNum path startFrame numFrames
        // startFrame=0, numFrames=0 means "read entire file"
        self.send_message(
            "/b_allocRead",
            vec![
                OscType::Int(buf_id),
                OscType::String(path.to_string()),
                OscType::Int(0), // startFrame
                OscType::Int(0), // numFrames (0 = all)
            ],
        )
        .await?;

        let sync_id = self.alloc_sync_id();
        self.send_message("/sync", vec![OscType::Int(sync_id)])
            .await?;
        self.await_synced(sync_id, std::time::Duration::from_secs(5))
            .await?;

        tracing::info!("b_allocRead: buf {} <- {}", buf_id, path);
        Ok(())
    }

    /// Free a scsynth buffer via /b_free.
    pub async fn free_buffer(&mut self, buf_id: i32) -> Result<()> {
        self.send_message("/b_free", vec![OscType::Int(buf_id)])
            .await?;
        tracing::info!("b_free: buf {}", buf_id);
        Ok(())
    }

    /// Create a new synth node with extra args (key-value pairs).
    /// Used to pass buffer ID as a control parameter at /s_new time.
    pub async fn new_synth_with_args(
        &mut self,
        thing_name: &str,
        synthdef_name: &str,
        args: &[(&str, f32)],
    ) -> Result<i32> {
        // Free existing node for this thing if present
        if let Some(&old_id) = self.nodes.get(thing_name) {
            self.free_node_by_id(old_id).await?;
        }

        let node_id = self.alloc_node_id();

        let mut osc_args = vec![
            OscType::String(synthdef_name.to_string()),
            OscType::Int(node_id),
            OscType::Int(0), // addAction: head
            OscType::Int(1), // target: default group
        ];
        for (key, value) in args {
            osc_args.push(OscType::String(key.to_string()));
            osc_args.push(OscType::Float(*value));
        }

        self.send_message("/s_new", osc_args).await?;

        self.nodes.insert(thing_name.to_string(), node_id);
        tracing::info!("s_new (with args): {} -> node {}", thing_name, node_id);
        Ok(node_id)
    }

    /// Encode an OscMessage and send it over the UDP socket.
    async fn send_message(&self, addr: &str, args: Vec<OscType>) -> Result<()> {
        let packet = OscPacket::Message(OscMessage {
            addr: addr.to_string(),
            args,
        });
        let bytes = encoder::encode(&packet)
            .map_err(|e| OscBridgeError::EncodeError(format!("{:?}", e)))?;
        self.socket.send(&bytes).await?;
        tracing::debug!("osc send: {} ({} bytes)", addr, bytes.len());
        Ok(())
    }

    // -- Internal helpers --

    fn alloc_node_id(&mut self) -> i32 {
        let id = self.next_node_id;
        self.next_node_id += 1;
        id
    }

    fn alloc_sync_id(&mut self) -> i32 {
        let id = self.next_sync_id;
        self.next_sync_id += 1;
        id
    }

    async fn free_node_by_id(&self, node_id: i32) -> Result<()> {
        self.send_message("/n_free", vec![OscType::Int(node_id)])
            .await
    }
}
