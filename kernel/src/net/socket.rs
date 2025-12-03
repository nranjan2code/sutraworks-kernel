use alloc::collections::VecDeque;
use crate::net::ip::Ipv4Addr;
use crate::net::tcp::TcpState;

/// Socket Type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketType {
    Stream,   // TCP
    Datagram, // UDP
}

/// Network Socket
pub struct Socket {
    pub socket_type: SocketType,
    pub state: TcpState, // Only relevant for TCP
    pub local_port: u16,
    pub remote_addr: Ipv4Addr,
    pub remote_port: u16,
    pub recv_buffer: VecDeque<u8>,
    pub send_buffer: VecDeque<u8>,
}

impl Socket {
    pub fn new(socket_type: SocketType) -> Self {
        Self {
            socket_type,
            state: TcpState::Closed,
            local_port: 0,
            remote_addr: Ipv4Addr::ANY,
            remote_port: 0,
            recv_buffer: VecDeque::new(),
            send_buffer: VecDeque::new(),
        }
    }
    
    pub fn bind(&mut self, port: u16) -> Result<(), &'static str> {
        if self.local_port != 0 {
            return Err("Socket already bound");
        }
        self.local_port = port;
        Ok(())
    }
    
    pub fn connect(&mut self, addr: Ipv4Addr, port: u16) -> Result<(), &'static str> {
        self.remote_addr = addr;
        self.remote_port = port;
        
        if self.socket_type == SocketType::Stream {
            // Start TCP Handshake
            self.state = TcpState::SynSent;
            // In a real stack, we would send a SYN packet here.
            // For now, we just update state.
        }
        
        Ok(())
    }
    
    pub fn send(&mut self, data: &[u8]) -> usize {
        for &byte in data {
            self.send_buffer.push_back(byte);
        }
        // In a real stack, this would trigger packet transmission.
        data.len()
    }
    
    pub fn recv(&mut self, buf: &mut [u8]) -> usize {
        let mut count = 0;
        for byte in buf.iter_mut() {
            if let Some(val) = self.recv_buffer.pop_front() {
                *byte = val;
                count += 1;
            } else {
                break;
            }
        }
        count
    }
}

use crate::fs::vfs::{FileOps, FileStat, SeekFrom};

impl FileOps for Socket {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        Ok(self.recv(buf))
    }
    
    fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str> {
        Ok(self.send(buf))
    }
    
    fn seek(&mut self, _pos: SeekFrom) -> Result<u64, &'static str> {
        Err("Socket does not support seeking")
    }
    
    fn close(&mut self) -> Result<(), &'static str> {
        self.state = TcpState::Closed;
        Ok(())
    }
    
    fn stat(&self) -> Result<FileStat, &'static str> {
        Ok(FileStat {
            size: 0,
            mode: 0, 
            inode: 0,
        })
    }
    
    fn as_any(&mut self) -> &mut dyn core::any::Any {
        self
    }
}
