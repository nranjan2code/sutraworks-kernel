//! Hardware drivers for Raspberry Pi 5

pub mod uart;
pub mod gpio;
pub mod timer;
pub mod interrupts;
pub mod mailbox;
pub mod framebuffer;
pub mod console;
pub mod hailo;
pub mod hailo_tensor;
pub mod ramdisk;
pub mod pcie;
pub mod rng;
pub mod usb;
pub mod sd;
pub mod rp1;
pub mod virtio;
pub mod virtio_blk;
pub mod ethernet;

// Re-export commonly used items
pub use uart::Uart;
pub use gpio::Pin;
pub use timer::{Deadline, Stopwatch, Periodic};
pub use interrupts::Gic;
pub use mailbox::PropertyBuffer;
pub use framebuffer::{Framebuffer, Color};
pub use console::Console;
pub use rng::Rng;

// ═══════════════════════════════════════════════════════════════════════════════
// DYNAMIC HARDWARE CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════════

use crate::dtb::{self, MachineType};

pub fn uart_base() -> usize {
    match dtb::machine_type() {
        MachineType::RaspberryPi5 => 0x1_0000_0000 + 0x0020_1000, // Pi 5 PL011
        _ => 0x0900_0000, // QemuVirt or Unknown
    }
}

pub fn gicd_base() -> usize {
    match dtb::machine_type() {
        MachineType::RaspberryPi5 => 0x1_0004_0000 + 0x1000,
        _ => 0x0800_0000, // QemuVirt or Unknown (Default to QEMU)
    }
}

pub fn gicc_base() -> usize {
    match dtb::machine_type() {
        MachineType::RaspberryPi5 => 0x1_0004_0000 + 0x2000,
        _ => 0x0801_0000, // QemuVirt or Unknown (Default to QEMU)
    }
}

pub fn rng_base() -> usize {
    // Pi 5 RNG
    0x1_0000_0000 + 0x0010_4000
}

pub fn pcie_ecam_base() -> usize {
    match dtb::machine_type() {
        MachineType::QemuVirt => 0x3f00_0000, // Highmem ECAM on virt
        _ => 0x1_0000_0000, // Pi 5 PCIe
    }
}

// Legacy constants (deprecated, but kept if needed for specific Pi 5 drivers)
pub const PERIPHERAL_BASE_PI5: usize = 0x1_0000_0000;
pub const GPIO_BASE: usize = PERIPHERAL_BASE_PI5 + 0x0020_0000;
pub const MBOX_BASE: usize = PERIPHERAL_BASE_PI5 + 0x0000_B880;

// ═══════════════════════════════════════════════════════════════════════════════
// BCM2712 MEMORY MAP
// ═══════════════════════════════════════════════════════════════════════════════

/// Legacy peripheral base (for backward compatibility checks)
pub const LEGACY_PERIPHERAL_BASE: usize = 0xFE00_0000;

// Peripheral offsets from base
pub const _PADS_BANK0: usize = 0x00f0_0000;
pub const GPIO_OFFSET: usize = 0x0020_0000;

pub const AUX_OFFSET: usize = 0x0021_5000;        // Mini UART, SPI1, SPI2
pub const _HAILO_STATUS: usize = 0x04;
const _HAILO_IRQ_STATUS: usize = 0x10;
const _HAILO_IRQ_MASK: usize = 0x14;
pub const TIMER_OFFSET: usize = 0x0000_3000;      // System timer
pub const IRQ_OFFSET: usize = 0x0000_B200;        // Interrupt controller
pub const PM_OFFSET: usize = 0x0010_0000;         // Power management
pub const RNG_OFFSET: usize = 0x0010_4000;        // Hardware RNG

// Absolute addresses
pub const AUX_BASE: usize = PERIPHERAL_BASE_PI5 + AUX_OFFSET;
pub const TIMER_BASE: usize = PERIPHERAL_BASE_PI5 + TIMER_OFFSET;
pub const IRQ_BASE: usize = PERIPHERAL_BASE_PI5 + IRQ_OFFSET;
pub const PM_BASE: usize = PERIPHERAL_BASE_PI5 + PM_OFFSET;
