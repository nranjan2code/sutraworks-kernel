//! Architecture-specific code for ARM64 (AArch64)

pub mod multicore;

#[cfg(not(feature = "test_mocks"))]
mod real {
    extern "C" {
        pub fn enable_interrupts();
        pub fn disable_interrupts() -> u64;
        pub fn enable_all_interrupts();
        pub fn disable_all_interrupts() -> u64;
        pub fn restore_interrupts(state: u64);
        pub fn get_exception_level() -> u64;
        pub fn get_core_id() -> u64;
        pub fn memory_barrier();
        pub fn data_sync_barrier();
        pub fn instruction_barrier();
        pub fn full_barrier();
        pub fn send_event();
        pub fn wait_for_event();
        pub fn wait_for_interrupt();

        pub fn wake_core(core: u64, entry: extern "C" fn());
        pub fn halt_core() -> !;

        pub fn read_timer() -> u64;
        pub fn read_timer_freq() -> u64;

        #[allow(dead_code)]
        pub fn switch_to(prev: *mut u8, next: *const u8);
    }
}

#[cfg(feature = "test_mocks")]
mod mocks {
    use core::sync::atomic::{AtomicU64, Ordering};
    static MOCK_TIMER: AtomicU64 = AtomicU64::new(1_000_000); // Start at 1s

    #[no_mangle]
    pub unsafe extern "C" fn read_timer() -> u64 { 
        MOCK_TIMER.fetch_add(100, Ordering::Relaxed)
    }

    #[no_mangle]
    pub unsafe extern "C" fn read_timer_freq() -> u64 { 1_000_000 } // 1 MHz for simple math

    #[no_mangle]
    pub unsafe extern "C" fn wait_for_interrupt() {}

    #[no_mangle]
    pub unsafe extern "C" fn disable_interrupts() -> u64 { 0 }

    #[no_mangle]
    pub unsafe extern "C" fn restore_interrupts(_state: u64) {}

    #[no_mangle]
    pub unsafe extern "C" fn get_core_id() -> u64 { 0 }

    #[no_mangle]
    pub unsafe extern "C" fn enable_interrupts() {}

    #[no_mangle]
    pub unsafe extern "C" fn enable_all_interrupts() {}

    #[no_mangle]
    pub unsafe extern "C" fn disable_all_interrupts() -> u64 { 0 }

    #[no_mangle]
    pub unsafe extern "C" fn get_exception_level() -> u64 { 1 }

    #[no_mangle]
    pub unsafe extern "C" fn memory_barrier() {}

    #[no_mangle]
    pub unsafe extern "C" fn data_sync_barrier() {}

    #[no_mangle]
    pub unsafe extern "C" fn instruction_barrier() {}

    #[no_mangle]
    pub unsafe extern "C" fn full_barrier() {}

    #[no_mangle]
    pub unsafe extern "C" fn send_event() {}

    #[no_mangle]
    pub unsafe extern "C" fn wait_for_event() {}

    #[no_mangle]
    pub unsafe extern "C" fn wake_core(_core: u64, _entry: extern "C" fn()) {}

    #[no_mangle]
    pub unsafe extern "C" fn halt_core() -> ! { loop {} }

    #[no_mangle]
    pub unsafe extern "C" fn switch_to(_prev: *mut u8, _next: *mut u8) {}
}

#[cfg(not(feature = "test_mocks"))]
pub use real::*;

#[cfg(feature = "test_mocks")]
pub use mocks::*;

// ═══════════════════════════════════════════════════════════════════════════════
// SAFE WRAPPERS
// ═══════════════════════════════════════════════════════════════════════════════

/// Get the current exception level (1-3)
#[inline]
pub fn exception_level() -> u64 {
    unsafe { get_exception_level() }
}

/// Get the current core ID (0-3)
#[inline]
pub fn core_id() -> u64 {
    unsafe { get_core_id() }
}

/// Barrier that ensures all memory accesses complete
#[inline]
pub fn dmb() {
    unsafe { memory_barrier() }
}

/// Data synchronization barrier
#[inline]
pub fn dsb() {
    unsafe { data_sync_barrier() }
}

/// Instruction synchronization barrier
#[inline]
pub fn isb() {
    unsafe { instruction_barrier() }
}

/// Full barrier (DSB + ISB)
#[inline]
pub fn barrier() {
    unsafe { full_barrier() }
}

/// Signal an event to wake other cores
#[inline]
pub fn sev() {
    unsafe { send_event() }
}

/// Wait for an event
#[inline]
pub fn wfe() {
    unsafe { wait_for_event() }
}

/// Wait for interrupt (low power)
#[inline]
pub fn wfi() {
    unsafe { wait_for_interrupt() }
}

