//! System Call Interface
//!
//! Handles requests from User Mode (EL0).

use crate::kernel::process::Process;
use crate::kernel::scheduler;

/// System Call Numbers
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u64)]
pub enum SyscallNumber {
    Yield = 1,
    Print = 2,
    Sleep = 3,
    Unknown,
}

impl From<u64> for SyscallNumber {
    fn from(n: u64) -> Self {
        match n {
            1 => SyscallNumber::Yield,
            2 => SyscallNumber::Print,
            3 => SyscallNumber::Sleep,
            _ => SyscallNumber::Unknown,
        }
    }
}

/// Handle a system call
/// 
/// Arguments are passed in x0-x7.
/// Return value is placed in x0.
pub fn dispatcher(num: u64, arg0: u64, arg1: u64, _arg2: u64) -> u64 {
    let syscall = SyscallNumber::from(num);
    
    match syscall {
        SyscallNumber::Yield => {
            scheduler::yield_task();
            0
        }
        SyscallNumber::Print => {
            // arg0 = ptr, arg1 = len
            let ptr = arg0 as *const u8;
            let len = arg1 as usize;
            
            if len > 1024 {
                return u64::MAX; // Error: Too long
            }
            
            // Validate pointer!
            match crate::kernel::memory::validate_user_str(ptr, len) {
                Ok(s) => {
                    crate::kprint!("{}", s);
                    0
                }
                Err(_) => {
                    u64::MAX // Error: Invalid pointer or UTF-8
                }
            }
        }
        SyscallNumber::Sleep => {
            // arg0 = ms
            crate::drivers::timer::delay_ms(arg0);
            0
        }
        SyscallNumber::Unknown => {
            crate::kprintln!("Unknown syscall: {}", num);
            u64::MAX
        }
    }
}
