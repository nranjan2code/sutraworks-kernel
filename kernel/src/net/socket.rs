//! Socket Filesystem Interface
//! 
//! Bridges the VFS `FileOps` trait with the Network Stack.

use alloc::vec::Vec;
use alloc::sync::Arc;
use core::any::Any;
use crate::fs::vfs::FileStat;
use crate::fs::{FileOps, SeekFrom};
use crate::net::udp;
use crate::kprintln;

/// A File representing a bound UDP socket
pub struct SocketFile {
    pub port: u16,
}

impl SocketFile {
    pub fn new(port: u16) -> Self {
        Self { port }
    }
}

impl FileOps for SocketFile {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, &'static str> {
        // Standard read() pulls payload only? 
        // Or should we return error and force recvfrom?
        // Ideally `read` should return just data for connected sockets.
        // For connectionless, it returns data from the first packet in queue.
        
        // This is a blocking read in a real OS, but here we are non-blocking or just check queue.
        // Let's implement non-blocking read for now.
        
        if let Some(msg) = udp::recv_from(self.port) {
            let len = core::cmp::min(_buf.len(), msg.payload.len());
            _buf[..len].copy_from_slice(&msg.payload[..len]);
            Ok(len)
        } else {
            Ok(0) // No data (EAGAIN in unix)
        }
    }

    fn write(&mut self, _buf: &[u8]) -> Result<usize, &'static str> {
        // write() on UDP socket requires connect() first, which we don't have.
        // So this should probably error or default to broadcast?
        Err("Use sendto instead")
    }

    fn seek(&mut self, _pos: SeekFrom) -> Result<u64, &'static str> {
        Err("Cannot seek on socket")
    }

    fn close(&mut self) -> Result<(), &'static str> {
        kprintln!("[Socket] Closing port {}", self.port);
        udp::unregister_listener(self.port)?;
        Ok(())
    }

    fn stat(&self) -> Result<FileStat, &'static str> {
        Ok(FileStat {
            size: 0,
            mode: 0, // S_IFSOCK
            inode: 0, // TODO
        })
    }
    
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

impl Drop for SocketFile {
    fn drop(&mut self) {
        // Ensure we unregister if dropped without close
        let _ = udp::unregister_listener(self.port);
    }
}