/// Halt this core forever
#[inline]
pub fn halt() -> ! {
    unsafe { halt_core() }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTERRUPT CONTROL
// ═══════════════════════════════════════════════════════════════════════════════

/// Enable IRQ interrupts
#[inline]
pub fn irq_enable() {
    unsafe { enable_interrupts() }
}

/// Disable IRQ interrupts, return previous state
#[inline]
pub fn irq_disable() -> u64 {
    unsafe { disable_interrupts() }
}

/// Restore interrupt state
#[inline]
pub fn irq_restore(state: u64) {
    unsafe { restore_interrupts(state) }
}

/// Guard that disables interrupts and restores on drop
pub struct InterruptGuard {
    state: u64,
}

impl InterruptGuard {
    pub fn new() -> Self {
        InterruptGuard {
            state: irq_disable(),
        }
    }
}

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        irq_restore(self.state);
    }
}

/// Execute a closure with interrupts disabled
#[inline]
pub fn without_interrupts<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let _guard = InterruptGuard::new();
    f()
}

// ═══════════════════════════════════════════════════════════════════════════════
// MULTICORE
// ═══════════════════════════════════════════════════════════════════════════════

/// Wake a secondary core and start it at the given entry point
pub fn start_core(core: usize, entry: extern "C" fn()) {
    if core > 0 && core < 4 {
        unsafe { wake_core(core as u64, entry) }
    }
}

/// Spin lock for multicore synchronization
pub struct SpinLock<T: ?Sized> {
    lock: core::sync::atomic::AtomicBool,
    data: core::cell::UnsafeCell<T>,
}

unsafe impl<T: ?Sized + Send> Sync for SpinLock<T> {}
unsafe impl<T: ?Sized + Send> Send for SpinLock<T> {}

impl<T> SpinLock<T> {
    pub const fn new(data: T) -> Self {
        SpinLock {
            lock: core::sync::atomic::AtomicBool::new(false),
            data: core::cell::UnsafeCell::new(data),
        }
    }
}

impl<T: ?Sized> SpinLock<T> {
    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        use core::sync::atomic::Ordering;
        
        // Disable interrupts to prevent deadlock if an ISR tries to take the same lock
        let saved_int_state = irq_disable();
        
        while self.lock.compare_exchange_weak(
            false, true, 
            Ordering::Acquire, 
            Ordering::Relaxed
        ).is_err() {
            while self.lock.load(Ordering::Relaxed) {
                core::hint::spin_loop();
            }
        }
        SpinLockGuard { 
            lock: self,
            saved_int_state,
        }
    }
    
    pub fn unlock(&self) {
        use core::sync::atomic::Ordering;
        self.lock.store(false, Ordering::Release);
    }
    
    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        use core::sync::atomic::Ordering;
        
        let saved_int_state = irq_disable();
        
        if self.lock.compare_exchange(
            false, true,
            Ordering::Acquire,
            Ordering::Relaxed
        ).is_ok() {
            Some(SpinLockGuard { 
                lock: self,
                saved_int_state,
            })
        } else {
            // Restore interrupts if we failed to get the lock
            irq_restore(saved_int_state);
            None
        }
    }
}

/// RAII guard for SpinLock
pub struct SpinLockGuard<'a, T: ?Sized> {
    lock: &'a SpinLock<T>,
    saved_int_state: u64,
}

impl<'a, T: ?Sized> core::ops::Deref for SpinLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> core::ops::DerefMut for SpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T: ?Sized> Drop for SpinLockGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.unlock();
        irq_restore(self.saved_int_state);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// REGISTER ACCESS
// ═══════════════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════════════
// REGISTER ACCESS
// ═══════════════════════════════════════════════════════════════════════════════

/// Read a 32-bit value from a memory-mapped register
#[inline]
#[cfg(not(feature = "test_mocks"))]
pub unsafe fn read32(addr: usize) -> u32 {
    core::ptr::read_volatile(addr as *const u32)
}

#[inline]
#[cfg(feature = "test_mocks")]
pub unsafe fn read32(_addr: usize) -> u32 { 0 }

/// Write a 32-bit value to a memory-mapped register
#[inline]
#[cfg(not(feature = "test_mocks"))]
pub unsafe fn write32(addr: usize, value: u32) {
    core::ptr::write_volatile(addr as *mut u32, value);
}

#[inline]
#[cfg(feature = "test_mocks")]
pub unsafe fn write32(_addr: usize, _value: u32) {}

/// Read a 64-bit value from a memory-mapped register
#[inline]
#[cfg(not(feature = "test_mocks"))]
pub unsafe fn read64(addr: usize) -> u64 {
    core::ptr::read_volatile(addr as *const u64)
}

