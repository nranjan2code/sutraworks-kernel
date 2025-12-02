//! Agent Management
//!
//! Defines the Agent Control Block (ACB) and associated structures.
//! Simplified for stroke-native kernel.


use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use core::sync::atomic::{AtomicU64, Ordering};
use crate::kernel::memory::paging::{VMM, UserAddressSpace};
use crate::kernel::capability::Capability;

/// Unique Agent Identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AgentId(pub u64);

impl AgentId {
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        AgentId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Agent State
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
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

/// Agent Control Block
/// 
/// An Agent is a lightweight execution unit.
pub struct Agent {
    pub id: AgentId,
    pub state: AgentState,
    pub context: Context,
    pub capabilities: Vec<Capability>,
    pub vmm: Option<UserAddressSpace>,
    pub kernel_stack: Vec<u8>,
    pub user_stack: Vec<u8>,
}

impl Agent {
    /// Create a new kernel agent (simple)
    pub fn new_kernel_simple(entry: fn()) -> Self {
        let mut agent = Agent {
            id: AgentId::new(),
            state: AgentState::Ready,
            context: Context::default(),
            capabilities: Vec::new(),
            vmm: None,
            kernel_stack: alloc::vec![0u8; 16 * 1024], // 16KB stack
            user_stack: alloc::vec![], // No user stack
        };

        let stack_top = agent.kernel_stack.as_ptr() as u64 + agent.kernel_stack.len() as u64;
        
        // Align stack to 16 bytes
        let stack_top = stack_top & !0xF;

        agent.context.sp = stack_top;
        agent.context.lr = entry as u64;
        agent.context.ttbr0 = 0; // Kernel threads share TTBR1, TTBR0 is unused/zeroed

        agent
    }

    /// Create a new user agent (simple)
    pub fn new_user_simple(entry: fn(), arg: u64) -> Self {
        // 1. Create Address Space
        let mut space = UserAddressSpace::new().expect("Failed to create user address space");
        
        // 2. Allocate Stacks (Kernel & User)
        let kernel_stack = alloc::vec![0u8; 16 * 1024];
        let user_stack = alloc::vec![0u8; 16 * 1024];
        
        // 3. Map User Stack into Address Space
        // We identity map it for now, but with User Permissions
        let ustack_phys = user_stack.as_ptr() as u64;
        let ustack_size = user_stack.len();
        space.map_user(ustack_phys, ustack_phys, ustack_size).expect("Failed to map user stack");
        
        // 4. Map User Code (The entry point)
        // We assume the entry point is in the kernel binary, so it's already mapped as EL1.
        // We need to remap that specific page as User Accessible.
        // This is tricky because it might overlap with kernel code we want to protect.
        // For this "Simple" prototype, we'll just map the page containing the function.
        let entry_phys = entry as u64;
        let entry_page = entry_phys & !0xFFF;
        space.map_user(entry_page, entry_page, 4096).expect("Failed to map user code");

        let mut agent = Agent {
            id: AgentId::new(),
            state: AgentState::Ready,
            context: Context::default(),
            capabilities: Vec::new(),
            vmm: Some(space),
            kernel_stack,
            user_stack,
        };

        // Kernel Stack Setup (for when we are in kernel mode handling this process)
        let kstack_top = agent.kernel_stack.as_ptr() as u64 + agent.kernel_stack.len() as u64;
        let kstack_top = kstack_top & !0xF;
        agent.context.sp = kstack_top;

        // User Stack Setup (passed to jump_to_userspace)
        let ustack_top = agent.user_stack.as_ptr() as u64 + agent.user_stack.len() as u64;
        let ustack_top = ustack_top & !0xF;

        // Set up trampoline
        // switch_to restores x19..x29. We use them to pass args to jump_to_userspace.
        agent.context.lr = user_trampoline as *const () as u64;
        agent.context.x19 = entry as u64;      // Entry point
        agent.context.x20 = ustack_top;        // User Stack
        agent.context.x21 = arg;               // Argument

        // Set TTBR0 to the new User Table
        agent.context.ttbr0 = agent.vmm.as_ref().unwrap().table_base();

        agent
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
