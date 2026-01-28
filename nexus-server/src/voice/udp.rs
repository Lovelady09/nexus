//! UDP/DTLS voice server for real-time audio communication
//!
//! This module handles the UDP side of voice chat, receiving voice packets
//! from clients over DTLS and relaying them to other participants.
//!
//! ## Architecture
//!
//! - DTLS listener on port 7500 (same as TCP, OS routes by protocol)
//! - Uses the same certificate as TCP/TLS
//! - Voice packets authenticated by token from VoiceJoinResponse
//! - Server decrypts, validates token, adds sender info, re-encrypts, relays
//!
//! ## Packet Flow
//!
//! 1. Client joins voice via TCP (VoiceJoin) and receives token
//! 2. Client establishes DTLS connection to server UDP port
//! 3. Client sends VoicePacket with token for authentication
//! 4. Server validates token, looks up session in VoiceRegistry
//! 5. Server relays as RelayedVoicePacket to other participants

use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, RwLock as StdRwLock};
use std::time::{Duration, Instant};

use dtls::config::Config as DtlsConfig;
use dtls::crypto::Certificate;
use dtls::listener::listen;
use tokio::sync::RwLock;
use uuid::Uuid;
use webrtc_util::conn::{Conn, Listener};

use nexus_common::voice::{
    MAX_VOICE_PACKET_SIZE, RelayedVoicePacket, VOICE_SESSION_TIMEOUT_SECS, VoiceMessageType,
    VoicePacket,
};

use crate::db::Permission;
use crate::ip_rule_cache::IpRuleCache;
use crate::users::UserManager;

use super::VoiceRegistry;

/// DTLS connection state for a voice client
#[allow(dead_code)] // Fields used in handle_connection and cleanup_loop
struct DtlsClient {
    /// The DTLS connection
    conn: Arc<dyn Conn + Send + Sync>,
    /// Voice token for this client (set after first valid packet)
    token: Option<Uuid>,
    /// Client's remote address
    addr: SocketAddr,
    /// Last packet received time (for timeout)
    last_packet: Instant,
    /// Nickname of the user (cached for relay, set after token validation)
    nickname: Option<String>,
}

/// Manages UDP/DTLS voice connections
pub struct VoiceUdpServer {
    /// DTLS listener for voice connections
    listener: Arc<dyn Listener + Send + Sync>,
    /// Voice registry for session lookups
    registry: VoiceRegistry,
    /// Active DTLS clients, keyed by remote address
    clients: Arc<RwLock<HashMap<SocketAddr, Arc<RwLock<DtlsClient>>>>>,
    /// IP rule cache for ban checking
    ip_rule_cache: Arc<StdRwLock<IpRuleCache>>,
    /// User manager for permission checks
    user_manager: UserManager,
    /// Debug mode flag
    debug: bool,
}

#[allow(dead_code)] // Will be wired up in main.rs
impl VoiceUdpServer {
    /// Create a new voice UDP server with a pre-created DTLS listener
    ///
    /// # Arguments
    ///
    /// * `listener` - Pre-created DTLS listener
    /// * `registry` - Voice registry for session lookups
    /// * `ip_rule_cache` - IP rule cache for ban checking
    /// * `user_manager` - User manager for permission checks
    /// * `debug` - Enable debug logging
    pub fn new(
        listener: Arc<dyn Listener + Send + Sync>,
        registry: VoiceRegistry,
        ip_rule_cache: Arc<StdRwLock<IpRuleCache>>,
        user_manager: UserManager,
        debug: bool,
    ) -> Self {
        Self {
            listener,
            registry,
            clients: Arc::new(RwLock::new(HashMap::new())),
            ip_rule_cache,
            user_manager,
            debug,
        }
    }

    /// Run the voice UDP server
    ///
    /// This method runs forever, accepting DTLS connections and processing voice packets.
    /// It should be spawned as a separate tokio task.
    pub async fn run(self: Arc<Self>) {
        // Spawn cleanup task
        let cleanup_self = self.clone();
        tokio::spawn(async move {
            cleanup_self.cleanup_loop().await;
        });

        // Accept loop
        loop {
            match self.listener.accept().await {
                Ok((conn, remote_addr)) => {
                    // Check IP ban before processing (trust bypasses ban)
                    let should_allow = {
                        let cache = self
                            .ip_rule_cache
                            .read()
                            .expect("ip rule cache lock poisoned");
                        if cache.needs_rebuild() {
                            drop(cache);
                            self.ip_rule_cache
                                .write()
                                .expect("ip rule cache lock poisoned")
                                .should_allow(remote_addr.ip())
                        } else {
                            cache.should_allow_read_only(remote_addr.ip())
                        }
                    };

                    if !should_allow {
                        if self.debug {
                            eprintln!("Voice DTLS: Rejected banned IP {}", remote_addr.ip());
                        }
                        let _ = conn.close().await;
                        continue;
                    }

                    // Reject IPs that don't have an active voice session
                    if !self.registry.has_session_for_ip(remote_addr.ip()).await {
                        if self.debug {
                            eprintln!(
                                "Voice DTLS: Rejected IP {} (no voice session)",
                                remote_addr.ip()
                            );
                        }
                        let _ = conn.close().await;
                        continue;
                    }

                    if self.debug {
                        eprintln!("Voice DTLS: New connection from {}", remote_addr);
                    }

                    // Create client state
                    let client = Arc::new(RwLock::new(DtlsClient {
                        conn: conn.clone(),
                        token: None,
                        addr: remote_addr,
                        last_packet: Instant::now(),
                        nickname: None,
                    }));

                    // Store client
                    {
                        let mut clients = self.clients.write().await;
                        clients.insert(remote_addr, client.clone());
                    }

                    // Spawn handler for this connection
                    let server = self.clone();
                    tokio::spawn(async move {
                        server.handle_connection(client, remote_addr).await;
                    });
                }
                Err(e) => {
                    if self.debug {
                        eprintln!("Voice DTLS accept error: {}", e);
                    }
                }
            }
        }
    }