#[inline]
#[cfg(feature = "test_mocks")]
pub unsafe fn read64(_addr: usize) -> u64 { 0 }

/// Write a 64-bit value to a memory-mapped register
#[inline]
#[cfg(not(feature = "test_mocks"))]
pub unsafe fn write64(addr: usize, value: u64) {
    core::ptr::write_volatile(addr as *mut u64, value);
}

#[inline]
#[cfg(feature = "test_mocks")]
pub unsafe fn write64(_addr: usize, _value: u64) {}

/// Modify a 32-bit register: clear bits in mask, then set bits in value
#[inline]
pub unsafe fn modify32(addr: usize, mask: u32, value: u32) {
    let old = read32(addr);
    write32(addr, (old & !mask) | (value & mask));
}

// ═══════════════════════════════════════════════════════════════════════════════
// DELAY
// ═══════════════════════════════════════════════════════════════════════════════

/// Busy-wait delay for a number of cycles
#[inline]
pub fn delay_cycles(n: u32) {
    for _ in 0..n {
        core::hint::spin_loop();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// VIRTUAL MEMORY (VMSA)
// ═══════════════════════════════════════════════════════════════════════════════

/// Set Translation Control Register (TCR_EL1)
#[inline]
#[cfg(not(feature = "test_mocks"))]
pub unsafe fn set_tcr(value: u64) {
    core::arch::asm!("msr tcr_el1, {}", in(reg) value, options(nostack));
}

#[inline]
#[cfg(feature = "test_mocks")]
pub unsafe fn set_tcr(_value: u64) {}

/// Set Memory Attribute Indirection Register (MAIR_EL1)
#[inline]
#[cfg(not(feature = "test_mocks"))]
pub unsafe fn set_mair(value: u64) {
    core::arch::asm!("msr mair_el1, {}", in(reg) value, options(nostack));
}

#[inline]
#[cfg(feature = "test_mocks")]
pub unsafe fn set_mair(_value: u64) {}

/// Set Translation Table Base Register 0 (TTBR0_EL1)
#[inline]
#[cfg(not(feature = "test_mocks"))]
pub unsafe fn set_ttbr0(value: u64) {
    core::arch::asm!("msr ttbr0_el1, {}", in(reg) value, options(nostack));
}

#[inline]
#[cfg(feature = "test_mocks")]
pub unsafe fn set_ttbr0(_value: u64) {}

/// Set Translation Table Base Register 1 (TTBR1_EL1)
#[inline]
#[cfg(not(feature = "test_mocks"))]
pub unsafe fn set_ttbr1(value: u64) {
    core::arch::asm!("msr ttbr1_el1, {}", in(reg) value, options(nostack));
}

#[inline]
#[cfg(feature = "test_mocks")]
pub unsafe fn set_ttbr1(_value: u64) {}

/// Get System Control Register (SCTLR_EL1)
#[inline]
#[cfg(not(feature = "test_mocks"))]
pub unsafe fn get_sctlr() -> u64 {
    let value: u64;
    core::arch::asm!("mrs {}, sctlr_el1", out(reg) value, options(nostack));
    value
}

#[inline]
#[cfg(feature = "test_mocks")]
pub unsafe fn get_sctlr() -> u64 { 0 }

/// Set System Control Register (SCTLR_EL1)
#[inline]
#[cfg(not(feature = "test_mocks"))]
pub unsafe fn set_sctlr(value: u64) {
    core::arch::asm!("msr sctlr_el1, {}", in(reg) value, options(nostack));
}

#[inline]
#[cfg(feature = "test_mocks")]
pub unsafe fn set_sctlr(_value: u64) {}

/// Invalidate TLB (all)
#[inline]
#[cfg(not(feature = "test_mocks"))]
pub unsafe fn tlb_invalidate_all() {
    core::arch::asm!("tlbi vmalle1is", options(nostack));
    dsb();
    isb();
}

#[inline]
#[cfg(feature = "test_mocks")]
pub unsafe fn tlb_invalidate_all() {}

// ═══════════════════════════════════════════════════════════════════════════════
// CONTEXT SWITCHING
// ═══════════════════════════════════════════════════════════════════════════════

core::arch::global_asm!(
    ".global switch_to",
    "switch_to:",
    // x0 = prev_ctx pointer
    // x1 = next_ctx pointer

    // Save callee-saved registers to prev_ctx
    "stp x19, x20, [x0, #0]",
    "stp x21, x22, [x0, #16]",
    "stp x23, x24, [x0, #32]",
    "stp x25, x26, [x0, #48]",
    "stp x27, x28, [x0, #64]",
    "stp x29, x30, [x0, #80]",  // x29=FP, x30=LR
    
    // Save Stack Pointer
    "mov x9, sp",
    "str x9, [x0, #96]",

    // Save TTBR0 (User/Process Page Table)
    "mrs x9, ttbr0_el1",
    "str x9, [x0, #104]",

    // -----------------------------------------------------------------------

    // Restore callee-saved registers from next_ctx
    "ldp x19, x20, [x1, #0]",
    "ldp x21, x22, [x1, #16]",
    "ldp x23, x24, [x1, #32]",
    "ldp x25, x26, [x1, #48]",
    "ldp x27, x28, [x1, #64]",
    "ldp x29, x30, [x1, #80]",

    // Restore Stack Pointer
    "ldr x9, [x1, #96]",
    "mov sp, x9",

    // Restore TTBR0
    "ldr x9, [x1, #104]",
    "msr ttbr0_el1, x9",
    "tlbi vmalle1",
    "dsb nsh",
    "isb",

    // Return to the address in LR (x30)
    "ret"
);

extern "C" {
    pub fn switch_to(prev: *mut u8, next: *const u8);
    pub fn jump_to_userspace(entry: u64, stack: u64, arg: u64) -> !;
}

core::arch::global_asm!(
    ".global jump_to_userspace",
    "jump_to_userspace:",
    // x0 = entry point
    // x1 = stack pointer
    // x2 = argument

    // Mask all interrupts (DAIF) during transition
    "msr daifset, #0xf",

    // Set SPSR_EL1 to EL0t (0b0000)
    // M[3:0] = 0000 (EL0t)
    "mov x3, #0",
    "msr spsr_el1, x3",

    // Set ELR_EL1 to entry point
    "msr elr_el1, x0",

    // Set SP_EL0 to stack pointer
    "msr sp_el0, x1",

    // Move argument to x0 (where the user function expects it)
    "mov x0, x2",

    // Clear other registers to avoid leaking kernel data
    "mov x1, #0",
    "mov x2, #0",
    "mov x3, #0",
    "mov x4, #0",
    "mov x5, #0",
    "mov x6, #0",
    "mov x7, #0",
    "mov x8, #0",
    "mov x9, #0",
    "mov x10, #0",
    "mov x11, #0",
    "mov x12, #0",
    "mov x13, #0",
    "mov x14, #0",
    "mov x15, #0",
    "mov x16, #0",
    "mov x17, #0",
    "mov x18, #0",
    "mov x19, #0",
    "mov x20, #0",
    "mov x21, #0",
    "mov x22, #0",
    "mov x23, #0",
    "mov x24, #0",
    "mov x25, #0",
    "mov x26, #0",
    "mov x27, #0",
    "mov x28, #0",
    "mov x29, #0",
    "mov x30, #0",

    // Unmask interrupts (DAIF) in SPSR so they are enabled in EL0
    // We want IRQ (bit 7) = 0 (enabled)
    // SPSR was set to 0 above, which means all interrupts enabled in EL0.
    // That is correct.

    // Return to EL0
    "eret"
);

core::arch::global_asm!(
    ".global restore_exception_frame",
    "restore_exception_frame:",
    // x0 = frame pointer
    // x1 = sp_el0
    
    "msr sp_el0, x1",
    "mov sp, x0",
    
    // Restore SPSR, ELR
    "ldr x2, [sp, #264]", // spsr
    "msr spsr_el1, x2",
    "ldr x2, [sp, #256]", // elr
    "msr elr_el1, x2",
    
    // Restore registers
    "ldp x0, x1, [sp, #0]",
    "ldp x2, x3, [sp, #16]",
    "ldp x4, x5, [sp, #32]",
    "ldp x6, x7, [sp, #48]",
    "ldp x8, x9, [sp, #64]",
    "ldp x10, x11, [sp, #80]",
    "ldp x12, x13, [sp, #96]",
    "ldp x14, x15, [sp, #112]",
    "ldp x16, x17, [sp, #128]",
    "ldp x18, x19, [sp, #144]",
    "ldp x20, x21, [sp, #160]",
    "ldp x22, x23, [sp, #176]",
    "ldp x24, x25, [sp, #192]",
    "ldp x26, x27, [sp, #208]",
    "ldp x28, x29, [sp, #224]",
    "ldr x30, [sp, #240]",
    
    // Reclaim stack space (ExceptionFrame size = 280 bytes)
    "add sp, sp, #280",
    
    "eret"
);

extern "C" {
    pub fn restore_exception_frame(frame: *const u8, sp_el0: u64) -> !;
}

