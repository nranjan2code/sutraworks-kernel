//! System Timer Driver for Raspberry Pi 5
//!
//! Uses the ARM Generic Timer (architected timer) for accurate timing.

use crate::arch;
use core::sync::atomic::{AtomicU64, Ordering};

// ═══════════════════════════════════════════════════════════════════════════════
// TIMER STATE
// ═══════════════════════════════════════════════════════════════════════════════

static TIMER_FREQ: AtomicU64 = AtomicU64::new(0);
static BOOT_TIME: AtomicU64 = AtomicU64::new(0);

// ═══════════════════════════════════════════════════════════════════════════════
// ARM GENERIC TIMER
// ═══════════════════════════════════════════════════════════════════════════════

/// Initialize the timer subsystem
pub fn init() {
    // Read timer frequency from system register
    let freq = unsafe { arch::read_timer_freq() };
    TIMER_FREQ.store(freq, Ordering::SeqCst);
    
    // Record boot time
    let now = unsafe { arch::read_timer() };
    BOOT_TIME.store(now, Ordering::SeqCst);
}

/// Get timer frequency in Hz
pub fn frequency() -> u64 {
    let freq = TIMER_FREQ.load(Ordering::Relaxed);
    if freq == 0 {
        // Fallback: typical Pi frequency is 54MHz
        54_000_000
    } else {
        freq
    }
}

/// Read current timer count
#[inline]
pub fn ticks() -> u64 {
    unsafe { arch::read_timer() }
}

/// Get time since boot in microseconds
pub fn uptime_us() -> u64 {
    let now = ticks();
    let boot = BOOT_TIME.load(Ordering::Relaxed);
    let elapsed = now.wrapping_sub(boot);
    let freq = frequency();
    
    // Convert to microseconds: (elapsed * 1_000_000) / freq
    // Use u128 to avoid overflow
    ((elapsed as u128 * 1_000_000) / freq as u128) as u64
}

/// Get time since boot in milliseconds
pub fn uptime_ms() -> u64 {
    uptime_us() / 1000
}

/// Get time since boot in seconds
pub fn uptime_secs() -> u64 {
    uptime_us() / 1_000_000
}

/// Delay for a number of microseconds
pub fn delay_us(us: u64) {
    let freq = frequency();
    let ticks_needed = (us * freq) / 1_000_000;
    let start = ticks();
    
    while ticks().wrapping_sub(start) < ticks_needed {
        core::hint::spin_loop();
    }
}

/// Delay for a number of milliseconds
pub fn delay_ms(ms: u64) {
    delay_us(ms * 1000);
}

/// Delay for a number of seconds
pub fn delay_secs(secs: u64) {
    delay_us(secs * 1_000_000);
}

/// Convert ticks to microseconds
pub fn ticks_to_us(t: u64) -> u64 {
    let freq = frequency();
    ((t as u128 * 1_000_000) / freq as u128) as u64
}

/// Convert microseconds to ticks
pub fn us_to_ticks(us: u64) -> u64 {
    let freq = frequency();
    (us * freq) / 1_000_000
}

// ═══════════════════════════════════════════════════════════════════════════════
// DEADLINE
// ═══════════════════════════════════════════════════════════════════════════════

/// A point in time for deadline checking
#[derive(Clone, Copy)]
pub struct Deadline {
    target: u64,
}

impl Deadline {
    /// Create a deadline from now
    pub fn from_now_us(us: u64) -> Self {
        Deadline {
            target: ticks() + us_to_ticks(us),
        }
    }
    
    /// Create a deadline from now in milliseconds
    pub fn from_now_ms(ms: u64) -> Self {
        Self::from_now_us(ms * 1000)
    }
    
    /// Check if deadline has passed
    pub fn is_expired(&self) -> bool {
        ticks() >= self.target
    }
    
    /// Wait until deadline
    pub fn wait(&self) {
        while !self.is_expired() {
            core::hint::spin_loop();
        }
    }
    
    /// Remaining time in microseconds (0 if expired)
    pub fn remaining_us(&self) -> u64 {
        let now = ticks();
        if now >= self.target {
            0
        } else {
            ticks_to_us(self.target - now)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// STOPWATCH
// ═══════════════════════════════════════════════════════════════════════════════

/// A stopwatch for measuring elapsed time
#[derive(Clone, Copy)]
pub struct Stopwatch {
    start: u64,
}

impl Stopwatch {
    /// Start a new stopwatch
    pub fn start() -> Self {
        Stopwatch { start: ticks() }
    }
    
    /// Get elapsed time in microseconds
    pub fn elapsed_us(&self) -> u64 {
        ticks_to_us(ticks().wrapping_sub(self.start))
    }
    
    /// Get elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed_us() / 1000
    }
    
    /// Reset the stopwatch
    pub fn reset(&mut self) {
        self.start = ticks();
    }
    
    /// Lap: get elapsed and reset
    pub fn lap(&mut self) -> u64 {
        let elapsed = self.elapsed_us();
        self.reset();
        elapsed
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PERIODIC TIMER
// ═══════════════════════════════════════════════════════════════════════════════

/// A periodic timer that fires at regular intervals
pub struct Periodic {
    interval_ticks: u64,
    next_fire: u64,
}

impl Periodic {
    /// Create a new periodic timer with interval in microseconds
    pub fn new_us(interval_us: u64) -> Self {
        let interval_ticks = us_to_ticks(interval_us);
        Periodic {
            interval_ticks,
            next_fire: ticks() + interval_ticks,
        }
    }
    
    /// Create a new periodic timer with interval in milliseconds
    pub fn new_ms(interval_ms: u64) -> Self {
        Self::new_us(interval_ms * 1000)
    }
    
    /// Check if the timer has fired, reset if so
    pub fn check(&mut self) -> bool {
        if ticks() >= self.next_fire {
            self.next_fire += self.interval_ticks;
            true
        } else {
            false
        }
    }
    
    /// Wait for next fire
    pub fn wait(&mut self) {
        while !self.check() {
            core::hint::spin_loop();
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTERRUPT SUPPORT
// ═══════════════════════════════════════════════════════════════════════════════

/// Set the timer to trigger an interrupt after `us` microseconds
pub fn set_timer_interrupt(us: u64) {
    let ticks = us_to_ticks(us);
    unsafe {
        // Write to CNTV_TVAL_EL0 (Virtual Timer Value Register)
        core::arch::asm!("msr cntv_tval_el0, {}", in(reg) ticks, options(nostack));
        
        // Enable timer and unmask interrupt in CNTV_CTL_EL0
        core::arch::asm!("msr cntv_ctl_el0, {}", in(reg) 1u64, options(nostack));
    }
}

/// Disable the timer interrupt
pub fn disable_timer_interrupt() {
    unsafe {
        // Disable timer (Bit 0 = 0)
        core::arch::asm!("msr cntv_ctl_el0, {}", in(reg) 0u64, options(nostack));
    }
}
