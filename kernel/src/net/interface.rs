use alloc::vec::Vec;
use alloc::collections::VecDeque;

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