    /// Handle a single DTLS connection
    async fn handle_connection(&self, client: Arc<RwLock<DtlsClient>>, remote_addr: SocketAddr) {
        let mut buf = vec![0u8; MAX_VOICE_PACKET_SIZE + 100]; // Extra for DTLS overhead

        loop {
            // Get connection reference
            let conn = {
                let client_guard = client.read().await;
                client_guard.conn.clone()
            };

            // Read with timeout
            let read_result = tokio::time::timeout(
                Duration::from_secs(VOICE_SESSION_TIMEOUT_SECS),
                conn.recv(&mut buf),
            )
            .await;

            match read_result {
                Ok(Ok(len)) if len > 0 => {
                    let packet_data = buf[..len].to_vec();
                    self.handle_packet(&client, &packet_data).await;
                }
                Ok(Ok(_)) => {
                    // Zero-length read, connection closed
                    if self.debug {
                        eprintln!("Voice DTLS: Connection closed from {}", remote_addr);
                    }
                    break;
                }
                Ok(Err(e)) => {
                    if self.debug {
                        eprintln!("Voice DTLS read error from {}: {}", remote_addr, e);
                    }
                    break;
                }
                Err(_) => {
                    // Timeout
                    if self.debug {
                        eprintln!("Voice DTLS: Connection timeout from {}", remote_addr);
                    }
                    break;
                }
            }
        }

        // Remove client on disconnect
        {
            let mut clients = self.clients.write().await;
            clients.remove(&remote_addr);
        }

        // Close the connection
        let conn = {
            let client_guard = client.read().await;
            client_guard.conn.clone()
        };
        let _ = conn.close().await;
    }

    /// Handle an incoming voice packet
    async fn handle_packet(&self, client: &Arc<RwLock<DtlsClient>>, data: &[u8]) {
        // Parse the voice packet
        let Some(packet) = VoicePacket::from_bytes(data) else {
            if self.debug {
                let addr = client.read().await.addr;
                eprintln!("Voice DTLS: Invalid packet from {}", addr);
            }
            return;
        };

        // Update last packet time
        {
            let mut client_guard = client.write().await;
            client_guard.last_packet = Instant::now();
        }

        // Get or validate token
        let (token, nickname) = {
            let client_guard = client.read().await;
            (client_guard.token, client_guard.nickname.clone())
        };

        let (validated_token, sender_nickname) = if let (Some(t), Some(n)) = (token, nickname) {
            // Already validated
            (t, n)
        } else {
            // First packet or need to validate
            let Some(session) = self.registry.get_by_token(packet.token).await else {
                if self.debug {
                    let addr = client.read().await.addr;
                    eprintln!("Voice DTLS: Unknown token from {}", addr);
                }
                return;
            };

            // Update client state with validated info
            {
                let mut client_guard = client.write().await;
                client_guard.token = Some(packet.token);
                client_guard.nickname = Some(session.nickname.clone());

                // Update UDP address in registry
                if session.udp_addr.is_none() {
                    self.registry
                        .set_udp_addr(packet.token, client_guard.addr)
                        .await;
                }
            }

            (packet.token, session.nickname)
        };

        // Verify token matches (in case client tries to switch tokens)
        if packet.token != validated_token {
            if self.debug {
                let addr = client.read().await.addr;
                eprintln!("Voice DTLS: Token mismatch from {}", addr);
            }
            return;
        }

        // Handle based on message type
        match packet.msg_type {
            VoiceMessageType::Keepalive => {
                // Keepalive just updates last_packet time, already done above
                if self.debug {
                    let addr = client.read().await.addr;
                    eprintln!("Voice DTLS: Keepalive from {} ({})", sender_nickname, addr);
                }
            }
            VoiceMessageType::VoiceData
            | VoiceMessageType::SpeakingStarted
            | VoiceMessageType::SpeakingStopped => {
                // Get target key from session
                if let Some(session) = self.registry.get_by_token(packet.token).await {
                    // Check voice_talk permission before relaying (optimized to avoid cloning UserSession)
                    match self
                        .user_manager
                        .has_permission(session.session_id, Permission::VoiceTalk)
                        .await
                    {
                        Some(true) => {
                            // User has permission, relay the packet
                            self.relay_packet(&packet, &sender_nickname, &session.target_key())
                                .await;
                        }
                        Some(false) => {
                            // User lacks permission, drop packet silently
                            if self.debug {
                                eprintln!(
                                    "Voice DTLS: {} lacks voice_talk permission, dropping packet",
                                    sender_nickname
                                );
                            }
                        }
                        None => {
                            // User disconnected, drop packet
                        }
                    }
                }
            }
        }
    }

