use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use core::convert::TryInto;
use crate::net::ip::Ipv4Addr;
use crate::arch::SpinLock;

/// UDP Packet
#[derive(Debug, Clone)]
pub struct UdpPacket {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub checksum: u16,
    pub payload: Vec<u8>,
}

impl UdpPacket {
    pub fn parse(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 8 {
            return Err("Packet too short for UDP header");
        }
        
        let src_port = u16::from_be_bytes(data[0..2].try_into().unwrap());
        let dst_port = u16::from_be_bytes(data[2..4].try_into().unwrap());
        let length = u16::from_be_bytes(data[4..6].try_into().unwrap());
        let checksum = u16::from_be_bytes(data[6..8].try_into().unwrap());
        
        let payload_len = (length as usize).saturating_sub(8);
        if data.len() < 8 + payload_len {
            return Err("Packet shorter than UDP length field");
        }
        
        let payload = data[8..8+payload_len].to_vec();
        
        Ok(Self {
            src_port,
            dst_port,
            length,
            checksum,
            payload,
        })
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(8 + self.payload.len());
        
        bytes.extend_from_slice(&self.src_port.to_be_bytes());
        bytes.extend_from_slice(&self.dst_port.to_be_bytes());
        bytes.extend_from_slice(&self.length.to_be_bytes());
        bytes.extend_from_slice(&self.checksum.to_be_bytes());
        bytes.extend_from_slice(&self.payload);
        
        bytes
    }
}

/// UDP packet with source address
#[derive(Debug, Clone)]
pub struct UdpMessage {
    pub src_addr: Ipv4Addr,
    pub src_port: u16,
    pub payload: Vec<u8>,
}

/// UDP Listener Queue
pub struct UdpListener {
    port: u16,
    queue: alloc::collections::VecDeque<UdpMessage>,
    max_queue: usize,
}

impl UdpListener {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            queue: alloc::collections::VecDeque::new(),
            max_queue: 64, // Max 64 pending packets per port
        }
    }
    
    /// Enqueue a received packet
    pub fn enqueue(&mut self, msg: UdpMessage) -> Result<(), &'static str> {
        if self.queue.len() >= self.max_queue {
            return Err("UDP queue full");
        }
        self.queue.push_back(msg);
        Ok(())
    }
    
    /// Dequeue a packet (non-blocking)
    pub fn dequeue(&mut self) -> Option<UdpMessage> {
        self.queue.pop_front()
    }
    
    /// Check if queue has packets
    pub fn has_packets(&self) -> bool {
        !self.queue.is_empty()
    }
}

/// Global UDP Listener Registry
static UDP_LISTENERS: SpinLock<BTreeMap<u16, UdpListener>> = SpinLock::new(BTreeMap::new());

/// Register a UDP listener on a specific port
pub fn register_listener(port: u16) -> Result<(), &'static str> {
    let mut listeners = UDP_LISTENERS.lock();
    
    if listeners.contains_key(&port) {
        return Err("Port already in use");
    }
    
    listeners.insert(port, UdpListener::new(port));
    Ok(())
}

/// Unregister a UDP listener
pub fn unregister_listener(port: u16) -> Result<(), &'static str> {
    let mut listeners = UDP_LISTENERS.lock();
    
    if listeners.remove(&port).is_some() {
        Ok(())
    } else {
        Err("Port not registered")
    }
}

/// Receive a packet from a registered port (non-blocking)
pub fn recv_from(port: u16) -> Option<UdpMessage> {
    let mut listeners = UDP_LISTENERS.lock();
    
    if let Some(listener) = listeners.get_mut(&port) {
        listener.dequeue()
    } else {
        None
    }
}

/// Handle incoming UDP packet
pub fn handle_packet(data: &[u8], src_ip: Ipv4Addr) -> Result<(), &'static str> {
    let packet = UdpPacket::parse(data)?;
    
    // Dispatch to registered listener
    let mut listeners = UDP_LISTENERS.lock();
    if let Some(listener) = listeners.get_mut(&packet.dst_port) {
        let msg = UdpMessage {
            src_addr: src_ip,
            src_port: packet.src_port,
            payload: packet.payload,
        };
        
        listener.enqueue(msg)?;
    } else {
        // No listener registered for this port - silently drop
        // In production, could send ICMP Port Unreachable
    }
    
    Ok(())
}

