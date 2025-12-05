use alloc::vec::Vec;
use core::convert::TryInto;

/// IPv4 Address
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv4Addr(pub [u8; 4]);

impl Ipv4Addr {
    pub const BROADCAST: Self = Self([255, 255, 255, 255]);
    pub const LOOPBACK: Self = Self([127, 0, 0, 1]);
    pub const ANY: Self = Self([0, 0, 0, 0]);
    
    pub fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self([a, b, c, d])
    }
}

/// IPv4 Protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IpProtocol {
    ICMP = 1,
    TCP = 6,
    UDP = 17,
    Unknown(u8),
}

impl From<u8> for IpProtocol {
    fn from(val: u8) -> Self {
        match val {
            1 => IpProtocol::ICMP,
            6 => IpProtocol::TCP,
            17 => IpProtocol::UDP,
            _ => IpProtocol::Unknown(val),
        }
    }
}

/// IPv4 Header
#[derive(Debug, Clone)]
pub struct Ipv4Header {
    pub version: u8, // 4 bits
    pub ihl: u8,     // 4 bits (Internet Header Length in 32-bit words)
    pub dscp: u8,
    pub ecn: u8,
    pub total_length: u16,
    pub identification: u16,
    pub flags: u8,
    pub fragment_offset: u16,
    pub ttl: u8,
    pub protocol: IpProtocol,
    pub checksum: u16,
    pub src: Ipv4Addr,
    pub dst: Ipv4Addr,
}

impl Ipv4Header {
    pub fn parse(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 20 {
            return Err("Packet too short for IPv4 header");
        }
        
        let version_ihl = data[0];
        let version = version_ihl >> 4;
        let ihl = version_ihl & 0x0F;
        
        if version != 4 {
            return Err("Not an IPv4 packet");
        }
        
        if ihl < 5 {
            return Err("Invalid IHL");
        }
        
        let dscp_ecn = data[1];
        let dscp = dscp_ecn >> 2;
        let ecn = dscp_ecn & 0x03;
        
        let total_length = u16::from_be_bytes(data[2..4].try_into().unwrap());
        let identification = u16::from_be_bytes(data[4..6].try_into().unwrap());
        
        let flags_frag = u16::from_be_bytes(data[6..8].try_into().unwrap());
        let flags = (flags_frag >> 13) as u8;
        let fragment_offset = flags_frag & 0x1FFF;
        
        let ttl = data[8];
        let protocol = IpProtocol::from(data[9]);
        let checksum = u16::from_be_bytes(data[10..12].try_into().unwrap());
        
        let src = Ipv4Addr(data[12..16].try_into().unwrap());
        let dst = Ipv4Addr(data[16..20].try_into().unwrap());
        
        Ok(Self {
            version,
            ihl,
            dscp,
            ecn,
            total_length,
            identification,
            flags,
            fragment_offset,
            ttl,
            protocol,
            checksum,
            src,
            dst,
        })
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(20);
        
        let version_ihl = (self.version << 4) | (self.ihl & 0x0F);
        bytes.push(version_ihl);
        
        let dscp_ecn = (self.dscp << 2) | (self.ecn & 0x03);
        bytes.push(dscp_ecn);
        
        bytes.extend_from_slice(&self.total_length.to_be_bytes());
        bytes.extend_from_slice(&self.identification.to_be_bytes());
        
        let flags_frag = ((self.flags as u16) << 13) | (self.fragment_offset & 0x1FFF);
        bytes.extend_from_slice(&flags_frag.to_be_bytes());
        
        bytes.push(self.ttl);
        
        let proto_u8 = match self.protocol {
            IpProtocol::ICMP => 1,
            IpProtocol::TCP => 6,
            IpProtocol::UDP => 17,
            IpProtocol::Unknown(val) => val,
        };
        bytes.push(proto_u8);
        
        // Checksum initialized to 0 for calculation
        bytes.extend_from_slice(&self.checksum.to_be_bytes());
        
        bytes.extend_from_slice(&self.src.0);
        bytes.extend_from_slice(&self.dst.0);
        
        bytes
    }
}
