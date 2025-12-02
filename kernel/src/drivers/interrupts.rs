//! GIC-400 Interrupt Controller Driver for Raspberry Pi 5
//!
//! The Generic Interrupt Controller handles all hardware interrupts
//! from peripherals and routes them to CPU cores.

use crate::arch;
use core::sync::atomic::{AtomicBool, Ordering};

// ═══════════════════════════════════════════════════════════════════════════════
// GIC-400 REGISTER OFFSETS
// ═══════════════════════════════════════════════════════════════════════════════

// GIC Distributor (GICD) registers
const GICD_CTLR: usize = 0x000;      // Distributor Control Register
const GICD_TYPER: usize = 0x004;     // Interrupt Controller Type Register
const GICD_ISENABLER: usize = 0x100; // Interrupt Set-Enable Registers
const GICD_ICENABLER: usize = 0x180; // Interrupt Clear-Enable Registers
const GICD_ISPENDR: usize = 0x200;   // Interrupt Set-Pending Registers
const GICD_ICPENDR: usize = 0x280;   // Interrupt Clear-Pending Registers
const GICD_IPRIORITYR: usize = 0x400; // Interrupt Priority Registers
const GICD_ITARGETSR: usize = 0x800; // Interrupt Processor Targets
const GICD_ICFGR: usize = 0xC00;     // Interrupt Configuration Registers

// GIC CPU Interface (GICC) registers
const GICC_CTLR: usize = 0x000;      // CPU Interface Control Register
const GICC_PMR: usize = 0x004;       // Interrupt Priority Mask Register
const GICC_IAR: usize = 0x00C;       // Interrupt Acknowledge Register
const GICC_EOIR: usize = 0x010;      // End of Interrupt Register
const GICC_RPR: usize = 0x014;       // Running Priority Register

// ═══════════════════════════════════════════════════════════════════════════════
// INTERRUPT NUMBERS
// ═══════════════════════════════════════════════════════════════════════════════

// Software Generated Interrupts (SGI) 0-15
pub const SGI_BASE: u32 = 0;

// Private Peripheral Interrupts (PPI) 16-31
pub const PPI_BASE: u32 = 16;
pub const PPI_TIMER: u32 = 27;      // Virtual timer
pub const PPI_PHYS_TIMER: u32 = 30; // Physical timer

// Shared Peripheral Interrupts (SPI) 32+
pub const SPI_BASE: u32 = 32;
pub const IRQ_UART0: u32 = 153;     // UART0 interrupt
pub const IRQ_GPIO0: u32 = 113;     // GPIO bank 0
pub const IRQ_GPIO1: u32 = 114;     // GPIO bank 1
pub const IRQ_GPIO2: u32 = 115;     // GPIO bank 2
pub const IRQ_TIMER: u32 = 96;      // System timer
pub const IRQ_MAILBOX: u32 = 97;    // Mailbox

// Maximum interrupt ID we support
const MAX_IRQ: u32 = 256;

// ═══════════════════════════════════════════════════════════════════════════════
// INTERRUPT STATE
// ═══════════════════════════════════════════════════════════════════════════════

static GIC_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Interrupt handler function type
pub type IrqHandler = fn(irq: u32);

// Handler table (protected by SpinLock)
static IRQ_HANDLERS: crate::arch::SpinLock<[Option<IrqHandler>; MAX_IRQ as usize]> = 
    crate::arch::SpinLock::new([None; MAX_IRQ as usize]);

// ═══════════════════════════════════════════════════════════════════════════════
// GIC CONTROLLER
// ═══════════════════════════════════════════════════════════════════════════════

/// GIC-400 Interrupt Controller
pub struct Gic {
    gicd_base: usize,
    gicc_base: usize,
}

impl Gic {
    /// Create a new GIC instance
    /// 
    /// # Safety
    /// Caller must ensure the base addresses are correct for the hardware
    pub const unsafe fn new(gicd_base: usize, gicc_base: usize) -> Self {
        Gic { gicd_base, gicc_base }
    }
    
    // GICD register access
    fn gicd_read(&self, offset: usize) -> u32 {
        unsafe { arch::read32(self.gicd_base + offset) }
    }
    
    fn gicd_write(&self, offset: usize, value: u32) {
        unsafe { arch::write32(self.gicd_base + offset, value) }
    }
    
    // GICC register access
    fn gicc_read(&self, offset: usize) -> u32 {
        unsafe { arch::read32(self.gicc_base + offset) }
    }
    
    fn gicc_write(&self, offset: usize, value: u32) {
        unsafe { arch::write32(self.gicc_base + offset, value) }
    }
    
