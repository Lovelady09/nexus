//! Network module constants

use std::net::{Ipv4Addr, Ipv6Addr};
use std::time::Duration;

use ipnet::{Ipv4Net, Ipv6Net};

/// Connection timeout duration (30 seconds)
pub const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);

/// Buffer size for the Iced stream channel
pub const STREAM_CHANNEL_SIZE: usize = 100;

/// Default features to request during login
pub const DEFAULT_FEATURES: &[&str] = &["chat", "files", "news"];

/// Yggdrasil mesh network IPv6 range (0200::/7)
pub const YGGDRASIL_NETWORK: Ipv6Net =
    Ipv6Net::new_assert(Ipv6Addr::new(0x200, 0, 0, 0, 0, 0, 0, 0), 7);

/// RFC 1918 private IPv4 networks
pub const PRIVATE_10: Ipv4Net = Ipv4Net::new_assert(Ipv4Addr::new(10, 0, 0, 0), 8);
pub const PRIVATE_172: Ipv4Net = Ipv4Net::new_assert(Ipv4Addr::new(172, 16, 0, 0), 12);
pub const PRIVATE_192: Ipv4Net = Ipv4Net::new_assert(Ipv4Addr::new(192, 168, 0, 0), 16);

/// IPv6 Unique Local Addresses (fc00::/7)
pub const IPV6_ULA: Ipv6Net = Ipv6Net::new_assert(Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 0), 7);

/// Ping interval for NAT keepalive (5 minutes)
///
/// Most consumer NAT routers drop idle TCP connections after 30-60 minutes.
/// Sending a ping every 5 minutes keeps the NAT mapping alive.
pub const PING_INTERVAL: u64 = 300;
