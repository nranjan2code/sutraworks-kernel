//! Hardware drivers for Raspberry Pi 5

pub mod uart;
pub mod gpio;
pub mod timer;
pub mod interrupts;
pub mod mailbox;
pub mod framebuffer;
pub mod hailo;
pub mod ramdisk;
pub mod pcie;
pub mod rng;

// Re-export commonly used items
pub use uart::Uart;
pub use gpio::Pin;
pub use timer::{Deadline, Stopwatch, Periodic};
pub use interrupts::Gic;
pub use mailbox::PropertyBuffer;
pub use framebuffer::{Framebuffer, Color};
pub use rng::Rng;

// ═══════════════════════════════════════════════════════════════════════════════
// BCM2712 MEMORY MAP
// ═══════════════════════════════════════════════════════════════════════════════

/// Raspberry Pi 5 uses BCM2712 with a different peripheral base
#[cfg(not(test))]
pub const PERIPHERAL_BASE: usize = 0x1_0000_0000;

/// QEMU 'virt' machine uses 0x0900_0000 for UART
#[cfg(test)]
pub const PERIPHERAL_BASE: usize = 0x0900_0000;

/// Legacy peripheral base (for backward compatibility checks)
pub const LEGACY_PERIPHERAL_BASE: usize = 0xFE00_0000;

// Peripheral offsets from base
pub const GPIO_OFFSET: usize = 0x0020_0000;

#[cfg(not(test))]
pub const UART0_OFFSET: usize = 0x0020_1000;      // PL011 UART on Pi 5

#[cfg(test)]
pub const UART0_OFFSET: usize = 0x0000_0000;      // PL011 UART on virt machine

pub const AUX_OFFSET: usize = 0x0021_5000;        // Mini UART, SPI1, SPI2
pub const TIMER_OFFSET: usize = 0x0000_3000;      // System timer
pub const IRQ_OFFSET: usize = 0x0000_B200;        // Interrupt controller
pub const MBOX_OFFSET: usize = 0x0000_B880;       // Mailbox
pub const PM_OFFSET: usize = 0x0010_0000;         // Power management
pub const RNG_OFFSET: usize = 0x0010_4000;        // Hardware RNG

// Absolute addresses
pub const GPIO_BASE: usize = PERIPHERAL_BASE + GPIO_OFFSET;
pub const UART0_BASE: usize = PERIPHERAL_BASE + UART0_OFFSET;
pub const AUX_BASE: usize = PERIPHERAL_BASE + AUX_OFFSET;
pub const TIMER_BASE: usize = PERIPHERAL_BASE + TIMER_OFFSET;
pub const IRQ_BASE: usize = PERIPHERAL_BASE + IRQ_OFFSET;
pub const MBOX_BASE: usize = PERIPHERAL_BASE + MBOX_OFFSET;
pub const PM_BASE: usize = PERIPHERAL_BASE + PM_OFFSET;
pub const RNG_BASE: usize = PERIPHERAL_BASE + RNG_OFFSET;

// GIC-400 for Pi 5
pub const GIC_BASE: usize = 0x1_0004_0000;
pub const GICD_BASE: usize = GIC_BASE + 0x1000;   // Distributor
pub const GICC_BASE: usize = GIC_BASE + 0x2000;   // CPU interface
