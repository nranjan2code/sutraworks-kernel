//! GPIO Driver for Raspberry Pi 5 (BCM2712)
//!
//! Controls the GPIO pins on the BCM2712.
#![allow(dead_code)]

use crate::arch;
use super::GPIO_BASE;

// ═══════════════════════════════════════════════════════════════════════════════
// GPIO REGISTERS (BCM2712)
// ═══════════════════════════════════════════════════════════════════════════════

// Function select registers (3 bits per pin, 10 pins per register)
const GPFSEL0: usize = 0x00;   // GPIO 0-9
const GPFSEL1: usize = 0x04;   // GPIO 10-19
const GPFSEL2: usize = 0x08;   // GPIO 20-29
const GPFSEL3: usize = 0x0C;   // GPIO 30-39
const GPFSEL4: usize = 0x10;   // GPIO 40-49
const GPFSEL5: usize = 0x14;   // GPIO 50-57

// Output set registers
const GPSET0: usize = 0x1C;    // GPIO 0-31
const GPSET1: usize = 0x20;    // GPIO 32-57

// Output clear registers
const GPCLR0: usize = 0x28;    // GPIO 0-31
const GPCLR1: usize = 0x2C;    // GPIO 32-57

// Pin level registers
const GPLEV0: usize = 0x34;    // GPIO 0-31
const GPLEV1: usize = 0x38;    // GPIO 32-57

// Event detect status
const GPEDS0: usize = 0x40;    // GPIO 0-31
const GPEDS1: usize = 0x44;    // GPIO 32-57

// Rising edge detect
const GPREN0: usize = 0x4C;
const GPREN1: usize = 0x50;

// Falling edge detect
const GPFEN0: usize = 0x58;
const GPFEN1: usize = 0x5C;

// High detect
const GPHEN0: usize = 0x64;
const GPHEN1: usize = 0x68;

// Low detect
const GPLEN0: usize = 0x70;
const GPLEN1: usize = 0x74;

// Async rising edge
const GPAREN0: usize = 0x7C;
const GPAREN1: usize = 0x80;

// Async falling edge
const GPAFEN0: usize = 0x88;
const GPAFEN1: usize = 0x8C;

// Pull-up/down (BCM2712 uses different registers)
const GPIO_PUP_PDN_CNTRL0: usize = 0xE4;
const GPIO_PUP_PDN_CNTRL1: usize = 0xE8;
const GPIO_PUP_PDN_CNTRL2: usize = 0xEC;
const GPIO_PUP_PDN_CNTRL3: usize = 0xF0;

// ═══════════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════════

const GPIO_MAX_PIN: u32 = 57;

// Pi 5 specific pins
const ACTIVITY_LED_PIN: u32 = 42;  // ACT LED on Pi 5
const POWER_LED_PIN: u32 = 43;     // PWR LED on Pi 5

// ═══════════════════════════════════════════════════════════════════════════════
// TYPES
// ═══════════════════════════════════════════════════════════════════════════════

/// GPIO function selection
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Function {
    Input = 0b000,
    Output = 0b001,
    Alt0 = 0b100,
    Alt1 = 0b101,
    Alt2 = 0b110,
    Alt3 = 0b111,
    Alt4 = 0b011,
    Alt5 = 0b010,
}

/// Pull-up/down configuration
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pull {
    None = 0b00,
    Up = 0b01,
    Down = 0b10,
}

/// Edge detection type
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edge {
    None,
    Rising,
    Falling,
    Both,
}

/// Pin level
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

