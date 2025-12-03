use alloc::vec::Vec;
use core::convert::TryInto;

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
