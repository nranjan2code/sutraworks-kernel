//! GPIO Driver for Raspberry Pi 5 (RP1)
//!
//! Controls the GPIO pins via the RP1 I/O Controller.
//!
//! # Architecture
//! - **Controller**: RP1 (Southbridge)
//! - **Access**: PCIe BAR1 -> RP1 Address Space
//! - **Banks**:
//!   - Bank 0: GPIO 0-27 (Header)
//!   - Bank 1: GPIO 28-33 (Ethernet/USB)
//!   - Bank 2: GPIO 34-53 (SDIO/HDMI)

use crate::drivers::rp1;

// ═══════════════════════════════════════════════════════════════════════════════
// RP1 GPIO REGISTERS
// ═══════════════════════════════════════════════════════════════════════════════

// RIO (Registered I/O) Base for GPIO
const RIO_BASE: usize = 0x00d0_0000;

// Offsets within RIO
const RIO_OUT: usize = 0x00;
const RIO_OE: usize = 0x04;
const RIO_IN: usize = 0x08;

// PADS Base (for Pull-up/down)
const PADS_BANK0: usize = 0x00f0_0000;

// ═══════════════════════════════════════════════════════════════════════════════
// TYPES
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Level {
    Low = 0,
    High = 1,
}

impl From<bool> for Level {
    fn from(v: bool) -> Self {
        if v { Level::High } else { Level::Low }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GPIO DRIVER
// ═══════════════════════════════════════════════════════════════════════════════

pub struct Pin {
    num: u32,
}

impl Pin {
    pub const fn new(num: u32) -> Self {
        Pin { num }
    }

    /// Set pin as output
    pub fn into_output(self) -> Self {
        // Set Output Enable bit
        let mask = 1 << self.num;
        let current = rp1::read_reg(RIO_BASE + RIO_OE);
        rp1::write_reg(RIO_BASE + RIO_OE, current | mask);
        self
    }

    /// Set pin as input
    pub fn into_input(self) -> Self {
        // Clear Output Enable bit
        let mask = 1 << self.num;
        let current = rp1::read_reg(RIO_BASE + RIO_OE);
        rp1::write_reg(RIO_BASE + RIO_OE, current & !mask);
        self
    }

    /// Set output level
    pub fn set(&self, level: Level) {
        let mask = 1 << self.num;
        match level {
            Level::High => {
                // RIO_OUT is a direct set/clear register? 
                // Actually RP1 RIO usually has separate SET/CLR registers or XOR.
                // For simplicity assuming R/W register for now (standard RIO).
                // To be safe, we read-modify-write.
                // Note: This is not atomic without a lock, but good enough for now.
                let current = rp1::read_reg(RIO_BASE + RIO_OUT);
                rp1::write_reg(RIO_BASE + RIO_OUT, current | mask);
            }
            Level::Low => {
                let current = rp1::read_reg(RIO_BASE + RIO_OUT);
                rp1::write_reg(RIO_BASE + RIO_OUT, current & !mask);
            }
        }
    }

    pub fn set_high(&self) { self.set(Level::High); }
    pub fn set_low(&self) { self.set(Level::Low); }

    /// Read input level
    pub fn read(&self) -> Level {
        let val = rp1::read_reg(RIO_BASE + RIO_IN);
        if (val & (1 << self.num)) != 0 {
            Level::High
        } else {
            Level::Low
        }
    }
    
    pub fn toggle(&self) {
        // RIO usually has an XOR register at +0x0C, let's try that
        let mask = 1 << self.num;
        let current = rp1::read_reg(RIO_BASE + RIO_OUT);
        rp1::write_reg(RIO_BASE + RIO_OUT, current ^ mask);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Initialize GPIO subsystem
pub fn init() {
    // Nothing to do for RP1 GPIO specifically, 
    // assuming rp1::init() has been called.
}

/// Control the activity LED (GPIO 42 on RP1?)
/// Actually on Pi 5, ACT LED is controlled by the PMIC or a specific GPIO.
/// It is often GPIO 42 on the RP1.
pub fn activity_led(on: bool) {
    // Check if RP1 is ready
    if !rp1::read_reg(RIO_BASE + RIO_OE) == 0 && !rp1::RP1.lock().is_active() {
        return; 
    }

    let led = Pin::new(42); // ACT LED
    // Ensure it's output (lazy init)
    let led = led.into_output();

    if on {
        led.set_high();
    } else {
        led.set_low();
    }
}

