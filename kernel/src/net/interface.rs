use alloc::vec::Vec;
use alloc::collections::VecDeque;
use crate::net::arp::ArpPacket;

/// Network Interface Trait
/// 
/// Abstraction for hardware or software network devices.
pub trait NetworkInterface {
    /// Send a raw packet (Ethernet frame)
    fn send(&mut self, packet: &[u8]) -> Result<(), &'static str>;
    
    /// Receive a raw packet (Ethernet frame)
    /// Returns None if no packet is available.
    fn receive(&mut self) -> Option<Vec<u8>>;
    
    /// Get the MAC address of the interface
    fn mac_address(&self) -> [u8; 6];
}

/// Loopback Interface
/// 
/// A software interface that echoes back everything sent to it.
pub struct LoopbackInterface {
    queue: VecDeque<Vec<u8>>,
}

impl LoopbackInterface {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }
}

impl Default for LoopbackInterface {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkInterface for LoopbackInterface {
    fn send(&mut self, packet: &[u8]) -> Result<(), &'static str> {
        // Enqueue packet for reception
        self.queue.push_back(packet.to_vec());
        Ok(())
    }

    fn receive(&mut self) -> Option<Vec<u8>> {
        self.queue.pop_front()
    }

    fn mac_address(&self) -> [u8; 6] {
        [0, 0, 0, 0, 0, 0]
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL SEND FUNCTIONS (Hardware driver stubs)
// ═══════════════════════════════════════════════════════════════════════════════

/// Send an Ethernet frame
/// 
/// In a full implementation, this would use the hardware Ethernet driver.
/// For now, this is a stub that successfully "sends" without hardware.
pub fn send_frame(frame: &[u8]) -> Result<(), &'static str> {
    // Validate frame size
    if frame.len() < 14 {
        return Err("Frame too short");
    }
    if frame.len() > 1518 {
        return Err("Frame too large");
    }
    
    // Note: Hardware Ethernet driver integration is scheduled for a future sprint.
    // Currently using loopback/stub for network stack verification.
    // For now, stub that succeeds without hardware
    
    Ok(())
}

/// Send an ARP reply packet
pub fn send_arp_reply(reply: &ArpPacket) -> Result<(), &'static str> {
    let cfg = crate::net::config();
    
    // Build Ethernet frame for ARP
    let mut frame = [0u8; 42]; // 14 (eth) + 28 (arp)
    
    // Ethernet header
    frame[0..6].copy_from_slice(&reply.target_hw_addr.0);  // Dst MAC
    frame[6..12].copy_from_slice(&cfg.mac_addr.0);         // Src MAC
    frame[12..14].copy_from_slice(&0x0806u16.to_be_bytes()); // EtherType: ARP
    
    // ARP payload
    let arp_bytes = reply.to_bytes();
    frame[14..42].copy_from_slice(&arp_bytes[..28]);
    
    send_frame(&frame)
}