impl From<Level> for bool {
    fn from(v: Level) -> Self {
        v == Level::High
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GPIO DRIVER
// ═══════════════════════════════════════════════════════════════════════════════

/// GPIO pin representation
pub struct Pin {
    num: u32,
}

impl Pin {
    /// Create a new pin instance
    pub const fn new(num: u32) -> Self {
        Pin { num }
    }
    
    /// Get pin number
    pub fn num(&self) -> u32 {
        self.num
    }
    
    /// Read a GPIO register
    #[inline]
    fn read(offset: usize) -> u32 {
        unsafe { arch::read32(GPIO_BASE + offset) }
    }
    
    /// Write a GPIO register
    #[inline]
    fn write(offset: usize, value: u32) {
        unsafe { arch::write32(GPIO_BASE + offset, value) }
    }
    
    /// Set the function of this pin
    pub fn set_function(&self, func: Function) {
        if self.num > GPIO_MAX_PIN {
            return;
        }
        
        let reg_offset = GPFSEL0 + ((self.num / 10) * 4) as usize;
        let shift = (self.num % 10) * 3;
        
        let mut value = Self::read(reg_offset);
        value &= !(0b111 << shift);
        value |= (func as u32) << shift;
        Self::write(reg_offset, value);
    }
    
    /// Get the current function of this pin
    pub fn get_function(&self) -> Function {
        if self.num > GPIO_MAX_PIN {
            return Function::Input;
        }
        
        let reg_offset = GPFSEL0 + ((self.num / 10) * 4) as usize;
        let shift = (self.num % 10) * 3;
        let value = (Self::read(reg_offset) >> shift) & 0b111;
        
        match value {
            0b000 => Function::Input,
            0b001 => Function::Output,
            0b100 => Function::Alt0,
            0b101 => Function::Alt1,
            0b110 => Function::Alt2,
            0b111 => Function::Alt3,
            0b011 => Function::Alt4,
            0b010 => Function::Alt5,
            _ => Function::Input,
        }
    }
    
    /// Set the pin high
    pub fn set_high(&self) {
        if self.num > GPIO_MAX_PIN {
            return;
        }
        
        let reg_offset = if self.num < 32 { GPSET0 } else { GPSET1 };
        let bit = self.num % 32;
        Self::write(reg_offset, 1 << bit);
    }
    
    /// Set the pin low
    pub fn set_low(&self) {
        if self.num > GPIO_MAX_PIN {
            return;
        }
        
        let reg_offset = if self.num < 32 { GPCLR0 } else { GPCLR1 };
        let bit = self.num % 32;
        Self::write(reg_offset, 1 << bit);
    }
    
    /// Set the pin to a level
    pub fn set(&self, level: Level) {
        match level {
            Level::High => self.set_high(),
            Level::Low => self.set_low(),
        }
    }
    
    /// Read the pin level
    pub fn read_level(&self) -> Level {
        if self.num > GPIO_MAX_PIN {
            return Level::Low;
        }
        
        let reg_offset = if self.num < 32 { GPLEV0 } else { GPLEV1 };
        let bit = self.num % 32;
        
        if Self::read(reg_offset) & (1 << bit) != 0 {
            Level::High
        } else {
            Level::Low
        }
    }
    
    /// Toggle the pin
    pub fn toggle(&self) {
        match self.read_level() {
            Level::High => self.set_low(),
            Level::Low => self.set_high(),
        }
    }
    
    /// Configure pull-up/down
    pub fn set_pull(&self, pull: Pull) {
        if self.num > GPIO_MAX_PIN {
            return;
        }
        
        // BCM2712 uses different pull configuration
        let reg_offset = GPIO_PUP_PDN_CNTRL0 + ((self.num / 16) * 4) as usize;
        let shift = (self.num % 16) * 2;
        
        let mut value = Self::read(reg_offset);
        value &= !(0b11 << shift);
        value |= (pull as u32) << shift;
        Self::write(reg_offset, value);
    }
    
    /// Configure edge detection
    pub fn set_edge_detect(&self, edge: Edge) {
        if self.num > GPIO_MAX_PIN {
            return;
        }
        
        let (ren_offset, fen_offset) = if self.num < 32 {
            (GPREN0, GPFEN0)
        } else {
            (GPREN1, GPFEN1)
        };
        let bit = self.num % 32;
        let mask = 1u32 << bit;
        
        let mut ren = Self::read(ren_offset);
        let mut fen = Self::read(fen_offset);
        
        match edge {
            Edge::None => {
                ren &= !mask;
                fen &= !mask;
            }
            Edge::Rising => {
                ren |= mask;
                fen &= !mask;
            }
            Edge::Falling => {
                ren &= !mask;
                fen |= mask;
            }
            Edge::Both => {
                ren |= mask;
                fen |= mask;
            }
        }
        
        Self::write(ren_offset, ren);
        Self::write(fen_offset, fen);
    }
    
    /// Check and clear event
    pub fn check_event(&self) -> bool {
        if self.num > GPIO_MAX_PIN {
            return false;
        }
        
        let reg_offset = if self.num < 32 { GPEDS0 } else { GPEDS1 };
        let bit = self.num % 32;
        let mask = 1u32 << bit;
        
        let status = Self::read(reg_offset);
        if status & mask != 0 {
            // Clear by writing 1
            Self::write(reg_offset, mask);
            true
        } else {
            false
        }
    }
    
    /// Configure as output
    pub fn into_output(self) -> OutputPin {
        self.set_function(Function::Output);
        OutputPin { pin: self }
    }
    
    /// Configure as input
    pub fn into_input(self) -> InputPin {
        self.set_function(Function::Input);
        InputPin { pin: self }
    }
}

/// An output pin
pub struct OutputPin {
    pin: Pin,
}

impl OutputPin {
    pub fn set_high(&self) {
        self.pin.set_high();
    }
    
    pub fn set_low(&self) {
        self.pin.set_low();
    }
    
    pub fn set(&self, level: Level) {
        self.pin.set(level);
    }
    
    pub fn toggle(&self) {
        self.pin.toggle();
    }
    
    pub fn num(&self) -> u32 {
        self.pin.num
    }
}

/// An input pin
pub struct InputPin {
    pin: Pin,
}

impl InputPin {
    pub fn read(&self) -> Level {
        self.pin.read_level()
    }
    
    pub fn is_high(&self) -> bool {
        self.pin.read_level() == Level::High
    }
    
    pub fn is_low(&self) -> bool {
        self.pin.read_level() == Level::Low
    }
    
    pub fn set_pull(&self, pull: Pull) {
        self.pin.set_pull(pull);
    }
    
    pub fn num(&self) -> u32 {
        self.pin.num
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Initialize GPIO subsystem
pub fn init() {
    // Configure activity LED as output
    let led = Pin::new(ACTIVITY_LED_PIN);
    led.set_function(Function::Output);
    led.set_low();
}

/// Control the activity LED
pub fn activity_led(on: bool) {
    let led = Pin::new(ACTIVITY_LED_PIN);
    if on {
        led.set_high();
    } else {
        led.set_low();
    }
}

/// Blink the activity LED
pub fn blink_led(times: u32, delay_cycles: u32) {
    let led = Pin::new(ACTIVITY_LED_PIN);
    led.set_function(Function::Output);
    
    for _ in 0..times {
        led.set_high();
        arch::delay_cycles(delay_cycles);
        led.set_low();
        arch::delay_cycles(delay_cycles);
    }
}

/// Set a pin as output with value
pub fn set_output(pin: u32, high: bool) {
    let p = Pin::new(pin);
    p.set_function(Function::Output);
    if high {
        p.set_high();
    } else {
        p.set_low();
    }
}

/// Read a pin as input
pub fn read_input(pin: u32) -> bool {
    let p = Pin::new(pin);
    p.set_function(Function::Input);
    p.read_level() == Level::High
}

/// Toggle a pin
pub fn toggle(pin: u32) {
    let p = Pin::new(pin);
    p.toggle();
}
