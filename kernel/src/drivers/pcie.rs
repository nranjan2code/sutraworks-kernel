//! PCIe Root Complex Driver for BCM2712 (Raspberry Pi 5)
//!
//! This driver handles the PCIe Root Complex, enabling communication with
//! downstream devices like the RP1 I/O Controller and the Hailo-8 AI Accelerator.
//!
//! # Architecture
//! - **Controller**: DesignWare PCIe Host Controller
//! - **Access Mechanism**: ECAM (Enhanced Configuration Access Mechanism)
//! - **Topology**:
//!   - Bus 0: Root Complex
//!   - Bus 1: Downstream Devices (RP1, Hailo)

use crate::kprintln;
use crate::arch::{self, SpinLock};

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════════

/// PCIe Configuration Space Base Address (ECAM)
/// On BCM2712, the PCIe0 (connected to RP1) ECAM is at 0x10_0000_0000 + offset.
/// The exact offset for ECAM on Pi 5 is 0x10_0010_0000 (BRCM_PCIE_ECAM).
pub const PCIE_ECAM_BASE: usize = 0x1_0000_0000 + 0x0010_0000;

/// PCIe Register Base (Control Registers)
pub const PCIE_REG_BASE: usize = 0x1_0000_0000 + 0x0011_0000;

// Vendor IDs
pub const VENDOR_ID_BROADCOM: u16 = 0x14E4;
pub const VENDOR_ID_RPI: u16 = 0x1DE4;      // Raspberry Pi (RP1)
pub const VENDOR_ID_HAILO: u16 = 0x1E60;    // Hailo AI

// Device IDs
pub const DEVICE_ID_RP1_C0: u16 = 0x0001;   // RP1 PCIe Bridge

// ═══════════════════════════════════════════════════════════════════════════════
// TYPES
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy)]
pub struct PcieDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
}

pub struct PcieController {
    ecam_base: usize,
    reg_base: usize,
    devices: [Option<PcieDevice>; 32], // Cache of found devices
    device_count: usize,
}

impl PcieController {
    pub const fn new() -> Self {
        Self {
            ecam_base: PCIE_ECAM_BASE,
            reg_base: PCIE_REG_BASE,
            devices: [None; 32],
            device_count: 0,
        }
    }

    /// Initialize the PCIe controller
    pub fn init(&mut self) -> Result<(), &'static str> {
        kprintln!("[PCIe] Initializing Root Complex...");

        // 1. Check Link Status
        // The bootloader (EEPROM/firmware) should have already brought up the link
        // to the RP1. We verify this by checking the DesignWare "Debug View" or similar.
        // For now, we'll try to read the Root Complex Vendor ID at 0:0:0.
        
        if let Some((vid, did)) = self.read_id(0, 0, 0) {
            kprintln!("[PCIe] Root Complex found: {:04x}:{:04x}", vid, did);
        } else {
            return Err("PCIe Root Complex not found (Link Down?)");
        }

        // 2. Enumerate Bus
        self.enumerate();

