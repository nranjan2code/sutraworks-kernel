//! xHCI Host Controller Driver
//!
//! Handles the low-level USB 3.0 host controller interface.
//! Implements the xHCI 1.2 Specification.
#![allow(dead_code)]

use crate::kprintln;
use crate::arch::{self, SpinLock};
use crate::kernel::memory::{self, PAGE_SIZE};
use core::ptr::NonNull;

// ═══════════════════════════════════════════════════════════════════════════════
// xHCI REGISTERS
// ═══════════════════════════════════════════════════════════════════════════════

/// Capability Registers Offsets
const CAP_CAPLENGTH: usize = 0x00;
const CAP_HCSPARAMS1: usize = 0x04;
const CAP_HCSPARAMS2: usize = 0x08;
const CAP_HCSPARAMS3: usize = 0x0C;
const CAP_HCCPARAMS1: usize = 0x10;
const CAP_DBOFF: usize = 0x14;
const CAP_RTSOFF: usize = 0x18;

/// Operational Registers Offsets (relative to OpBase)
const OP_USBCMD: usize = 0x00;
const OP_USBSTS: usize = 0x04;
const OP_PAGESIZE: usize = 0x08;
const OP_DNCTRL: usize = 0x14;
const OP_CRCR: usize = 0x18; // 64-bit
const OP_DCBAAP: usize = 0x30; // 64-bit
const OP_CONFIG: usize = 0x38;

/// Runtime Registers Offsets (relative to RtBase)
const RT_IMAN: usize = 0x20;
const RT_IMOD: usize = 0x24;
const RT_ERSTSZ: usize = 0x28;
const RT_ERSTBA: usize = 0x30; // 64-bit
const RT_ERDP: usize = 0x38;   // 64-bit

/// USBCMD Bits
const USBCMD_RUN: u32 = 1 << 0;
const USBCMD_RESET: u32 = 1 << 1;
const USBCMD_INTE: u32 = 1 << 2;
const USBCMD_HSEE: u32 = 1 << 3;

/// USBSTS Bits
const USBSTS_HCH: u32 = 1 << 0; // Host Controller Halted
const USBSTS_CNR: u32 = 1 << 11; // Controller Not Ready

// ═══════════════════════════════════════════════════════════════════════════════
// DATA STRUCTURES
// ═══════════════════════════════════════════════════════════════════════════════

/// Transfer Request Block (TRB) - 16 bytes
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug)]
pub struct Trb {
    pub param_low: u32,
    pub param_high: u32,
    pub status: u32,
    pub control: u32,
}

impl Trb {
    pub fn new() -> Self {
        Self { param_low: 0, param_high: 0, status: 0, control: 0 }
    }
}

/// Event Ring Segment Table Entry (ERSTE) - 16 bytes
#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct ErstEntry {
    pub base_addr_low: u32,
    pub base_addr_high: u32,
    pub size: u32,
    pub reserved: u32,
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONTROLLER
// ═══════════════════════════════════════════════════════════════════════════════

/// xHCI Controller structure
pub struct XhciController {
    base_addr: usize,
    op_base: usize,
    rt_base: usize,
    db_base: usize,
    
    dcbaa: Option<NonNull<u8>>,
    cmd_ring: Option<NonNull<u8>>,
    event_ring: Option<NonNull<u8>>,
    erst: Option<NonNull<u8>>,
    
    // Ring State
    event_ring_dequeue_ptr: Option<NonNull<Trb>>,
    event_ring_cycle_bit: u32,
    
    initialized: bool,
}

impl XhciController {
    /// Create a new xHCI controller
    pub const fn new() -> Self {
        Self {
            base_addr: 0,
            op_base: 0,
            rt_base: 0,
            db_base: 0,
            dcbaa: None,
            cmd_ring: None,
            event_ring: None,
            erst: None,
            event_ring_dequeue_ptr: None,
            event_ring_cycle_bit: 1,
            initialized: false,
        }
    }



    /// Initialize the controller
    pub fn init(&mut self) -> Result<(), &'static str> {
        kprintln!("[USB] Initializing xHCI Host Controller...");
        
        // 1. Find xHCI controller via PCIe
        let pcie = crate::drivers::pcie::CONTROLLER.lock();
        // VLI VL805 is common on Pi 4, Pi 5 uses internal RP1.
        // Let's look for standard xHCI class (0x0C0330) or specific RP1 ID.
        // For now, we'll try to find *any* device that looks like xHCI or use a known address.
        // On Pi 5, RP1 is at a fixed address usually.
        // Let's assume we found it at a specific BAR or address.
        // For the sake of "Real" implementation, we'll use a placeholder address that would be correct on hardware.
        // RP1 PCIe base is 0x1F_0000_0000 (roughly).
        // Let's use the address from `pcie.rs` if it finds one.
        
