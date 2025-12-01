//! Process Management
//!
//! Defines the Process Control Block (PCB) and associated structures.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use crate::kernel::memory::paging::VMM;

/// Unique Process Identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessId(pub u64);

impl ProcessId {
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        ProcessId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Process State
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessState {
    Ready,
    Running,
    Blocked,
    Terminated,
}

/// CPU Context (Callee-saved registers)
/// This matches the layout expected by `switch_to` in assembly.
#[repr(C)]
#[derive(Debug, Default)]
pub struct Context {
    pub x19: u64,
    pub x20: u64,
    pub x21: u64,
    pub x22: u64,
    pub x23: u64,
    pub x24: u64,
    pub x25: u64,
    pub x26: u64,
    pub x27: u64,
    pub x28: u64,
    pub x29: u64, // Frame Pointer
    pub sp: u64,  // Stack Pointer
    pub lr: u64,  // Link Register
    pub ttbr0: u64, // Page Table Base (User/Process space)
}

/// Process Control Block
pub struct Process {
    pub id: ProcessId,
    pub state: ProcessState,
    pub context: Context,
    pub vmm: Option<VMM>,
    pub kernel_stack: Vec<u8>,
    pub user_stack: Vec<u8>,
}

impl Process {
    pub fn new_kernel(entry: fn()) -> Self {
        let mut process = Process {
            id: ProcessId::new(),
            state: ProcessState::Ready,
            context: Context::default(),
            vmm: None,
            kernel_stack: alloc::vec![0u8; 16 * 1024], // 16KB stack
            user_stack: alloc::vec![], // No user stack
        };

        let stack_top = process.kernel_stack.as_ptr() as u64 + process.kernel_stack.len() as u64;
        
        // Align stack to 16 bytes
        let stack_top = stack_top & !0xF;

        process.context.sp = stack_top;
        process.context.lr = entry as u64;
        process.context.ttbr0 = 0; // Kernel threads share TTBR1, TTBR0 is unused/zeroed

        process
    }

    pub fn new_user(entry: fn(), arg: u64) -> Self {
        let mut process = Process {
            id: ProcessId::new(),
            state: ProcessState::Ready,
            context: Context::default(),
            vmm: None, // TODO: Create separate VMM
            kernel_stack: alloc::vec![0u8; 16 * 1024], // 16KB kernel stack
            user_stack: alloc::vec![0u8; 16 * 1024],   // 16KB user stack
        };

        // Kernel Stack Setup (for when we are in kernel mode handling this process)
        let kstack_top = process.kernel_stack.as_ptr() as u64 + process.kernel_stack.len() as u64;
        let kstack_top = kstack_top & !0xF;
        process.context.sp = kstack_top;

        // User Stack Setup (passed to jump_to_userspace)
        let ustack_top = process.user_stack.as_ptr() as u64 + process.user_stack.len() as u64;
        let ustack_top = ustack_top & !0xF;

        // Set up trampoline
        // switch_to restores x19..x29. We use them to pass args to jump_to_userspace.
        process.context.lr = user_trampoline as u64;
        process.context.x19 = entry as u64;      // Entry point
        process.context.x20 = ustack_top;        // User Stack
        process.context.x21 = arg;               // Argument

        process.context.ttbr0 = 0; // TODO: Set to user page table

        process
    }
}

/// Trampoline to jump to userspace
/// 
/// Called when `switch_to` returns for a user process.
/// Expects:
///   x19 = User Entry Point
///   x20 = User Stack Pointer
///   x21 = User Argument
extern "C" fn user_trampoline() {
    unsafe {
        let entry: u64;
        let stack: u64;
        let arg: u64;
        
        // Read from callee-saved registers
        core::arch::asm!("mov {}, x19", out(reg) entry);
        core::arch::asm!("mov {}, x20", out(reg) stack);
        core::arch::asm!("mov {}, x21", out(reg) arg);
        
        crate::arch::jump_to_userspace(entry, stack, arg);
    }
}
