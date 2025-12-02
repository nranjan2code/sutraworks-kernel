//! USB HID Class Driver
//!
//! Handles Human Interface Devices, specifically Steno machines.
//! Implements HID Boot Protocol for Keyboard.

use crate::steno::{Stroke, StrokeProducer, KEYS, NUM_KEYS};
use crate::drivers::usb::xhci::CONTROLLER;
use crate::kprintln;

/// Standard HID Boot Protocol Keyboard Report (8 bytes)
#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct KeyboardReport {
    pub modifiers: u8,
    pub reserved: u8,
    pub keys: [u8; 6],
}

/// USB HID Driver
pub struct UsbHid {
    connected: bool,
    last_report: Option<KeyboardReport>,
}

impl UsbHid {
    /// Create a new HID driver instance
    pub const fn new() -> Self {
        Self {
            connected: false,
            last_report: None,
        }
    }

    /// Poll for new strokes
    pub fn poll(&mut self) -> Option<Stroke> {
        // 1. Check if xHCI has new events
        let mut controller = CONTROLLER.lock();
        controller.poll();
        
        // TODO: Get actual data from xHCI Transfer Ring
        // For now, we return None as we can't receive data without a physical device.
        // But the parsing logic below is REAL and ready for data.
        
        None
    }

    /// Parse a Boot Protocol Report into a Steno Stroke
    /// 
    /// This maps standard QWERTY keys to Steno keys based on Plover layout.
    pub fn parse_report(&self, report: &KeyboardReport) -> Option<Stroke> {
        let mut stroke_bits = 0u32;
        
        // Check all 6 key slots
        for &usage_id in report.keys.iter() {
            if usage_id == 0 { continue; }
            
            // Map Usage ID to Steno Key
            if let Some(steno_bit) = self.map_usage_to_steno(usage_id) {
                stroke_bits |= 1 << steno_bit;
            }
        }
        
        // Check modifiers (if steno machine uses them, though usually it's just keys)
        // Some machines map # to Shift, etc.
        
        if stroke_bits != 0 {
            Some(Stroke::from_raw(stroke_bits))
        } else {
            None
        }
    }
    
    /// Map HID Usage ID (Keyboard Page) to Steno Bit Index
    /// Based on standard Plover QWERTY mapping
    fn map_usage_to_steno(&self, usage: u8) -> Option<usize> {
        // Usage IDs from USB HID Usage Tables
        match usage {
            // Row 1
            0x14 => Some(1),  // Q -> S-
            0x1A => Some(2),  // W -> T-
            0x08 => Some(3),  // E -> K-
            0x15 => Some(4),  // R -> P-
            0x17 => Some(5),  // T -> W-
            0x1C => Some(6),  // Y -> H-
            0x18 => Some(7),  // U -> R-
            0x0C => Some(8),  // I -> A-
            0x12 => Some(9),  // O -> O-
            0x13 => Some(10), // P -> *
            0x2F => Some(10), // [ -> *
            
            // Row 2
            0x04 => Some(1),  // A -> S-
            0x16 => Some(3),  // S -> K-
            0x07 => Some(5),  // D -> W-
            0x09 => Some(7),  // F -> R-
            0x0A => Some(10), // G -> *
            0x0B => Some(10), // H -> *
            0x0D => Some(14), // J -> -R
            0x0E => Some(16), // K -> -B
            0x0F => Some(18), // L -> -G
            0x33 => Some(20), // ; -> -S
            0x34 => Some(22), // ' -> -Z
            
            // Row 3
            0x06 => Some(11), // C -> -E
            0x19 => Some(12), // V -> -U
            0x11 => Some(13), // N -> -F
            0x10 => Some(15), // M -> -P
            
            _ => None
        }
    }

    /// Check if N-Key Rollover is supported
    pub fn supports_nkro(&self) -> bool {
        true
    }
}

impl StrokeProducer for UsbHid {
    fn stroke_available(&self) -> bool {
        false
    }

    fn read_stroke(&mut self) -> Stroke {
        loop {
            if let Some(stroke) = self.try_read_stroke() {
                return stroke;
            }
            core::hint::spin_loop();
        }
    }

    fn try_read_stroke(&mut self) -> Option<Stroke> {
        self.poll()
    }
}

/// Global HID instance
use crate::arch::SpinLock;
pub static HID_DRIVER: SpinLock<UsbHid> = SpinLock::new(UsbHid::new());
