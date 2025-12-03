//! Hailo-8 AI Accelerator Driver
//!
//! Controls the Hailo-8 NPU via PCIe.
//!
//! # Architecture
//! - **Connection**: PCIe Gen 3 x4
//! - **Access**: BAR0 (Control Registers), BAR2/4 (Doorbell/DMA)
//! - **Protocol**: Hailo Control Protocol (HCP)

use crate::kprintln;
use crate::arch::{self, SpinLock};
use crate::drivers::pcie::{self, PcieDevice, VENDOR_ID_HAILO};

// ═══════════════════════════════════════════════════════════════════════════════
// HAILO REGISTERS (Offsets in BAR0)
// ═══════════════════════════════════════════════════════════════════════════════

const HAILO_CONTROL: usize = 0x00;
const HAILO_STATUS: usize = 0x04;
const HAILO_IRQ_STATUS: usize = 0x10;
const HAILO_IRQ_MASK: usize = 0x14;

// ═══════════════════════════════════════════════════════════════════════════════
// DRIVER
// ═══════════════════════════════════════════════════════════════════════════════

pub struct HailoDriver {
    device: Option<PcieDevice>,
    bar0_addr: usize,
    bar0_size: usize,
    initialized: bool,
}

impl HailoDriver {
    pub const fn new() -> Self {
        Self {
            device: None,
            bar0_addr: 0,
            bar0_size: 0,
            initialized: false,
        }
    }

    /// Initialize the Hailo-8 driver
    pub fn init(&mut self) -> Result<(), &'static str> {
        kprintln!("[HAILO] Initializing AI Accelerator...");

        // 1. Find Hailo-8 on PCIe bus
        if let Some(dev) = pcie::find_device(VENDOR_ID_HAILO) {
            self.device = Some(dev);
            kprintln!("[HAILO] Found device at {:02x}:{:02x}.{}", dev.bus, dev.device, dev.function);
        } else {
            return Err("Hailo-8 not found on PCIe bus");
        }

        let dev = self.device.unwrap();

        // 2. Enable Bus Mastering
        pcie::enable_master(&dev);

        // 3. Get BAR0 (Control Registers)
        if let Some((addr, size)) = pcie::get_bar(&dev, 0) {
            self.bar0_addr = addr;
            self.bar0_size = size;
            kprintln!("[HAILO] BAR0 mapped at {:#010x} (Size: {} MB)", addr, size / (1024 * 1024));
        } else {
            return Err("Failed to read Hailo BAR0");
        }

        // 4. Reset Device
        self.reset()?;

        self.initialized = true;
        kprintln!("[HAILO] Initialization complete. NPU Ready.");
        Ok(())
    }

    /// Reset the Hailo device
    fn reset(&self) -> Result<(), &'static str> {
        // Write reset bit to control register
        // Note: Real reset sequence is more complex, this is a minimal example.
        self.write_reg(HAILO_CONTROL, 1);
        
        // Wait for reset to clear
        let mut timeout = 1000;
        while timeout > 0 {
            if (self.read_reg(HAILO_CONTROL) & 1) == 0 {
                return Ok(());
            }
            timeout -= 1;
            crate::drivers::timer::delay_us(100);
        }
        
        Err("Reset timeout")
    }

    /// Read a register from BAR0
    fn read_reg(&self, offset: usize) -> u32 {
        if !self.initialized { return 0; }
        unsafe { arch::read32(self.bar0_addr + offset) }
    }

    /// Write a register to BAR0
    fn write_reg(&self, offset: usize, value: u32) {
        if !self.initialized { return; }
        unsafe { arch::write32(self.bar0_addr + offset, value) }
    }
    
    /// Check if driver is active
    pub fn is_active(&self) -> bool {
        self.initialized
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL INSTANCE
// ═══════════════════════════════════════════════════════════════════════════════

pub static HAILO: SpinLock<HailoDriver> = SpinLock::new(HailoDriver::new());

/// Initialize Hailo driver
pub fn init() {
    let mut driver = HAILO.lock();
    if let Err(e) = driver.init() {
        kprintln!("[HAILO] Init failed: {}", e);
    }
}
