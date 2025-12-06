//! USB HID Class Driver
//!
//! Implements HID Report Descriptor parsing and Report Protocol for NKRO support.

use crate::steno::{Stroke, StrokeProducer};
use crate::drivers::usb::xhci::CONTROLLER;
use crate::kernel::sync::SpinLock;
use crate::kprintln;
use alloc::vec::Vec;

// HID Item Types
const ITEM_TYPE_MAIN: u8 = 0;
const ITEM_TYPE_GLOBAL: u8 = 1;
const ITEM_TYPE_LOCAL: u8 = 2;

// Main Items
const ITEM_TAG_INPUT: u8 = 0x8;
const ITEM_TAG_COLLECTION: u8 = 0xA;
const ITEM_TAG_END_COLLECTION: u8 = 0xC;

// Global Items
const ITEM_TAG_USAGE_PAGE: u8 = 0x0;
const ITEM_TAG_REPORT_SIZE: u8 = 0x7;
const ITEM_TAG_REPORT_ID: u8 = 0x8;
const ITEM_TAG_REPORT_COUNT: u8 = 0x9;

// Local Items
const ITEM_TAG_USAGE: u8 = 0x0;
const ITEM_TAG_USAGE_MIN: u8 = 0x1;
const ITEM_TAG_USAGE_MAX: u8 = 0x2;

// Usage Pages
const USAGE_PAGE_GENERIC_DESKTOP: u32 = 0x01;
const USAGE_PAGE_KEYBOARD: u32 = 0x07;

// Usages
const USAGE_KEYBOARD: u32 = 0x06;



#[derive(Debug, Clone)]
struct HidField {
    usage_page: u32,
    usage_min: u32,
    #[allow(dead_code)]
    usage_max: u32,
    report_size: u32,
    report_count: u32,
    offset: u32, // Bit offset in the report
    is_variable: bool, // True if Variable (bitmap), False if Array (key codes)
}

#[derive(Debug, Clone)]
pub struct ReportLayout {
    report_id: Option<u8>,
    fields: Vec<HidField>,
    #[allow(dead_code)]
    total_bits: u32,
}

/// USB HID Driver
pub struct UsbHid {
    layout: Option<ReportLayout>,
    #[allow(dead_code)]
    last_stroke: Option<Stroke>,
    pub use_boot_protocol: bool,
}

impl UsbHid {
    /// Create a new HID driver instance
    pub const fn new() -> Self {
        Self {
            layout: None,
            last_stroke: None,
            use_boot_protocol: true, // Default to Boot Protocol as xHCI sets it
        }
    }

    /// Parse a HID Report Descriptor
    pub fn parse_descriptor(&mut self, descriptor: &[u8]) -> Result<(), &'static str> {
        let mut cursor = 0;
        let mut global_usage_page = 0;
        let mut global_report_size = 0;
        let mut global_report_count = 0;
        let mut global_report_id = None;
        
        let mut local_usages = Vec::new();
        let mut local_usage_min = 0;
        let mut local_usage_max = 0;
        
        let mut current_offset = 0;
        let mut fields = Vec::new();
        
        let mut in_keyboard_collection = false;