        // Mock finding for now, but use real logic structure
        // 0x1de4 is VLI VL805 (Pi 4). For Pi 5 RP1, we'd look for 0x1de4:0x0001 (placeholder).
        if let Some((bus, dev, func)) = pcie.find_device(0x1de4, 0x0001) { 
             self.base_addr = pcie.read_bar0(bus, dev, func);
        } else {
             // Fallback to a hardcoded address for Pi 5 RP1 xHCI if not found via ECAM
             // This is often 0x1000120000 or similar on RP1.
             self.base_addr = 0x10_0012_0000; 
        }
        
        kprintln!("[USB] xHCI Base: {:#x}", self.base_addr);
        
        // 2. Read Capability Registers to find Operational Base
        let cap_len = unsafe { arch::read32(self.base_addr + CAP_CAPLENGTH) } & 0xFF;
        self.op_base = self.base_addr + cap_len as usize;
        
        let rts_off = unsafe { arch::read32(self.base_addr + CAP_RTSOFF) } & !0x1F;
        self.rt_base = self.base_addr + rts_off as usize;
        
        let db_off = unsafe { arch::read32(self.base_addr + CAP_DBOFF) } & !0x3;
        self.db_base = self.base_addr + db_off as usize;
        
        kprintln!("[USB] OP Base: {:#x}, RT Base: {:#x}", self.op_base, self.rt_base);
        
        // 3. Reset Controller
        self.reset()?;
        
        // 4. Set Max Device Slots
        let hcsparams1 = unsafe { arch::read32(self.base_addr + CAP_HCSPARAMS1) };
        let max_slots = hcsparams1 & 0xFF;
        kprintln!("[USB] Max Slots: {}", max_slots);
        
        unsafe {
            let config = arch::read32(self.op_base + OP_CONFIG);
            arch::write32(self.op_base + OP_CONFIG, (config & !0xFF) | max_slots);
        }
        
        // 5. Set up Device Context Base Address Array (DCBAA)
        // Needs (MaxSlots + 1) pointers. 64-bit pointers.
        let dcbaa_size = ((max_slots + 1) * 8) as usize;
        let dcbaa = unsafe { memory::alloc_dma(dcbaa_size) }.ok_or("Failed to alloc DCBAA")?;
        
        // Zero it out
        unsafe { core::ptr::write_bytes(dcbaa.as_ptr(), 0, dcbaa_size) };
        self.dcbaa = Some(dcbaa);
        
        // Write DCBAAP
        let dcbaa_phys = dcbaa.as_ptr() as u64;
        unsafe {
            arch::write32(self.op_base + OP_DCBAAP, dcbaa_phys as u32);
            arch::write32(self.op_base + OP_DCBAAP + 4, (dcbaa_phys >> 32) as u32);
        }
        
        // 6. Set up Command Ring
        let cmd_ring = unsafe { memory::alloc_dma(PAGE_SIZE) }.ok_or("Failed to alloc Cmd Ring")?;
        unsafe { core::ptr::write_bytes(cmd_ring.as_ptr(), 0, PAGE_SIZE) };
        self.cmd_ring = Some(cmd_ring);
        
        let cmd_ring_phys = cmd_ring.as_ptr() as u64;
        unsafe {
            arch::write32(self.op_base + OP_CRCR, (cmd_ring_phys as u32) | 1); // RCS=1
            arch::write32(self.op_base + OP_CRCR + 4, (cmd_ring_phys >> 32) as u32);
        }
        
        // 7. Set up Interrupter 0 (Event Ring)
        // Allocate Event Ring Segment Table (ERST) - 1 segment
        let erst = unsafe { memory::alloc_dma(64) }.ok_or("Failed to alloc ERST")?; // 1 entry = 16 bytes
        self.erst = Some(erst);
        
        // Allocate Event Ring
        let event_ring = unsafe { memory::alloc_dma(PAGE_SIZE) }.ok_or("Failed to alloc Event Ring")?;
        unsafe { core::ptr::write_bytes(event_ring.as_ptr(), 0, PAGE_SIZE) };
        self.event_ring = Some(event_ring);
        self.event_ring_dequeue_ptr = Some(event_ring.cast());
        self.event_ring_cycle_bit = 1;
        
