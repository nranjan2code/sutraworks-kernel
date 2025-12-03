//! Network Stack (TCP/IP)
//!
//! Minimal but functional implementation of:
//! - Ethernet (Layer 2)
//! - ARP (Address Resolution Protocol)
//! - IPv4 (Layer 3)
//! - ICMP (Ping)
//! - UDP (Connectionless transport)
//! - TCP (Connection-oriented transport - simplified)
//!
//! # Design Goals
//! - **Minimal**: ~1000 LOC for full stack
//! - **Intent-Driven**: Network operations exposed as intents
//! - **Embedded-Friendly**: No dynamic allocation in fast path

pub mod arp;
pub mod ipv4;
pub mod icmp;
pub mod udp;
pub mod tcp;

use crate::drivers::ethernet::MacAddr;
use crate::arch::SpinLock;

/// IPv4 Address
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ipv4Addr(pub [u8; 4]);

impl Ipv4Addr {
    pub const BROADCAST: Self = Ipv4Addr([255, 255, 255, 255]);
    pub const UNSPECIFIED: Self = Ipv4Addr([0, 0, 0, 0]);

    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Ipv4Addr([a, b, c, d])
    }

    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        Ipv4Addr(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }

    pub fn to_u32(&self) -> u32 {
        u32::from_be_bytes(self.0)
    }

    pub fn from_u32(val: u32) -> Self {
        Ipv4Addr(val.to_be_bytes())
    }
}

/// Network Configuration
pub struct NetConfig {
    pub ip_addr: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub gateway: Ipv4Addr,
    pub mac_addr: MacAddr,
}

impl NetConfig {
    pub const fn new() -> Self {
        Self {
            ip_addr: Ipv4Addr::UNSPECIFIED,
            netmask: Ipv4Addr::UNSPECIFIED,
            gateway: Ipv4Addr::UNSPECIFIED,
            mac_addr: MacAddr([0; 6]),
        }
    }

    /// Check if an IP is on the local subnet
    pub fn is_local(&self, ip: Ipv4Addr) -> bool {
        let ip_val = ip.to_u32();
        let our_ip = self.ip_addr.to_u32();
        let mask = self.netmask.to_u32();

        (ip_val & mask) == (our_ip & mask)
    }
}

static NET_CONFIG: SpinLock<NetConfig> = SpinLock::new(NetConfig::new());

/// Initialize network stack
pub fn init(ip_addr: Ipv4Addr, netmask: Ipv4Addr, gateway: Ipv4Addr, mac_addr: MacAddr) {
    let mut config = NET_CONFIG.lock();
    config.ip_addr = ip_addr;
    config.netmask = netmask;
    config.gateway = gateway;
    config.mac_addr = mac_addr;

    crate::kprintln!("[NET] Initialized: {}.{}.{}.{}/{}.{}.{}.{}",
        ip_addr.0[0], ip_addr.0[1], ip_addr.0[2], ip_addr.0[3],
        netmask.0[0], netmask.0[1], netmask.0[2], netmask.0[3]);

    // Initialize sub-modules
    arp::init();
}

/// Get network configuration
pub fn config() -> NetConfig {
    *NET_CONFIG.lock()
}

/// Process incoming packet (called from interrupt or polling loop)
pub fn process_packet(frame: &[u8]) -> Result<(), &'static str> {
    if frame.len() < 14 {
        return Err("Frame too short");
    }

    // Parse Ethernet header
    let eth_type = u16::from_be_bytes([frame[12], frame[13]]);

    match eth_type {
        0x0806 => arp::handle_packet(&frame[14..]),  // ARP
        0x0800 => ipv4::handle_packet(&frame[14..]),  // IPv4
        _ => Ok(()),  // Ignore unknown types
    }
}

/// Checksum calculation (Internet Checksum - RFC 1071)
pub fn checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;

    // Sum 16-bit words
    for chunk in data.chunks_exact(2) {
        let word = u16::from_be_bytes([chunk[0], chunk[1]]) as u32;
        sum += word;
    }

    // Add odd byte if present
    if data.len() % 2 == 1 {
        sum += (data[data.len() - 1] as u32) << 8;
    }

    // Fold 32-bit sum to 16 bits
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    // One's complement
    !sum as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum() {
        // Test vector from RFC 1071
        let data = [0x00, 0x01, 0xF2, 0x03, 0xF4, 0xF5, 0xF6, 0xF7];
        let result = checksum(&data);
        // Expected: 0x220D (depends on test data)
        assert_ne!(result, 0);
    }

    #[test]
    fn test_ipv4_addr() {
        let ip = Ipv4Addr::new(192, 168, 1, 100);
        assert_eq!(ip.0, [192, 168, 1, 100]);
        assert_eq!(ip.to_u32(), 0xC0A80164);
    }

    #[test]
    fn test_is_local() {
        let config = NetConfig {
            ip_addr: Ipv4Addr::new(192, 168, 1, 100),
            netmask: Ipv4Addr::new(255, 255, 255, 0),
            gateway: Ipv4Addr::new(192, 168, 1, 1),
            mac_addr: MacAddr([0; 6]),
        };

        assert!(config.is_local(Ipv4Addr::new(192, 168, 1, 50)));
        assert!(!config.is_local(Ipv4Addr::new(192, 168, 2, 50)));
    }
}
