//! PCIe Root Complex Driver for BCM2712
//!
//! This driver handles the initialization and enumeration of the PCIe bus.
//! It uses the ECAM (Enhanced Configuration Access Mechanism) to access
//! configuration space.

use crate::kprintln;

/// Base address for PCIe Configuration Space (ECAM).
/// NOTE: This is a placeholder. On BCM2712, this is likely at a high address
/// (e.g. 0x10_0000_0000 or similar). For now, we define it but don't access it
/// blindly to avoid SError until verified.
pub const PCIE_ECAM_BASE: usize = 0x10_0000_0000; 

pub struct PcieController {
    base_addr: usize,
}

impl PcieController {
    pub const fn new() -> Self {
        Self {
            base_addr: PCIE_ECAM_BASE,
        }
    }

    /// Initialize the PCIe controller.
    pub fn init(&self) -> Result<(), &'static str> {
        kprintln!("[PCIe] Initializing Root Complex...");
        
        // In a real driver, we would:
        // 1. Enable power domains
        // 2. De-assert resets
        // 3. Wait for Link Up
        
        // For now, we assume the bootloader (config.txt) has done the heavy lifting.
        // We just check if we can read a safe register or assume success for the stub.
        
        kprintln!("[PCIe] Link Up (Assumed)");
        Ok(())
    }

    /// Enumerate devices on the bus.
    pub fn enumerate(&self) {
        kprintln!("[PCIe] Scanning Bus 0...");
        
        // Simplified enumeration: Scan 32 devices, 8 functions
        for dev in 0..32 {
            for func in 0..8 {
                if let Some((vendor, device)) = self.read_id(0, dev, func) {
                    kprintln!("[PCIe] Found {:04x}:{:04x} at 00:{:02x}.{}", vendor, device, dev, func);
                    
                    if vendor == 0x1e60 && device == 0x2864 {
                        kprintln!("[PCIe] !!! FOUND HAILO-8 AI ACCELERATOR !!!");
                    }
                }
            }
        }
    }

    /// Find a device by Vendor and Device ID, returning its ECAM address
    pub fn find_device(&self, target_vendor: u16, target_device: u16) -> Option<(u8, u8, u8)> {
        for dev in 0..32 {
            for func in 0..8 {
                if let Some((vendor, device)) = self.read_id(0, dev, func) {
                    if vendor == target_vendor && device == target_device {
                        return Some((0, dev, func));
                    }
                }
            }
        }
        None
    }

    /// Read 32-bit value from config space
    pub fn read_config_32(&self, bus: u8, dev: u8, func: u8, offset: usize) -> u32 {
        let ecam_offset = ((bus as usize) << 20) | ((dev as usize) << 15) | ((func as usize) << 12) | (offset & 0xFFF);
        let addr = self.base_addr + ecam_offset;
        unsafe { crate::arch::read32(addr) }
    }

    /// Read BAR0
    pub fn read_bar0(&self, bus: u8, dev: u8, func: u8) -> usize {
        let bar0 = self.read_config_32(bus, dev, func, 0x10);
        // Mask out flag bits (assuming 64-bit BAR or 32-bit memory BAR)
        // For simplicity, assume 32-bit memory BAR for now
        (bar0 & 0xFFFF_FFF0) as usize
    }

    /// Read Vendor and Device ID from config space.
    fn read_id(&self, bus: u8, dev: u8, func: u8) -> Option<(u16, u16)> {
        // ECAM address calculation:
        // Address = Base + (Bus << 20) + (Dev << 15) + (Func << 12)
        let offset = ((bus as usize) << 20) | ((dev as usize) << 15) | ((func as usize) << 12);
        let addr = self.base_addr + offset;
        
        // SAFETY: We are accessing memory-mapped I/O. 
        // We assume base_addr is correct (it's a constant for now).
        // If the region is not mapped, this will cause a Data Abort.
        // In a real kernel, we'd ensure this is mapped in page tables first.
        let val = unsafe { crate::arch::read32(addr) };
        
        // 0xFFFFFFFF means no device present
        if val == 0xFFFFFFFF {
            return None;
        }
        
        let vendor = (val & 0xFFFF) as u16;
        let device = ((val >> 16) & 0xFFFF) as u16;
        
        Some((vendor, device))
    }
}

/// Global PCIe Controller
use crate::arch::SpinLock;
pub static CONTROLLER: SpinLock<PcieController> = SpinLock::new(PcieController::new());

pub fn init() {
    let controller = CONTROLLER.lock();
    if let Err(e) = controller.init() {
        kprintln!("[PCIe] Init failed: {}", e);
        return;
    }
    controller.enumerate();
}