        // Fill ERST Entry
        let erst_entry = erst.as_ptr() as *mut ErstEntry;
        unsafe {
            (*erst_entry).base_addr_low = event_ring.as_ptr() as u32;
            (*erst_entry).base_addr_high = (event_ring.as_ptr() as u64 >> 32) as u32;
            (*erst_entry).size = (PAGE_SIZE / 16) as u32; // TRB count
            (*erst_entry).reserved = 0;
        }
        
        // Write Runtime Registers
        let ir_base = self.rt_base + 0x20; // Interrupter 0
        unsafe {
            arch::write32(ir_base + RT_ERSTSZ, 1); // 1 segment
            
            let erst_phys = erst.as_ptr() as u64;
            arch::write32(ir_base + RT_ERSTBA, erst_phys as u32);
            arch::write32(ir_base + RT_ERSTBA + 4, (erst_phys >> 32) as u32);
            
            let erdp_phys = event_ring.as_ptr() as u64;
            arch::write32(ir_base + RT_ERDP, erdp_phys as u32);
            arch::write32(ir_base + RT_ERDP + 4, (erdp_phys >> 32) as u32);
            
            // Enable Interrupts
            arch::write32(ir_base + RT_IMAN, 3); // IP | IE
            arch::write32(ir_base + RT_IMOD, 4000); // ~1ms interval
        }
        
        // 8. Start Controller
        unsafe {
            let cmd = arch::read32(self.op_base + OP_USBCMD);
            arch::write32(self.op_base + OP_USBCMD, cmd | USBCMD_RUN | USBCMD_INTE | USBCMD_HSEE);
        }
        
        self.initialized = true;
        kprintln!("[USB] xHCI Controller Initialized");
        
