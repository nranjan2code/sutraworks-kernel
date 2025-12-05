use core::sync::atomic::{AtomicU64, Ordering};

/// Global performance counters for the kernel.
pub struct PerformanceCounters {
    /// Number of context switches performed.
    pub context_switches: AtomicU64,
    /// Number of system calls handled.
    pub syscalls: AtomicU64,
    /// Number of page faults handled.
    pub page_faults: AtomicU64,
    /// Number of interrupts handled.
    pub interrupts: AtomicU64,
    /// Total cycles spent in system calls (for latency measurement).
    pub total_syscall_cycles: AtomicU64,
}

impl PerformanceCounters {
    pub const fn new() -> Self {
        Self {
            context_switches: AtomicU64::new(0),
            syscalls: AtomicU64::new(0),
            page_faults: AtomicU64::new(0),
            interrupts: AtomicU64::new(0),
            total_syscall_cycles: AtomicU64::new(0),
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&self) {
        self.context_switches.store(0, Ordering::Relaxed);
        self.syscalls.store(0, Ordering::Relaxed);
        self.page_faults.store(0, Ordering::Relaxed);
        self.interrupts.store(0, Ordering::Relaxed);
        self.total_syscall_cycles.store(0, Ordering::Relaxed);
    }
}

/// Global singleton for performance counters.
pub static PROFILER: PerformanceCounters = PerformanceCounters::new();

/// Read the current cycle count (Time-Stamp Counter).
///
/// On AArch64, this reads the virtual counter register `CNTVCT_EL0`.
#[inline(always)]
pub fn rdtsc() -> u64 {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        let cnt: u64;
        core::arch::asm!("mrs {}, cntvct_el0", out(reg) cnt);
        cnt
    }

    #[cfg(not(target_arch = "aarch64"))]
    0 // Fallback for other architectures (or tests on host)
}