        Ok(())
    }

    /// Enumerate the PCIe bus
    pub fn enumerate(&mut self) {
        kprintln!("[PCIe] Enumerating bus...");
        self.device_count = 0;

        // Scan Bus 0 (Root Complex) and Bus 1 (Downstream)
        // A full scan would be recursive, but RPi5 topology is simple.
        for bus in 0..=1 {
            for dev in 0..32 {
                for func in 0..8 {
                    if let Some((vendor, device)) = self.read_id(bus, dev, func) {
                        kprintln!("[PCIe] Found {:04x}:{:04x} at {:02x}:{:02x}.{}", vendor, device, bus, dev, func);
                        
                        // Store in cache
                        if self.device_count < self.devices.len() {
                            self.devices[self.device_count] = Some(PcieDevice {
                                bus,
                                device: dev,
                                function: func,
                                vendor_id: vendor,
                                device_id: device,
                            });
                            self.device_count += 1;
                        }

                        // Identify known devices
                        if vendor == VENDOR_ID_RPI {
                            kprintln!("[PCIe] -> Raspberry Pi RP1 I/O Controller");
                        } else if vendor == VENDOR_ID_HAILO {
                            kprintln!("[PCIe] -> Hailo-8 AI Accelerator");
                        }
                    }
                }
            }
        }
        kprintln!("[PCIe] Enumeration complete. Found {} devices.", self.device_count);
    }

    /// Find a device by Vendor and Device ID
    pub fn find_device(&self, vendor_id: u16, device_id: u16) -> Option<PcieDevice> {
        for i in 0..self.device_count {
            if let Some(dev) = self.devices[i] {
                if dev.vendor_id == vendor_id && (device_id == 0 || dev.device_id == device_id) {
                    return Some(dev);
                }
            }
        }
        None
    }

    /// Find a device by Vendor ID only
    pub fn find_by_vendor(&self, vendor_id: u16) -> Option<PcieDevice> {
        self.find_device(vendor_id, 0)
    }

    /// Read 32-bit value from Configuration Space
    pub fn read_config(&self, bus: u8, dev: u8, func: u8, offset: usize) -> u32 {
        // ECAM Address:
        // Bits 20-27: Bus Number
        // Bits 15-19: Device Number
        // Bits 12-14: Function Number
        // Bits 00-11: Register Offset
        let addr = self.ecam_base 
            | ((bus as usize) << 20) 
            | ((dev as usize) << 15) 
            | ((func as usize) << 12) 
            | (offset & 0xFFF);
            
        unsafe { arch::read32(addr) }
    }

    /// Write 32-bit value to Configuration Space
    pub fn write_config(&self, bus: u8, dev: u8, func: u8, offset: usize, value: u32) {
        let addr = self.ecam_base 
            | ((bus as usize) << 20) 
            | ((dev as usize) << 15) 
            | ((func as usize) << 12) 
            | (offset & 0xFFF);
            
        unsafe { arch::write32(addr, value) }
    }

    /// Read Vendor and Device ID
    fn read_id(&self, bus: u8, dev: u8, func: u8) -> Option<(u16, u16)> {
        let val = self.read_config(bus, dev, func, 0x00);
        
        if val == 0xFFFFFFFF {
            return None;
        }
        
        let vendor = (val & 0xFFFF) as u16;
        let device = ((val >> 16) & 0xFFFF) as u16;
        Some((vendor, device))
    }

    /// Read Base Address Register (BAR)
    /// Returns the physical address and size
    pub fn read_bar(&self, device: &PcieDevice, bar_index: u8) -> Option<(usize, usize)> {
        if bar_index > 5 { return None; }
        
        let offset = 0x10 + (bar_index as usize * 4);
        
        // 1. Read original value
        let original = self.read_config(device.bus, device.device, device.function, offset);
        
        // 2. Write all 1s to determine size
        self.write_config(device.bus, device.device, device.function, offset, 0xFFFFFFFF);
        let size_mask = self.read_config(device.bus, device.device, device.function, offset);
        
        // 3. Restore original value
        self.write_config(device.bus, device.device, device.function, offset, original);
        
        // Check if memory mapped (bit 0 = 0)
        if (original & 0x1) != 0 {
            // I/O space not supported
            return None;
        }
        
        // Calculate size
        // Mask out type bits (0-3)
        let size = !(size_mask & 0xFFFFFFF0) + 1;
        let addr = (original & 0xFFFFFFF0) as usize;
        
        // Handle 64-bit BARs
        let type_bits = (original >> 1) & 0x3;
        if type_bits == 0x2 {
            // 64-bit BAR
            let upper = self.read_config(device.bus, device.device, device.function, offset + 4);
            let full_addr = addr | ((upper as usize) << 32);
            Some((full_addr, size as usize))
        } else {
            Some((addr, size as usize))
        }
    }
    
    /// Enable Bus Mastering for a device
    pub fn enable_master(&self, device: &PcieDevice) {
        let cmd_offset = 0x04;
        let mut cmd = self.read_config(device.bus, device.device, device.function, cmd_offset);
        
        // Bit 2: Bus Master Enable
        // Bit 1: Memory Space Enable
        cmd |= 0x6; 
        
        self.write_config(device.bus, device.device, device.function, cmd_offset, cmd);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL CONTROLLER
// ═══════════════════════════════════════════════════════════════════════════════

pub static CONTROLLER: SpinLock<PcieController> = SpinLock::new(PcieController::new());

/// Initialize PCIe subsystem
pub fn init() {
    let mut controller = CONTROLLER.lock();
    if let Err(e) = controller.init() {
        kprintln!("[PCIe] Init failed: {}", e);
    }
}

/// Find a device by Vendor ID
pub fn find_device(vendor_id: u16) -> Option<PcieDevice> {
    CONTROLLER.lock().find_by_vendor(vendor_id)
}

/// Read a BAR from a device
pub fn get_bar(device: &PcieDevice, bar_index: u8) -> Option<(usize, usize)> {
    CONTROLLER.lock().read_bar(device, bar_index)
}

/// Enable bus mastering
pub fn enable_master(device: &PcieDevice) {
    CONTROLLER.lock().enable_master(device);
}