    /// Initialize the GIC
    pub fn init(&self) {
        // Disable distributor
        self.gicd_write(GICD_CTLR, 0);
        
        // Read number of interrupt lines
        let typer = self.gicd_read(GICD_TYPER);
        let num_irqs = ((typer & 0x1f) + 1) * 32;
        
        // Disable all interrupts
        let mut i = 0;
        while i < num_irqs {
            self.gicd_write(GICD_ICENABLER + (i / 32 * 4) as usize, 0xFFFFFFFF);
            i += 32;
        }
        
        // Set all interrupts to lowest priority (0xFF)
        i = 0;
        while i < num_irqs {
            self.gicd_write(GICD_IPRIORITYR + i as usize, 0xFFFFFFFF);
            i += 4;
        }
        
        // Target all SPIs to CPU 0
        i = 32;
        while i < num_irqs {
            self.gicd_write(GICD_ITARGETSR + i as usize, 0x01010101);
            i += 4;
        }
        
        // Configure all SPIs as level-triggered
        i = 32;
        while i < num_irqs {
            self.gicd_write(GICD_ICFGR + (i / 16 * 4) as usize, 0);
            i += 16;
        }
        
        // Clear all pending interrupts
        i = 0;
        while i < num_irqs {
            self.gicd_write(GICD_ICPENDR + (i / 32 * 4) as usize, 0xFFFFFFFF);
            i += 32;
        }
        
        // Enable distributor with group 1 interrupts
        self.gicd_write(GICD_CTLR, 0x03);
        
        // === CPU Interface ===
        
        // Set priority mask to allow all priorities
        self.gicc_write(GICC_PMR, 0xFF);
        
        // Enable CPU interface
        self.gicc_write(GICC_CTLR, 0x03);
        
        GIC_INITIALIZED.store(true, Ordering::SeqCst);
    }
    
    /// Enable an interrupt
    pub fn enable_irq(&self, irq: u32) {
        if irq >= MAX_IRQ {
            return;
        }
        
        let reg_index = (irq / 32) as usize;
        let bit = 1u32 << (irq % 32);
        
        self.gicd_write(GICD_ISENABLER + reg_index * 4, bit);
    }
    
    /// Disable an interrupt
    pub fn disable_irq(&self, irq: u32) {
        if irq >= MAX_IRQ {
            return;
        }
        
        let reg_index = (irq / 32) as usize;
        let bit = 1u32 << (irq % 32);
        
        self.gicd_write(GICD_ICENABLER + reg_index * 4, bit);
    }
    
    /// Set interrupt priority (0 = highest, 255 = lowest)
    pub fn set_priority(&self, irq: u32, priority: u8) {
        if irq >= MAX_IRQ {
            return;
        }
        
        let reg_offset = GICD_IPRIORITYR + (irq as usize);
        let shift = (irq % 4) * 8;
        
        let mut value = self.gicd_read(reg_offset & !0x3);
        value &= !(0xFF << shift);
        value |= (priority as u32) << shift;
        self.gicd_write(reg_offset & !0x3, value);
    }
    
    /// Acknowledge an interrupt (returns IRQ number)
    pub fn acknowledge(&self) -> u32 {
        self.gicc_read(GICC_IAR) & 0x3FF
    }
    
    /// Signal end of interrupt
    pub fn end_of_interrupt(&self, irq: u32) {
        self.gicc_write(GICC_EOIR, irq);
    }
    
    /// Check if an interrupt is pending
    pub fn is_pending(&self, irq: u32) -> bool {
        if irq >= MAX_IRQ {
            return false;
        }
        
        let reg_index = (irq / 32) as usize;
        let bit = 1u32 << (irq % 32);
        
        (self.gicd_read(GICD_ISPENDR + reg_index * 4) & bit) != 0
    }
    
    /// Get current running priority
    pub fn running_priority(&self) -> u8 {
        (self.gicc_read(GICC_RPR) & 0xFF) as u8
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL INTERRUPT MANAGEMENT
// ═══════════════════════════════════════════════════════════════════════════════

/// Register an interrupt handler
pub fn register_handler(irq: u32, handler: IrqHandler) {
    if irq < MAX_IRQ {
        IRQ_HANDLERS.lock()[irq as usize] = Some(handler);
    }
}

/// Unregister an interrupt handler
pub fn unregister_handler(irq: u32) {
    if irq < MAX_IRQ {
        IRQ_HANDLERS.lock()[irq as usize] = None;
    }
}

/// Dispatch an interrupt to its handler
pub fn dispatch(irq: u32) {
    if irq < MAX_IRQ {
        // We need to be careful here. The lock is reentrant-safe ONLY because
        // we disabled interrupts in SpinLock::lock().
        // However, dispatch() is called FROM an interrupt handler.
        // If we try to take the lock, and it's held by non-interrupt code, we spin.
        // But since SpinLock disables interrupts, non-interrupt code CANNOT be holding
        // the lock when an interrupt fires on the same core!
        // So this is safe from deadlock on a single core.
        // For multicore, we spin until the other core releases it.
        let handler = IRQ_HANDLERS.lock()[irq as usize];
        if let Some(h) = handler {
            h(irq);
        }
    }
}

/// Check if GIC is initialized
pub fn is_initialized() -> bool {
    GIC_INITIALIZED.load(Ordering::Relaxed)
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTERRUPT CONTROLLER SINGLETON
// ═══════════════════════════════════════════════════════════════════════════════

use crate::drivers::{GICD_BASE, GICC_BASE};

/// Get the global GIC instance
pub fn gic() -> Gic {
    unsafe { Gic::new(GICD_BASE, GICC_BASE) }
}

/// Initialize the interrupt controller
pub fn init() {
    gic().init();
}

/// Enable a specific interrupt
pub fn enable(irq: u32) {
    gic().enable_irq(irq);
}

/// Disable a specific interrupt
pub fn disable(irq: u32) {
    gic().disable_irq(irq);
}
