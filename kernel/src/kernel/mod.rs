//! Kernel Subsystems
//!
//! Core kernel functionality including:
//! - Memory management
//! - Capability-based security
//! - Scheduler (future)
//! - IPC (future)

pub mod memory;
pub mod capability;
pub mod exception;
pub mod async_core;
pub mod process;
pub mod scheduler;
pub mod syscall;
pub mod elf;
pub mod signal;

// Re-export key types
pub use memory::{PAGE_SIZE, stats as memory_stats};
pub use capability::{
    Capability, CapabilityType, Permissions, CapError,
    mint_root, derive, revoke, validate,
};

/// Initialize all kernel subsystems
/// 
/// # Safety
/// Must be called once during boot
pub unsafe fn init(seed: u64) {
    // Initialize memory allocator first
    memory::init(seed);
    memory::init_dma();
    
    // Initialize capability system
    capability::init();
}
