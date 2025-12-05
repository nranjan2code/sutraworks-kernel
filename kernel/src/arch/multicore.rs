//! Multi-Core (SMP) Support for ARM64
//!
//! Provides functions to boot secondary cores, read core IDs, and manage
//! inter-processor communication on Raspberry Pi 5.

use crate::kprintln;

/// Maximum number of cores on Raspberry Pi 5
pub const MAX_CORES: usize = 4;

/// Per-core mailbox for wakeup communication
/// Physical address: 0x400000E0 + (core_id * 0x10)
const CORE_MAILBOX_BASE: u64 = 0x400000E0;

/// Read the current core ID from MPIDR_EL1
#[inline(always)]
#[cfg(not(feature = "test_mocks"))]

pub fn core_id() -> usize {
    let mpidr: u64;
    unsafe {
        core::arch::asm!(
            "mrs {}, mpidr_el1",
            out(reg) mpidr,
            options(pure, nomem, nostack)
        );
    }
    // Extract Aff0 (bits 0-7) - core ID within cluster
    (mpidr & 0xFF) as usize
}

#[cfg(feature = "test_mocks")]
pub fn core_id() -> usize {
    0
}

/// Boot a secondary core
///
/// # Arguments
/// * `core_id` - Core to boot (1, 2, or 3)
/// * `entry_addr` - Physical address of entry function
///
/// # Safety
/// Must only be called once per core. Entry function must be valid.
pub unsafe fn start_core(core_id: usize, entry_addr: u64) -> Result<(), &'static str> {
    if core_id == 0 || core_id >= MAX_CORES {
        return Err("Invalid core ID");
    }

    kprintln!("[MULTICORE] Starting core {}...", core_id);

    // Calculate mailbox address for this core
    let mailbox_addr = (CORE_MAILBOX_BASE + (core_id as u64 * 0x10)) as *mut u64;

    // Write entry address to core's mailbox
    // Core will jump to this address on wakeup
    core::ptr::write_volatile(mailbox_addr, entry_addr);

    // Send event to wake the core
    core::arch::asm!("sev", options(nomem, nostack));

    kprintln!("[MULTICORE] Core {} mailbox written: 0x{:x}", core_id, entry_addr);

    Ok(())
}

/// Send Inter-Processor Interrupt (IPI) to a core
///
/// Uses ARM Generic Interrupt Controller (GIC) to signal another core.
pub fn send_ipi(target_core: usize) -> Result<(), &'static str> {
    if target_core >= MAX_CORES {
        return Err("Invalid target core");
    }

    // Use SGI 0 for generic IPI
    crate::drivers::interrupts::send_ipi(target_core, 0);
    
    Ok(())
}

/// Halt the current core
///
/// Puts the core into low-power WFI (Wait For Interrupt) state indefinitely.
/// Core can be woken by IPI or hardware interrupt.
pub fn halt_core() -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfi", options(nomem, nostack));
        }
    }
}

/// Wait for all cores to reach a synchronization point
///
/// Uses atomic counters to implement a barrier.
pub fn barrier(num_cores: usize) {
    use core::sync::atomic::{AtomicUsize, Ordering};
    
    static BARRIER_COUNTER: AtomicUsize = AtomicUsize::new(0);
    static BARRIER_SENSE: AtomicUsize = AtomicUsize::new(0);
    
    let current_sense = BARRIER_SENSE.load(Ordering::Acquire);
    let count = BARRIER_COUNTER.fetch_add(1, Ordering::AcqRel);
    
    if count + 1 == num_cores {
        // Last core to arrive - reset for next barrier
        BARRIER_COUNTER.store(0, Ordering::Release);
        BARRIER_SENSE.fetch_xor(1, Ordering::Release);
    } else {
        // Wait for sense to flip
        while BARRIER_SENSE.load(Ordering::Acquire) == current_sense {
            core::hint::spin_loop();
        }
    }
}

/// Get the number of cores currently active
pub fn count_active_cores() -> usize {
    use core::sync::atomic::{AtomicUsize, Ordering};
    
    static ACTIVE_CORES: AtomicUsize = AtomicUsize::new(1); // Core 0 always active
    
    ACTIVE_CORES.load(Ordering::Relaxed)
}

/// Register a core as active (called by secondary core during boot)
pub fn register_core() {
    use core::sync::atomic::{AtomicUsize, Ordering};
    
    static ACTIVE_CORES: AtomicUsize = AtomicUsize::new(1);
    
    ACTIVE_CORES.fetch_add(1, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_id_bounds() {
        let id = core_id();
        assert!(id < MAX_CORES, "Core ID out of bounds");
    }

    #[test]
    fn test_invalid_core_start() {
        unsafe {
            assert!(start_core(0, 0x1000).is_err()); // Can't start core 0
            assert!(start_core(4, 0x1000).is_err()); // Core 4 doesn't exist
            assert!(start_core(5, 0x1000).is_err()); // Core 5 doesn't exist
        }
    }

    #[test]
    fn test_invalid_ipi() {
        assert!(send_ipi(4).is_err());
        assert!(send_ipi(100).is_err());
    }
}