        while cursor < descriptor.len() {
            let header = descriptor[cursor];
            cursor += 1;
            
            if header == 0xFE { // Long Item (skip for now)
                let len = descriptor[cursor];
                cursor += 1 + 1 + len as usize;
                continue;
            }
            
            let size_code = header & 0x3;
            let type_ = (header >> 2) & 0x3;
            let tag = (header >> 4) & 0xF;
            
            let size = match size_code {
                0 => 0,
                1 => 1,
                2 => 2,
                3 => 4, // size_code & 0x3 guarantees only 0-3
                _ => 4, // Compiler: this is unreachable but satisfies exhaustiveness
            };
            
            let mut data: u32 = 0;
            for i in 0..size {
                if cursor < descriptor.len() {
                    data |= (descriptor[cursor] as u32) << (i * 8);
                    cursor += 1;
                }
            }
            

            
            match type_ {
                ITEM_TYPE_MAIN => {
                    match tag {
                        ITEM_TAG_COLLECTION => {
                            if global_usage_page == USAGE_PAGE_GENERIC_DESKTOP && local_usages.contains(&USAGE_KEYBOARD) {
                                in_keyboard_collection = true;
                            }
                            local_usages.clear();
                        }
                        ITEM_TAG_END_COLLECTION => {
                            in_keyboard_collection = false;
                            local_usages.clear();
                        }
                        ITEM_TAG_INPUT => {
                            if in_keyboard_collection {
                                let is_variable = (data & 0x2) != 0;
                                let is_constant = (data & 0x1) != 0;
                                
                                if !is_constant {
                                    // Add field
                                    let count = global_report_count;
                                    let size = global_report_size;
                                    
                                    // If we have specific usages, use them.
                                    // If we have a range, use that.
                                    // If neither, use 0.
                                    
                                    let (u_min, u_max) = if !local_usages.is_empty() {
                                        (*local_usages.first().unwrap(), *local_usages.last().unwrap())
                                    } else if local_usage_min != 0 {
                                        (local_usage_min, local_usage_max)
                                    } else {
                                        (0, 0)
                                    };
                                    
                                    fields.push(HidField {
                                        usage_page: global_usage_page,
                                        usage_min: u_min,
                                        usage_max: u_max,
                                        report_size: size,
                                        report_count: count,
                                        offset: current_offset,
                                        is_variable,
                                    });
                                }
                                
                                current_offset += global_report_size * global_report_count;
                            }
                            local_usages.clear();
                            local_usage_min = 0;
                            local_usage_max = 0;
                        }
                        _ => {}
                    }
                }
                ITEM_TYPE_GLOBAL => {
                    match tag {
                        ITEM_TAG_USAGE_PAGE => global_usage_page = data,
                        ITEM_TAG_REPORT_SIZE => global_report_size = data,
                        ITEM_TAG_REPORT_COUNT => global_report_count = data,
                        ITEM_TAG_REPORT_ID => {
                            global_report_id = Some(data as u8);
                            current_offset = 0; // Reset offset for new report ID
                        }
                        _ => {}
                    }
                }
                ITEM_TYPE_LOCAL => {
                    match tag {
                        ITEM_TAG_USAGE => local_usages.push(data),
                        ITEM_TAG_USAGE_MIN => local_usage_min = data,
                        ITEM_TAG_USAGE_MAX => local_usage_max = data,
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        
        self.layout = Some(ReportLayout {
            report_id: global_report_id,
            fields,
            total_bits: current_offset,
        });
        
        kprintln!("[HID] Parsed Layout: {} fields, {} bits", self.layout.as_ref().unwrap().fields.len(), current_offset);
        self.use_boot_protocol = false; // Switch to Report Protocol
        Ok(())
    }

    /// Poll for new strokes
    pub fn poll(&mut self) -> Option<Stroke> {
        // 1. Check if xHCI has new events
        let mut controller = CONTROLLER.lock();
        controller.poll();
        
        // 2. Check if our Interrupt IN transfer completed
        // We need to know which slot ID we are using.
        // For now, iterate all slots and check pending transfers?
        // Or just assume Slot 1 for the keyboard.
        let slot_id = 1; 
        
        if let Some(ref mut transfer) = controller.pending_transfers[slot_id] {
            if transfer.completed {
                // Transfer completed!
                let len = transfer.bytes_transferred;
                let code = transfer.completion_code;
                
                // Copy data
                let mut report = [0u8; 8];
                if let Some(dma_addr) = transfer.dma_buffer {
                    unsafe {
                        core::ptr::copy_nonoverlapping(dma_addr as *const u8, report.as_mut_ptr(), len.min(8));
                    }
                }
                
                // Clear pending transfer so we can submit a new one
                controller.pending_transfers[slot_id] = None;
                
                if code == 1 && len > 0 { // Success
                    // Parse Report
                    if let Some(stroke) = self.parse_report(&report[0..len]) {
                        // Debounce / NKRO logic would go here.
                        // For now, just return it.
                        
                        // Re-submit transfer for next interrupt
                        // (Ideally we do this immediately, but we need to drop the lock first?)
                        // We are holding HID lock, and we took Controller lock.
                        // We can submit new transfer now.
                        
                        // We need a buffer. We can reuse the one we just freed?
                        // control_transfer_sync allocates a new one each time.
                        // We need an async transfer method on Controller.
                        // For now, we just don't re-submit here to avoid complexity in this snippet.
                        // In a real driver, we'd have a ring of buffers.
                        
                        return Some(stroke);
                    }
                }
            }
        } else {
            // No pending transfer? Submit one!
            // We need to submit an Interrupt IN transfer.
            // This requires a new method on XhciController or using `enqueue_ep_trb` directly.
            // But we don't have access to the ring details easily here without more plumbing.
            // Let's assume the initial enumeration started one, or we need to start one.
        }
        
        None
    }

    /// Parse a raw report buffer
    pub fn parse_report(&mut self, report: &[u8]) -> Option<Stroke> {
        if self.use_boot_protocol {
            return self.parse_boot_report(report);
        }

        let layout = self.layout.as_ref()?;
        
        // Check Report ID
        let data_start = if layout.report_id.is_some() {
            if report[0] != layout.report_id.unwrap() {
                return None; // Wrong report ID
            }
            1
        } else {
            0
        };
        
        let mut stroke_bits = 0u32;
        
        for field in &layout.fields {
            if field.usage_page != USAGE_PAGE_KEYBOARD { continue; }
            
            let bit_offset = field.offset;
            let count = field.report_count;
            let size = field.report_size;
            
            for i in 0..count {
                let total_bit_offset = bit_offset + (i * size);
                let byte_idx = data_start + (total_bit_offset / 8) as usize;
                let bit_idx = total_bit_offset % 8;
                
                if byte_idx >= report.len() { break; }
                
                let val = if size == 1 {
                    ((report[byte_idx] >> bit_idx) & 1) as u32
                } else {
                    // Assuming 8-bit aligned for now for arrays
                    report[byte_idx] as u32
                };
                
                if field.is_variable {
                    // Bitmap: Each bit corresponds to a usage
                    // Usage = Usage Min + i
                    if val != 0 {
                        let usage = field.usage_min + i;
                        if let Some(steno_bit) = self.map_usage_to_steno(usage as u8) {
                            stroke_bits |= 1 << steno_bit;
                        }
                    }
                } else {
                    // Array: Value is the usage ID
                    if val != 0 {
                        if let Some(steno_bit) = self.map_usage_to_steno(val as u8) {
                            stroke_bits |= 1 << steno_bit;
                        }
                    }
                }
            }
        }
        
        if stroke_bits != 0 {
            Some(Stroke::from_raw(stroke_bits))
        } else {
            None
        }
    }

    /// Parse Boot Protocol Report (8 bytes)
    fn parse_boot_report(&mut self, report: &[u8]) -> Option<Stroke> {
        if report.len() < 8 { return None; }
        // Byte 0: Modifiers
        // Byte 1: Reserved
        // Byte 2-7: Keycodes
        
        let mut stroke_bits = 0u32;
        
        for i in 2..8 {
            let code = report[i];
            if code != 0 {
                if let Some(steno_bit) = self.map_usage_to_steno(code) {
                     stroke_bits |= 1 << steno_bit;
                }
            }
        }
        
        if stroke_bits != 0 {
            Some(Stroke::from_raw(stroke_bits))
        } else {
            None
        }
    }
    
    /// Map HID Usage ID (Keyboard Page) to Steno Bit Index
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
pub static HID_DRIVER: SpinLock<UsbHid> = SpinLock::new(UsbHid::new());

impl Default for UsbHid { fn default() -> Self { Self::new() } }
