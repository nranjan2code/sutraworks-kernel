//! RP1 I/O Controller Driver
//!
//! The RP1 is the southbridge of the Raspberry Pi 5, connected via PCIe.
//! It handles GPIO, Ethernet, USB, SPI, I2C, etc.
//!
//! # Architecture
//! - **Connection**: PCIe Gen 2 x4
//! - **Access**: Memory Mapped I/O via PCIe BAR1
//! - **Peripherals**: Mapped at offsets within BAR1

use crate::kprintln;
use crate::arch::{self, SpinLock};
use crate::drivers::pcie::{self, PcieDevice, VENDOR_ID_RPI};

// ═══════════════════════════════════════════════════════════════════════════════
// OFFSETS WITHIN RP1 BAR1
// ═══════════════════════════════════════════════════════════════════════════════

pub const RP1_GPIO_BASE: usize = 0x00d0_0000;
pub const RP1_PADS_BANK0: usize = 0x00f0_0000;
pub const RP1_PADS_BANK1: usize = 0x00f0_4000;
pub const RP1_PADS_BANK2: usize = 0x00f0_8000;

// ═══════════════════════════════════════════════════════════════════════════════
// DRIVER
// ═══════════════════════════════════════════════════════════════════════════════

pub struct Rp1Driver {
    device: Option<PcieDevice>,
    bar1_addr: usize,
    bar1_size: usize,
    initialized: bool,
}

impl Rp1Driver {
    pub const fn new() -> Self {
        Self {
            device: None,
            bar1_addr: 0,
            bar1_size: 0,
            initialized: false,
        }
    }

    /// Initialize the RP1 driver
    pub fn init(&mut self) -> Result<(), &'static str> {
        kprintln!("[RP1] Initializing I/O Controller...");

        // 1. Find RP1 on PCIe bus
        // We look for Vendor ID 0x1DE4 (Raspberry Pi)
        if let Some(dev) = pcie::find_device(VENDOR_ID_RPI) {
            self.device = Some(dev);
            kprintln!("[RP1] Found device at {:02x}:{:02x}.{}", dev.bus, dev.device, dev.function);
        } else {
            return Err("RP1 not found on PCIe bus");
        }

        let dev = self.device.unwrap();

        // 2. Enable Bus Mastering
        pcie::enable_master(&dev);

        // 3. Get BAR1 (Peripheral Aperture)
        // BAR1 is usually the main 4GB window, but we only need access to the registers.
        if let Some((addr, size)) = pcie::get_bar(&dev, 1) {
            self.bar1_addr = addr;
            self.bar1_size = size;
            kprintln!("[RP1] BAR1 mapped at {:#010x} (Size: {} MB)", addr, size / (1024 * 1024));
        } else {
            return Err("Failed to read RP1 BAR1");
        }

        self.initialized = true;
        Ok(())
    }

    /// Read a register from the RP1 address space
    pub fn read(&self, offset: usize) -> u32 {
        if !self.initialized { return 0; }
        unsafe { arch::read32(self.bar1_addr + offset) }
    }

    /// Write a register to the RP1 address space
    pub fn write(&self, offset: usize, value: u32) {
        if !self.initialized { return; }
        unsafe { arch::write32(self.bar1_addr + offset, value) }
    }
    
    /// Check if driver is initialized
    pub fn is_active(&self) -> bool {
        self.initialized
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL INSTANCE
// ═══════════════════════════════════════════════════════════════════════════════

pub static RP1: SpinLock<Rp1Driver> = SpinLock::new(Rp1Driver::new());

/// Initialize RP1 driver
pub fn init() {
    let mut driver = RP1.lock();
    if let Err(e) = driver.init() {
        kprintln!("[RP1] Init failed: {}", e);
    }
}

/// Read from RP1 register
pub fn read_reg(offset: usize) -> u32 {
    RP1.lock().read(offset)
}

/// Write to RP1 register
pub fn write_reg(offset: usize, value: u32) {
    RP1.lock().write(offset, value)
}

impl Default for Rp1Driver { fn default() -> Self { Self::new() } }