    /// Relay a voice packet to other participants in the same voice session
    async fn relay_packet(&self, packet: &VoicePacket, sender_nickname: &str, target_key: &str) {
        // Get all sessions for this target
        let sessions = self.registry.get_sessions_for_target(target_key).await;

        // Create relayed packet
        let relayed = RelayedVoicePacket::from_voice_packet(packet, sender_nickname.to_string());
        let relayed_bytes = relayed.to_bytes();

        // Get client map for connection lookup
        let clients = self.clients.read().await;

        for session in sessions {
            // Don't send back to sender
            if session.nickname == sender_nickname {
                continue;
            }

            // Find the DTLS connection for this session
            if let Some(udp_addr) = session.udp_addr
                && let Some(client) = clients.get(&udp_addr)
            {
                let conn = {
                    let client_guard = client.read().await;
                    client_guard.conn.clone()
                };

                if let Err(e) = conn.send(&relayed_bytes).await
                    && self.debug
                {
                    eprintln!(
                        "Voice DTLS: Failed to relay to {} ({}): {}",
                        session.nickname, udp_addr, e
                    );
                }
            }
        }
    }

    /// Cleanup loop for removing stale client entries
    async fn cleanup_loop(&self) {
        let check_interval = Duration::from_secs(30);
        let timeout = Duration::from_secs(VOICE_SESSION_TIMEOUT_SECS);

        loop {
            tokio::time::sleep(check_interval).await;

            let now = Instant::now();
            let mut clients = self.clients.write().await;

            // Collect addresses of timed-out clients
            let mut timed_out = Vec::new();
            for (addr, client) in clients.iter() {
                let client_guard = client.read().await;
                if now.duration_since(client_guard.last_packet) > timeout {
                    timed_out.push(*addr);
                }
            }

            // Remove timed-out clients
            for addr in timed_out {
                if let Some(client) = clients.remove(&addr) {
                    let client_guard = client.read().await;
                    if self.debug {
                        eprintln!(
                            "Voice DTLS: Cleanup timed out client: {:?} ({})",
                            client_guard.nickname, addr
                        );
                    }
                    // Close connection
                    let _ = client_guard.conn.close().await;
                }
            }
        }
    }

    /// Get the number of active DTLS clients
    #[allow(dead_code)]
    pub async fn client_count(&self) -> usize {
        let clients = self.clients.read().await;
        clients.len()
    }
}

/// Create a DTLS listener for voice traffic
///
/// # Arguments
///
/// * `addr` - Socket address to bind to (typically same IP as TCP, port 7500)
/// * `cert_path` - Path to the certificate PEM file
/// * `key_path` - Path to the private key PEM file
///
/// # Returns
///
/// The DTLS listener, or an error message
pub async fn create_voice_listener(
    addr: SocketAddr,
    cert_path: &Path,
    key_path: &Path,
) -> Result<Arc<dyn Listener + Send + Sync>, String> {
    let config = load_dtls_config(cert_path, key_path)?;

    let listener = listen(addr, config)
        .await
        .map_err(|e| format!("Failed to create voice DTLS listener on {}: {}", addr, e))?;

    Ok(Arc::new(listener))
}

/// Load DTLS configuration from certificate and key files
///
/// Uses the same certificate as the TCP/TLS server.
fn load_dtls_config(cert_path: &Path, key_path: &Path) -> Result<DtlsConfig, String> {
    // Read certificate and key PEM files
    let cert_pem = fs::read_to_string(cert_path)
        .map_err(|e| format!("Failed to read certificate file: {}", e))?;

    let key_pem = fs::read_to_string(key_path)
        .map_err(|e| format!("Failed to read private key file: {}", e))?;

    // Combine into single PEM string (Certificate::from_pem expects both in one string)
    let combined_pem = format!("{}\n{}", cert_pem, key_pem);

    // Parse certificate with private key
    let certificate = Certificate::from_pem(&combined_pem)
        .map_err(|e| format!("Failed to parse certificate: {}", e))?;

    // Create DTLS config
    let config = DtlsConfig {
        certificates: vec![certificate],
        insecure_skip_verify: true, // Clients use TOFU model like TCP
        ..Default::default()
    };

    Ok(config)
}

#[cfg(test)]
mod tests {
    // Note: Full integration tests require actual DTLS listener setup
    // which needs certificate files. Unit tests for packet handling
    // are in nexus-common/src/voice.rs
}