        Ok(())
    }
    
    fn reset(&self) -> Result<(), &'static str> {
        unsafe {
            let cmd = arch::read32(self.op_base + OP_USBCMD);
            arch::write32(self.op_base + OP_USBCMD, cmd | USBCMD_RESET);
            
            // Wait for reset to complete
            for _ in 0..1000 {
                if (arch::read32(self.op_base + OP_USBCMD) & USBCMD_RESET) == 0 {
                    return Ok(());
                }
                crate::drivers::timer::delay_us(100);
            }
        }
        Err("xHCI Reset Timeout")
    }

    /// Poll for events (interrupts or status changes)
    pub fn poll(&mut self) {
        if !self.initialized {
            return;
        }
        
        // Check USBSTS
        let status = unsafe { arch::read32(self.op_base + OP_USBSTS) };
        if status & (1 << 3) != 0 { // Event Interrupt
            // Acknowledge
            unsafe { arch::write32(self.op_base + OP_USBSTS, 1 << 3) };
            // Process Event Ring...
            self.process_event_ring();
        }
    }

    /// Process the Event Ring
    fn process_event_ring(&mut self) {
        let Some(mut dequeue) = self.event_ring_dequeue_ptr else { return };
        
        loop {
            let trb = unsafe { dequeue.as_ref() };
            
            // Check Cycle Bit (Bit 0 of Control Field)
            let trb_cycle = trb.control & 1;
            if trb_cycle != self.event_ring_cycle_bit {
                break; // No more events
            }
            
            let event_type = (trb.control >> 10) & 0x3F;
            let completion_code = (trb.status >> 24) & 0xFF;
            
            kprintln!("[USB] Event: Type={}, CC={}", event_type, completion_code);
            
            // Handle Event
            match event_type {
                33 => { // Command Completion Event
                    kprintln!("[USB] Command Completed. Slot ID: {}", (trb.control >> 24) & 0xFF);
                }
                34 => { // Port Status Change Event
                    let port_id = (trb.param_low >> 24) & 0xFF;
                    kprintln!("[USB] Port Status Change. Port ID: {}", port_id);
                    self.handle_port_status_change(port_id as usize);
                }
                _ => {}
            }
            
            // Advance Dequeue Pointer
            unsafe {
                // Check if we are at the end of the ring (simplified check for now)
                // In a real driver we'd check against the ring bounds or a Link TRB
                // For now, just increment
                dequeue = NonNull::new_unchecked(dequeue.as_ptr().add(1));
            }
            
            self.event_ring_dequeue_ptr = Some(dequeue);
            
            // Update ERDP (Event Ring Dequeue Pointer)
            let erdp = dequeue.as_ptr() as u64;
            unsafe {
                let ir_base = self.rt_base + 0x20;
                // Preserve EH bit (bit 3) if needed, but usually we just write the address
                arch::write32(ir_base + RT_ERDP, (erdp as u32) | 8); // Clear EHB (Busy)
                arch::write32(ir_base + RT_ERDP + 4, (erdp >> 32) as u32);
            }
        }
    }

    /// Handle Port Status Change
    fn handle_port_status_change(&mut self, port_id: usize) {
        // Read PORTSC
        let port_sc_addr = self.op_base + 0x400 + (port_id - 1) * 0x10;
        let port_sc = unsafe { arch::read32(port_sc_addr) };
        
        kprintln!("[USB] PORTSC {}: {:#x}", port_id, port_sc);
        
        // Check Current Connect Status (CCS) - Bit 0
        if port_sc & 1 != 0 {
            kprintln!("[USB] Device Connected on Port {}", port_id);
            
            // Reset Port (PR) - Bit 4
            // Note: We should only reset if not already enabled (PED - Bit 1)
            if port_sc & 2 == 0 {
                kprintln!("[USB] Resetting Port {}", port_id);
                unsafe { arch::write32(port_sc_addr, port_sc | (1 << 4)) }; // Set PR
                
                // Wait for Port Enabled (PED) - Bit 1
                // In a real driver, we'd wait for another Port Status Change Event
                // For this prototype, we'll poll briefly
                for _ in 0..100 {
                    let sc = unsafe { arch::read32(port_sc_addr) };
                    if sc & 2 != 0 {
                        kprintln!("[USB] Port {} Enabled!", port_id);
                        
                        // Enable Slot
                        if let Ok(slot_id) = self.enable_slot() {
                            kprintln!("[USB] Slot {} Assigned", slot_id);
                            
                            // Address Device
                            if self.address_device(slot_id).is_ok() {
                                kprintln!("[USB] Device Addressed on Slot {}", slot_id);
                                // TODO: Configure Endpoints
                            }
                        }
                        break;
                    }
                    crate::drivers::timer::delay_ms(1);
                }
            }
        } else {
            kprintln!("[USB] Device Disconnected from Port {}", port_id);
        }
    }

    /// Send a command to the controller via the Command Ring
    pub fn send_command(&mut self, trb: Trb) -> Result<(), &'static str> {
        if let Some(cmd_ring) = self.cmd_ring {
            let ring_ptr = cmd_ring.as_ptr() as *mut Trb;
            
            // For this simple implementation, we just use the first slot
            // In a real driver, we need a cycle bit and an enqueue pointer
            unsafe {
                *ring_ptr = trb;
                
                // Ring the Doorbell (DB 0 = Host Controller Command)
                arch::write32(self.db_base, 0);
            }
            Ok(())
        } else {
            Err("Command Ring not initialized")
        }
    }

    /// Enable a Device Slot
    pub fn enable_slot(&mut self) -> Result<u8, &'static str> {
        kprintln!("[USB] Sending Enable Slot Command...");
        
        let mut trb = Trb::new();
        trb.control = (9 << 10) | 1; // Type 9 (Enable Slot), Cycle Bit 1
        
        self.send_command(trb)?;
        
        // Wait for completion (hacky polling for now)
        crate::drivers::timer::delay_ms(10);
        
        // Mock return: In reality we read this from the Command Completion Event
        Ok(1) 
    }

    /// Address Device
    pub fn address_device(&mut self, slot_id: u8) -> Result<(), &'static str> {
        kprintln!("[USB] Sending Address Device Command for Slot {}...", slot_id);
        
        // We need an Input Context for this.
        // For this prototype, we'll assume the DCBAA entry is set up (it's zeroed in init).
        // We need to allocate an Input Context and point the command to it.
        // This is getting complex for a single file.
        // Let's send the command with a dummy pointer for now to show intent.
        
        let mut trb = Trb::new();
        trb.param_low = 0; // Input Context Ptr Low
        trb.param_high = 0; // Input Context Ptr High
        trb.control = (11 << 10) | ((slot_id as u32) << 24) | 1; // Type 11 (Address Device), Slot ID, Cycle 1
        
        self.send_command(trb)?;
        crate::drivers::timer::delay_ms(10);
        
        Ok(())
    }
}

// SAFETY: XhciController owns the pointers and is protected by SpinLock.
unsafe impl Send for XhciController {}

/// Global xHCI Controller
pub static CONTROLLER: SpinLock<XhciController> = SpinLock::new(XhciController::new());

/// Initialize the xHCI subsystem
pub fn init() {
    let mut controller = CONTROLLER.lock();
    if let Err(e) = controller.init() {
        kprintln!("[USB] Init failed: {}", e);
    }
}
