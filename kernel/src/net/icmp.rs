use alloc::vec::Vec;
use core::convert::TryInto;
use crate::net::ip::Ipv4Addr;

/// ICMP Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IcmpType {
    EchoReply = 0,
    DestinationUnreachable = 3,
    EchoRequest = 8,
    Unknown(u8),
}

impl From<u8> for IcmpType {
    fn from(val: u8) -> Self {
        match val {
            0 => IcmpType::EchoReply,
            3 => IcmpType::DestinationUnreachable,
            8 => IcmpType::EchoRequest,
            _ => IcmpType::Unknown(val),
        }
    }
}

/// ICMP Packet
#[derive(Debug, Clone)]
pub struct IcmpPacket {
    pub icmp_type: IcmpType,
    pub code: u8,
    pub checksum: u16,
    pub rest_of_header: u32, // Identifier + Sequence Number for Echo
    pub payload: Vec<u8>,
}

impl IcmpPacket {
    pub fn parse(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 8 {
            return Err("Packet too short for ICMP header");
        }
        
        let icmp_type = IcmpType::from(data[0]);
        let code = data[1];
        let checksum = u16::from_be_bytes(data[2..4].try_into().unwrap());
        let rest_of_header = u32::from_be_bytes(data[4..8].try_into().unwrap());
        
        let payload = data[8..].to_vec();
        
        Ok(Self {
            icmp_type,
            code,
            checksum,
            rest_of_header,
            payload,
        })
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(8 + self.payload.len());
        
        let type_u8 = match self.icmp_type {
            IcmpType::EchoReply => 0,
            IcmpType::DestinationUnreachable => 3,
            IcmpType::EchoRequest => 8,
            IcmpType::Unknown(val) => val,
        };
        bytes.push(type_u8);
        bytes.push(self.code);
        bytes.extend_from_slice(&self.checksum.to_be_bytes());
        bytes.extend_from_slice(&self.rest_of_header.to_be_bytes());
        bytes.extend_from_slice(&self.payload);
        
        bytes
    }
}

/// Handle incoming ICMP packet
pub fn handle_packet(data: &[u8], src_ip: Ipv4Addr) -> Result<(), &'static str> {
    let packet = IcmpPacket::parse(data)?;
    
    match packet.icmp_type {
        IcmpType::EchoRequest => {
            // Send Echo Reply
            let reply = IcmpPacket {
                icmp_type: IcmpType::EchoReply,
                code: 0,
                checksum: 0,
                rest_of_header: packet.rest_of_header, // Keep same ID/Seq
                payload: packet.payload,
            };
            
            // Calculate checksum
            // 1. Serialize with checksum 0
            let mut reply_bytes = reply.to_bytes();
            
            // 2. Calculate checksum over serialized bytes
            let checksum = crate::net::checksum(&reply_bytes);
            
            // 3. Insert checksum into bytes (offset 2)
            reply_bytes[2] = (checksum >> 8) as u8;
            reply_bytes[3] = (checksum & 0xFF) as u8;
            
            // Send via IP layer (protocol 1 = ICMP)
            crate::net::ipv4::send_packet(src_ip, 1, &reply_bytes)?;
        }
        _ => {} // Ignore other ICMP types for now
    }
    
    Ok(())
}
