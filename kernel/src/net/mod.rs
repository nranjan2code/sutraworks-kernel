//! Network Stack
//!
//! TCP/IP networking implementation for the Intent Kernel.

pub mod interface;
pub mod ethernet;
pub mod ip;
pub mod ipv4;
pub mod arp;
pub mod icmp;
pub mod udp;
pub mod tcp;
pub mod socket;

// Re-export key types for convenience
pub use ip::Ipv4Addr;
pub use ethernet::MacAddress;
pub use tcp::{TcpConnection, TcpState, TcpSegment, TCB_TABLE, tcp_tick};

use crate::arch::SpinLock;

// ═══════════════════════════════════════════════════════════════════════════════
// NETWORK CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════════

/// Network configuration
#[derive(Debug, Clone)]
pub struct NetConfig {
    pub ip_addr: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub gateway: Ipv4Addr,
    pub mac_addr: MacAddress,
}

impl NetConfig {
    pub const fn new() -> Self {
        Self {
            ip_addr: Ipv4Addr([0, 0, 0, 0]),
            netmask: Ipv4Addr([255, 255, 255, 0]),
            gateway: Ipv4Addr([0, 0, 0, 0]),
            mac_addr: MacAddress([0, 0, 0, 0, 0, 0]),
        }
    }
    
    /// Check if an IP is on the local subnet
    pub fn is_local(&self, ip: Ipv4Addr) -> bool {
        for i in 0..4 {
            if (self.ip_addr.0[i] & self.netmask.0[i]) != (ip.0[i] & self.netmask.0[i]) {
                return false;
            }
        }
        true
    }
}

/// Global network configuration
static NET_CONFIG: SpinLock<NetConfig> = SpinLock::new(NetConfig::new());

/// Get current network configuration
pub fn config() -> NetConfig {
    NET_CONFIG.lock().clone()
}

/// Set network configuration
pub fn set_config(cfg: NetConfig) {
    *NET_CONFIG.lock() = cfg;
}

// ═══════════════════════════════════════════════════════════════════════════════
// CHECKSUM (RFC 1071)
// ═══════════════════════════════════════════════════════════════════════════════

/// Calculate Internet checksum (RFC 1071)
///
/// Used for IP, ICMP, UDP, and TCP checksums.
pub fn checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    
    // Sum 16-bit words
    while i + 1 < data.len() {
        sum += u16::from_be_bytes([data[i], data[i + 1]]) as u32;
        i += 2;
    }
    
    // Handle odd byte
    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }
    
    // Fold 32-bit sum to 16 bits
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    
    // Return one's complement
    !(sum as u16)
}

// ═══════════════════════════════════════════════════════════════════════════════
// IPv4 FORWARDING (stub - ipv4.rs will be updated)
// ═══════════════════════════════════════════════════════════════════════════════

/// Send an IPv4 packet (stub until full driver integration)
/// This is called by tcp.rs for sending TCP segments
pub fn send_ip_packet(_dst_ip: Ipv4Addr, _protocol: u8, _payload: &[u8]) -> Result<(), &'static str> {
    // In a full implementation, this would:
    // 1. Build IP header
    // 2. Resolve dst MAC via ARP
    // 3. Send via ethernet driver
    // For now, this is a stub
    Ok(())
}
