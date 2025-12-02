//! Hardware Random Number Generator (TRNG) driver.
#![allow(dead_code)]
//!
//! Provides access to the BCM2712 hardware RNG for entropy.

use crate::arch;


// ═══════════════════════════════════════════════════════════════════════════════
// RNG REGISTERS
// ═══════════════════════════════════════════════════════════════════════════════

const RNG_CTRL: usize = 0x00;
const RNG_STATUS: usize = 0x04;
const RNG_DATA: usize = 0x08;
const RNG_INT_MASK: usize = 0x10;

// Control Register bits
const RNG_CTRL_ENABLE: u32 = 1 << 0;

// Status Register bits
const RNG_STATUS_WARM_CNT: u32 = 0xFF << 24; // Warm-up count

// ═══════════════════════════════════════════════════════════════════════════════
// RNG DRIVER
// ═══════════════════════════════════════════════════════════════════════════════

pub struct Rng {
    base: usize,
}

impl Rng {
    /// Create a new RNG instance
    pub const fn new(base: usize) -> Self {
        Rng { base }
    }

    /// Initialize the RNG
    pub fn init(&self) {
        unsafe {
            // Enable RNG
            let ctrl = arch::read32(self.base + RNG_CTRL);
            arch::write32(self.base + RNG_CTRL, ctrl | RNG_CTRL_ENABLE);
        }
    }

    /// Get a random 32-bit integer
    pub fn next_u32(&self) -> u32 {
        unsafe {
            // Wait for data (status register indicates availability, 
            // but usually we can just read on these chips as it fills FIFO)
            // For robustness, we could check status, but simple read works on BCM.
            // Let's check the count bits if possible, but for now blocking read.
            
            // Wait for some entropy (simplified)
            for _ in 0..100 { core::hint::spin_loop(); }
            
            arch::read32(self.base + RNG_DATA)
        }
    }

    /// Get a random 64-bit integer
    pub fn next_u64(&self) -> u64 {
        let low = self.next_u32() as u64;
        let high = self.next_u32() as u64;
        (high << 32) | low
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL INSTANCE
// ═══════════════════════════════════════════════════════════════════════════════

use super::RNG_BASE;
use crate::arch::SpinLock;

static RNG_DEV: SpinLock<Rng> = SpinLock::new(Rng::new(RNG_BASE));

/// Initialize RNG
pub fn init() {
    RNG_DEV.lock().init();
}

/// Get a random u64
pub fn next_u64() -> u64 {
    RNG_DEV.lock().next_u64()
}

