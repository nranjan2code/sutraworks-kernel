use alloc::vec::Vec;
use core::convert::TryInto;

/// Ethernet Protocol Type (EtherType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum EtherType {
    IPv4 = 0x0800,
    ARP = 0x0806,
    IPv6 = 0x86DD,
    Unknown(u16),
}

impl From<u16> for EtherType {
    fn from(val: u16) -> Self {
        match val {
            0x0800 => EtherType::IPv4,
            0x0806 => EtherType::ARP,
            0x86DD => EtherType::IPv6,
            _ => EtherType::Unknown(val),
        }
    }
}

/// Ethernet MAC Address
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacAddress(pub [u8; 6]);

impl MacAddress {
    pub const BROADCAST: Self = Self([0xFF; 6]);
    pub const ZERO: Self = Self([0; 6]);
}

/// Ethernet Frame
#[derive(Debug, Clone)]
pub struct EthernetFrame {
    pub dst: MacAddress,
    pub src: MacAddress,
    pub ethertype: EtherType,
    pub payload: Vec<u8>,
}

impl EthernetFrame {
    /// Parse an Ethernet frame from raw bytes
    pub fn parse(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 14 {
            return Err("Packet too short for Ethernet header");
        }
        
        let dst = MacAddress(data[0..6].try_into().unwrap());
        let src = MacAddress(data[6..12].try_into().unwrap());
        let ethertype_u16 = u16::from_be_bytes(data[12..14].try_into().unwrap());
        let ethertype = EtherType::from(ethertype_u16);
        
        let payload = data[14..].to_vec();
        
        Ok(Self {
            dst,
            src,
            ethertype,
            payload,
        })
    }
    
    /// Serialize the Ethernet frame to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(14 + self.payload.len());
        
        bytes.extend_from_slice(&self.dst.0);
        bytes.extend_from_slice(&self.src.0);
        
        let ethertype_u16 = match self.ethertype {
            EtherType::IPv4 => 0x0800,
            EtherType::ARP => 0x0806,
            EtherType::IPv6 => 0x86DD,
            EtherType::Unknown(val) => val,
        };
        bytes.extend_from_slice(&ethertype_u16.to_be_bytes());
        
        bytes.extend_from_slice(&self.payload);
        
        bytes
    }
}
