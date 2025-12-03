//! Pipe Implementation
//!
//! Unidirectional data channel.

use alloc::collections::vec_deque::VecDeque;
use alloc::sync::Arc;
use crate::arch::SpinLock;
use crate::fs::vfs::{FileOps, SeekFrom, FileStat};

use alloc::vec::Vec;
use crate::kernel::scheduler::SCHEDULER;
use crate::kernel::process::AgentState;

struct PipeState {
    buffer: VecDeque<u8>,
    closed_read: bool,
    closed_write: bool,
    waiting_readers: Vec<u64>, // Agent IDs
}

pub struct PipeReader {
    state: Arc<SpinLock<PipeState>>,
}

pub struct PipeWriter {
    state: Arc<SpinLock<PipeState>>,
}

pub fn create_pipe() -> (PipeReader, PipeWriter) {
    let state = Arc::new(SpinLock::new(PipeState {
        buffer: VecDeque::new(),
        closed_read: false,
        closed_write: false,
        waiting_readers: Vec::new(),
    }));

    (
        PipeReader { state: state.clone() },
        PipeWriter { state },
    )
}

impl FileOps for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        loop {
            let mut state = self.state.lock();
            
            if !state.buffer.is_empty() {
                let mut read = 0;
                for b in buf.iter_mut() {
                    if let Some(byte) = state.buffer.pop_front() {
                        *b = byte;
                        read += 1;
                    } else {
                        break;
                    }
                }
                return Ok(read);
            }
            
            if state.closed_write {
                return Ok(0); // EOF
            }
            
            // Block
            let mut scheduler = SCHEDULER.lock();
            let current_id = scheduler.with_current_agent(|agent| {
                agent.state = AgentState::Blocked;
                agent.id.0
            });
            
            if let Some(id) = current_id {
                state.waiting_readers.push(id);
                drop(state); // Drop pipe lock before yielding
                drop(scheduler); // Drop scheduler lock
                
                crate::kernel::scheduler::yield_task();
            } else {
                // No current agent (kernel thread?), just return 0
                return Ok(0);
            }
        }
    }

    fn write(&mut self, _buf: &[u8]) -> Result<usize, &'static str> {
        Err("Bad file descriptor")
    }

    fn seek(&mut self, _pos: SeekFrom) -> Result<u64, &'static str> {
        Err("Illegal seek")
    }

    fn close(&mut self) -> Result<(), &'static str> {
        let mut state = self.state.lock();
        state.closed_read = true;
        Ok(())
    }

    fn stat(&self) -> Result<FileStat, &'static str> {
        Ok(FileStat {
            size: 0,
            mode: 0x1000, // S_IFIFO
            inode: 0,
        })
    }

    fn as_any(&mut self) -> &mut dyn core::any::Any {
        self
    }
}

impl FileOps for PipeWriter {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, &'static str> {
        Err("Bad file descriptor")
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str> {
        let mut state = self.state.lock();
        
        if state.closed_read {
            return Err("Broken pipe");
        }

        for &b in buf {
            state.buffer.push_back(b);
        }
        
        // Wake up readers
        if !state.waiting_readers.is_empty() {
            let mut scheduler = SCHEDULER.lock();
            for id in state.waiting_readers.drain(..) {
                if let Some(agent) = scheduler.get_agent_mut(id) {
                    if agent.state == AgentState::Blocked {
                        agent.state = AgentState::Ready;
                    }
                }
            }
        }
        
        Ok(buf.len())
    }

    fn seek(&mut self, _pos: SeekFrom) -> Result<u64, &'static str> {
        Err("Illegal seek")
    }

    fn close(&mut self) -> Result<(), &'static str> {
        let mut state = self.state.lock();
        state.closed_write = true;
        
        // Wake up readers so they see EOF
        if !state.waiting_readers.is_empty() {
            let mut scheduler = SCHEDULER.lock();
            for id in state.waiting_readers.drain(..) {
                if let Some(agent) = scheduler.get_agent_mut(id) {
                    if agent.state == AgentState::Blocked {
                        agent.state = AgentState::Ready;
                    }
                }
            }
        }
        
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipe_read_write() {
        let (mut reader, mut writer) = create_pipe();
        let mut buf = [0u8; 10];

        // Write to pipe
        assert_eq!(writer.write(b"hello").unwrap(), 5);

        // Read from pipe
        assert_eq!(reader.read(&mut buf).unwrap(), 5);
        assert_eq!(&buf[0..5], b"hello");
    }

    #[test]
    fn test_pipe_empty() {
        let (mut reader, _writer) = create_pipe();
        let mut buf = [0u8; 10];

        // Read from empty pipe (should return 0 or block, currently 0)
        assert_eq!(reader.read(&mut buf).unwrap(), 0);
    }
}
