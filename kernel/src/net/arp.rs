use alloc::vec::Vec;
use core::convert::TryInto;
use crate::net::ethernet::MacAddress;
use crate::net::ip::Ipv4Addr;
use crate::kernel::sync::SpinLock;

/// ARP Operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ArpOperation {
    Request = 1,
    Reply = 2,
    Unknown(u16),
}

impl From<u16> for ArpOperation {
    fn from(val: u16) -> Self {
        match val {
            1 => ArpOperation::Request,
            2 => ArpOperation::Reply,
            _ => ArpOperation::Unknown(val),
        }
    }
}

/// ARP Packet
#[derive(Debug, Clone)]
pub struct ArpPacket {
    pub hardware_type: u16, // 1 for Ethernet
    pub protocol_type: u16, // 0x0800 for IPv4
    pub hw_addr_len: u8,    // 6
    pub proto_addr_len: u8, // 4
    pub operation: ArpOperation,
    pub sender_hw_addr: MacAddress,
    pub sender_proto_addr: Ipv4Addr,
    pub target_hw_addr: MacAddress,
    pub target_proto_addr: Ipv4Addr,
}

impl ArpPacket {
    pub fn parse(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 28 {
            return Err("Packet too short for ARP");
        }
        
        let hardware_type = u16::from_be_bytes(data[0..2].try_into().unwrap());
        let protocol_type = u16::from_be_bytes(data[2..4].try_into().unwrap());
        let hw_addr_len = data[4];
        let proto_addr_len = data[5];
        let operation = ArpOperation::from(u16::from_be_bytes(data[6..8].try_into().unwrap()));
        
        let sender_hw_addr = MacAddress(data[8..14].try_into().unwrap());
        let sender_proto_addr = Ipv4Addr(data[14..18].try_into().unwrap());
        let target_hw_addr = MacAddress(data[18..24].try_into().unwrap());
        let target_proto_addr = Ipv4Addr(data[24..28].try_into().unwrap());
        
        Ok(Self {
            hardware_type,
            protocol_type,
            hw_addr_len,
            proto_addr_len,
            operation,
            sender_hw_addr,
            sender_proto_addr,
            target_hw_addr,
            target_proto_addr,
        })
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(28);
        
        bytes.extend_from_slice(&self.hardware_type.to_be_bytes());
        bytes.extend_from_slice(&self.protocol_type.to_be_bytes());
        bytes.push(self.hw_addr_len);
        bytes.push(self.proto_addr_len);
        
        let op_u16 = match self.operation {
            ArpOperation::Request => 1,
            ArpOperation::Reply => 2,
            ArpOperation::Unknown(val) => val,
        };
        bytes.extend_from_slice(&op_u16.to_be_bytes());
        
        bytes.extend_from_slice(&self.sender_hw_addr.0);
        bytes.extend_from_slice(&self.sender_proto_addr.0);
        bytes.extend_from_slice(&self.target_hw_addr.0);
        bytes.extend_from_slice(&self.target_proto_addr.0);
        
        bytes
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ARP CACHE
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum ARP cache entries
const MAX_ARP_ENTRIES: usize = 16;

/// ARP cache entry
#[derive(Debug, Clone, Copy)]
struct ArpEntry {
    ip: Ipv4Addr,
    mac: MacAddress,
    timestamp_ms: u64, // Time when entry was added (for expiration)
}

/// ARP cache
struct ArpCache {
    entries: [Option<ArpEntry>; MAX_ARP_ENTRIES],
    count: usize,
}

impl ArpCache {
    const fn new() -> Self {
        Self {
            entries: [None; MAX_ARP_ENTRIES],
            count: 0,
        }
    }
    
    fn lookup(&self, ip: Ipv4Addr) -> Option<MacAddress> {
        let now = crate::drivers::timer::uptime_ms();
        const ARP_TIMEOUT_MS: u64 = 20 * 60 * 1000; // 20 minutes per RFC 826
        
        for entry in self.entries.iter().flatten() {
            if entry.ip == ip {
                // Check if entry has expired
                if now.saturating_sub(entry.timestamp_ms) < ARP_TIMEOUT_MS {
                    return Some(entry.mac);
                }
                // Entry expired, will be replaced
            }
        }
        None
    }
    
    fn insert(&mut self, ip: Ipv4Addr, mac: MacAddress) {
        let now = crate::drivers::timer::uptime_ms();
        
        // Check if already exists (update timestamp)
        for entry in self.entries.iter_mut().flatten() {
            if entry.ip == ip {
                entry.mac = mac;
                entry.timestamp_ms = now;
                return;
            }
        }
        
        // Find empty slot
        for entry in &mut self.entries {
            if entry.is_none() {
                *entry = Some(ArpEntry { ip, mac, timestamp_ms: now });
                self.count += 1;
                return;
            }
        }
        
        // Cache full, replace first entry
        self.entries[0] = Some(ArpEntry { ip, mac, timestamp_ms: now });
    }
}

/// Global ARP cache
static ARP_CACHE: SpinLock<ArpCache> = SpinLock::new(ArpCache::new());

/// Resolve an IP address to a MAC address
///
/// Returns cached MAC if available, otherwise returns None.
/// A full implementation would send an ARP request and wait.
pub fn resolve(ip: Ipv4Addr) -> Option<MacAddress> {
    ARP_CACHE.lock().lookup(ip)
}

/// Add an entry to the ARP cache
pub fn cache_insert(ip: Ipv4Addr, mac: MacAddress) {
    ARP_CACHE.lock().insert(ip, mac);
}

/// Handle incoming ARP packet
pub fn handle_packet(data: &[u8]) -> Result<(), &'static str> {
    let packet = ArpPacket::parse(data)?;
    
    // Cache the sender's MAC (we learned something!)
    cache_insert(packet.sender_proto_addr, packet.sender_hw_addr);
    
    match packet.operation {
        ArpOperation::Request => {
            // Check if this request is for our IP
            let cfg = crate::net::config();
            if packet.target_proto_addr == cfg.ip_addr {
                // Send ARP reply
                let reply = ArpPacket {
                    hardware_type: 1,
                    protocol_type: 0x0800,
                    hw_addr_len: 6,
                    proto_addr_len: 4,
                    operation: ArpOperation::Reply,
                    sender_hw_addr: cfg.mac_addr,
                    sender_proto_addr: cfg.ip_addr,
                    target_hw_addr: packet.sender_hw_addr,
                    target_proto_addr: packet.sender_proto_addr,
                };
                
                let _ = crate::net::interface::send_arp_reply(&reply);
            }
        }
        ArpOperation::Reply => {
            // Already cached above
        }
        _ => {}
    }
    
    Ok(())
}
