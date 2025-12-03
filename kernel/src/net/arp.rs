//! ARP (Address Resolution Protocol)
//!
//! Maps IPv4 addresses to MAC addresses on the local network.

use super::{Ipv4Addr, config};
use crate::drivers::ethernet::{MacAddr, send_frame};
use crate::arch::SpinLock;
use alloc::vec::Vec;

/// ARP Cache Entry
#[derive(Clone, Copy)]
struct ArpEntry {
    ip: Ipv4Addr,
    mac: MacAddr,
    ttl: u64,  // Time to live in seconds
}

/// ARP Cache (simple linear search for embedded systems)
struct ArpCache {
    entries: Vec<ArpEntry>,
}

impl ArpCache {
    const fn new() -> Self {
        Self { entries: Vec::new() }
    }

    fn lookup(&self, ip: Ipv4Addr) -> Option<MacAddr> {
        for entry in &self.entries {
            if entry.ip == ip {
                return Some(entry.mac);
            }
        }
        None
    }

    fn insert(&mut self, ip: Ipv4Addr, mac: MacAddr) {
        // Remove existing entry for this IP
        self.entries.retain(|e| e.ip != ip);

        // Add new entry
        self.entries.push(ArpEntry {
            ip,
            mac,
            ttl: 300,  // 5 minutes
        });

        // Limit cache size
        if self.entries.len() > 16 {
            self.entries.remove(0);
        }
    }
}

static ARP_CACHE: SpinLock<ArpCache> = SpinLock::new(ArpCache::new());

/// Initialize ARP module
pub fn init() {
    // Nothing to do yet
}

/// Resolve an IP address to a MAC address
///
/// Returns cached MAC if available, otherwise sends ARP request and returns None.
/// Caller should retry after a delay.
pub fn resolve(ip: Ipv4Addr) -> Option<MacAddr> {
    // Check cache first
    if let Some(mac) = ARP_CACHE.lock().lookup(ip) {
        return Some(mac);
    }

    // Send ARP request
    let _ = send_arp_request(ip);

    None
}

/// Handle incoming ARP packet
pub fn handle_packet(data: &[u8]) -> Result<(), &'static str> {
    if data.len() < 28 {
        return Err("ARP packet too short");
    }

    // Parse ARP packet
    let hw_type = u16::from_be_bytes([data[0], data[1]]);
    let proto_type = u16::from_be_bytes([data[2], data[3]]);
    let hw_len = data[4];
    let proto_len = data[5];
    let operation = u16::from_be_bytes([data[6], data[7]]);

    // Only support Ethernet (hw_type=1) and IPv4 (proto_type=0x0800)
    if hw_type != 1 || proto_type != 0x0800 || hw_len != 6 || proto_len != 4 {
        return Err("Unsupported ARP parameters");
    }

    let sender_mac = MacAddr([data[8], data[9], data[10], data[11], data[12], data[13]]);
    let sender_ip = Ipv4Addr([data[14], data[15], data[16], data[17]]);
    let target_mac = MacAddr([data[18], data[19], data[20], data[21], data[22], data[23]]);
    let target_ip = Ipv4Addr([data[24], data[25], data[26], data[27]]);

    // Update ARP cache with sender info
    ARP_CACHE.lock().insert(sender_ip, sender_mac);

    // Handle based on operation
    match operation {
        1 => {
            // ARP Request
            let our_ip = config().ip_addr;
            if target_ip == our_ip {
                // Send ARP reply
                send_arp_reply(sender_ip, sender_mac)?;
            }
        }
        2 => {
            // ARP Reply - already cached above
        }
        _ => {}
    }

    Ok(())
}

/// Send ARP request
fn send_arp_request(target_ip: Ipv4Addr) -> Result<(), &'static str> {
    let cfg = config();

    let mut packet = [0u8; 42];  // Ethernet header (14) + ARP (28)

    // Ethernet header
    packet[0..6].copy_from_slice(&MacAddr::BROADCAST.0);  // Dest MAC
    packet[6..12].copy_from_slice(&cfg.mac_addr.0);       // Src MAC
    packet[12..14].copy_from_slice(&0x0806u16.to_be_bytes());  // EtherType: ARP

    // ARP packet
    packet[14..16].copy_from_slice(&1u16.to_be_bytes());  // HW Type: Ethernet
    packet[16..18].copy_from_slice(&0x0800u16.to_be_bytes());  // Proto Type: IPv4
    packet[18] = 6;  // HW Len
    packet[19] = 4;  // Proto Len
    packet[20..22].copy_from_slice(&1u16.to_be_bytes());  // Operation: Request

    packet[22..28].copy_from_slice(&cfg.mac_addr.0);  // Sender MAC
    packet[28..32].copy_from_slice(&cfg.ip_addr.0);   // Sender IP
    packet[32..38].copy_from_slice(&[0; 6]);          // Target MAC (unknown)
    packet[38..42].copy_from_slice(&target_ip.0);     // Target IP

    send_frame(&packet)
}

/// Send ARP reply
fn send_arp_reply(target_ip: Ipv4Addr, target_mac: MacAddr) -> Result<(), &'static str> {
    let cfg = config();

    let mut packet = [0u8; 42];

    // Ethernet header
    packet[0..6].copy_from_slice(&target_mac.0);    // Dest MAC
    packet[6..12].copy_from_slice(&cfg.mac_addr.0); // Src MAC
    packet[12..14].copy_from_slice(&0x0806u16.to_be_bytes());

    // ARP packet
    packet[14..16].copy_from_slice(&1u16.to_be_bytes());
    packet[16..18].copy_from_slice(&0x0800u16.to_be_bytes());
    packet[18] = 6;
    packet[19] = 4;
    packet[20..22].copy_from_slice(&2u16.to_be_bytes());  // Operation: Reply

    packet[22..28].copy_from_slice(&cfg.mac_addr.0);  // Sender MAC (us)
    packet[28..32].copy_from_slice(&cfg.ip_addr.0);   // Sender IP (us)
    packet[32..38].copy_from_slice(&target_mac.0);    // Target MAC
    packet[38..42].copy_from_slice(&target_ip.0);     // Target IP

    send_frame(&packet)
}
