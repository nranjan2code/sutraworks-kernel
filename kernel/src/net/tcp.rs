use alloc::vec::Vec;
use core::convert::TryInto;

/// TCP Flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpFlags(u16);

impl TcpFlags {
    pub const FIN: u16 = 1 << 0;
    pub const SYN: u16 = 1 << 1;
    pub const RST: u16 = 1 << 2;
    pub const PSH: u16 = 1 << 3;
    pub const ACK: u16 = 1 << 4;
    pub const URG: u16 = 1 << 5;
    pub const ECE: u16 = 1 << 6;
    pub const CWR: u16 = 1 << 7;
    
    pub fn new(val: u16) -> Self {
        Self(val)
    }
    
    pub fn contains(&self, flag: u16) -> bool {
        (self.0 & flag) != 0
    }
    
    pub fn bits(&self) -> u16 {
        self.0
    }
}

/// TCP State Machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

/// TCP Segment
#[derive(Debug, Clone)]
pub struct TcpSegment {
    pub src_port: u16,
    pub dst_port: u16,
    pub sequence_num: u32,
    pub ack_num: u32,
    pub data_offset: u8, // 4 bits (number of 32-bit words)
    pub flags: TcpFlags,
    pub window_size: u16,
    pub checksum: u16,
    pub urgent_pointer: u16,
    pub options: Vec<u8>,
    pub payload: Vec<u8>,
}

impl TcpSegment {
    pub fn parse(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 20 {
            return Err("Packet too short for TCP header");
        }
        
        let src_port = u16::from_be_bytes(data[0..2].try_into().unwrap());
        let dst_port = u16::from_be_bytes(data[2..4].try_into().unwrap());
        let sequence_num = u32::from_be_bytes(data[4..8].try_into().unwrap());
        let ack_num = u32::from_be_bytes(data[8..12].try_into().unwrap());
        
        let data_offset_res = data[12];
        let data_offset = data_offset_res >> 4;
        let header_len = (data_offset as usize) * 4;
        
        if data.len() < header_len {
            return Err("Packet shorter than TCP header length");
        }
        
        let flags_u16 = u16::from_be_bytes(data[12..14].try_into().unwrap()) & 0x01FF;
        let flags = TcpFlags::new(flags_u16);
        
        let window_size = u16::from_be_bytes(data[14..16].try_into().unwrap());
        let checksum = u16::from_be_bytes(data[16..18].try_into().unwrap());
        let urgent_pointer = u16::from_be_bytes(data[18..20].try_into().unwrap());
        
        let options = data[20..header_len].to_vec();
        let payload = data[header_len..].to_vec();
        
        Ok(Self {
            src_port,
            dst_port,
            sequence_num,
            ack_num,
            data_offset,
            flags,
            window_size,
            checksum,
            urgent_pointer,
            options,
            payload,
        })
    }
    
    pub fn to_bytes(&self) -> Vec<u8> {
        let header_len = 20 + self.options.len();
        let data_offset = (header_len / 4) as u8;
        
        let mut bytes = Vec::with_capacity(header_len + self.payload.len());
        
        bytes.extend_from_slice(&self.src_port.to_be_bytes());
        bytes.extend_from_slice(&self.dst_port.to_be_bytes());
        bytes.extend_from_slice(&self.sequence_num.to_be_bytes());
        bytes.extend_from_slice(&self.ack_num.to_be_bytes());
        
        let offset_res = (data_offset << 4) | ((self.flags.bits() >> 8) as u8);
        bytes.push(offset_res);
        bytes.push(self.flags.bits() as u8);
        
        bytes.extend_from_slice(&self.window_size.to_be_bytes());
        bytes.extend_from_slice(&self.checksum.to_be_bytes());
        bytes.extend_from_slice(&self.urgent_pointer.to_be_bytes());
        
        bytes.extend_from_slice(&self.options);
        
        // Padding if needed? Options should be 4-byte aligned.
        
        bytes.extend_from_slice(&self.payload);
        
        bytes
    }
}
