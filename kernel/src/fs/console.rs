use alloc::sync::Arc;
use core::any::Any;
use crate::fs::vfs::{self, FileOps, SeekFrom, FileStat};
use crate::kernel::sync::SpinLock;
use crate::drivers::uart;

pub struct ConsoleFile;

impl ConsoleFile {
    pub fn new() -> Arc<SpinLock<Self>> {
        Arc::new(SpinLock::new(ConsoleFile))
    }
}

impl FileOps for ConsoleFile {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        // Blocking read of at least one byte
        if buf.is_empty() { return Ok(0); }
        
        // Read one byte (blocking)
        let byte = uart::receive();
        buf[0] = byte;
        
        // If buffer is larger, we could try to read more if available?
        // But for interactive shell, one by one is fine/safer.
        
        Ok(1)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str> {
        for &b in buf {
            uart::send(b);
        }
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> Result<u64, &'static str> {
        Err("Console is not seekable")
    }

    fn close(&mut self) -> Result<(), &'static str> {
        Ok(())
    }

    fn stat(&self) -> Result<FileStat, &'static str> {
         Ok(FileStat { size: 0, mode: 0, inode: 0 })
    }
    
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}
