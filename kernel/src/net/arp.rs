use alloc::vec::Vec;
use core::convert::TryInto;
use crate::net::ethernet::MacAddress;
use crate::net::ip::Ipv4Addr;

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
