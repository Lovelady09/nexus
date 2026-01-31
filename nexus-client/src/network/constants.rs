//! Network module constants

use std::net::Ipv6Addr;
use std::time::Duration;

use ipnet::Ipv6Net;

/// Connection timeout duration (30 seconds)
pub const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);

/// Buffer size for the Iced stream channel
pub const STREAM_CHANNEL_SIZE: usize = 100;

/// Default features to request during login
pub const DEFAULT_FEATURES: &[&str] = &["chat", "files", "news"];

/// Yggdrasil mesh network IPv6 range (0200::/7)
pub const YGGDRASIL_NETWORK: Ipv6Net =
    Ipv6Net::new_assert(Ipv6Addr::new(0x200, 0, 0, 0, 0, 0, 0, 0), 7);

/// Ping interval for NAT keepalive (5 minutes)
///
/// Most consumer NAT routers drop idle TCP connections after 30-60 minutes.
/// Sending a ping every 5 minutes keeps the NAT mapping alive.
pub const PING_INTERVAL: u64 = 300;
