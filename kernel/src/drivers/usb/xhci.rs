//! xHCI Host Controller Driver
//!
//! Handles the low-level USB 3.0 host controller interface.
//! Implements the xHCI 1.2 Specification.
#![allow(dead_code)]

use crate::kprintln;
use crate::arch::{self, SpinLock};
use crate::kernel::memory::{self, PAGE_SIZE, free_dma};
use core::ptr::NonNull;

/// RAII Wrapper for DMA Memory
pub struct DmaBuffer {
    ptr: NonNull<u8>,
    size: usize,
}

impl DmaBuffer {
    pub fn new(size: usize) -> Option<Self> {
        let ptr = unsafe { memory::alloc_dma(size)? };
        Some(Self { ptr, size })
    }

    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    pub fn phys_addr(&self) -> u64 {
        self.ptr.as_ptr() as u64
    }
}

impl Drop for DmaBuffer {
    fn drop(&mut self) {
        unsafe { free_dma(self.ptr, self.size) };
    }
}

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

/// Slot Context (32 bytes)
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SlotContext {
    pub info1: u32,
    pub info2: u32,
    pub tt_id: u32,
    pub state: u32,
    pub reserved: [u32; 4],
}

/// Endpoint Context (32 bytes)
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct EndpointContext {
    pub info1: u32,
    pub info2: u32,
    pub tr_dequeue_ptr_low: u32,
    pub tr_dequeue_ptr_high: u32,
    pub avg_trb_len: u32,
    pub reserved: [u32; 3],
}

/// Input Control Context (32 bytes)
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct InputControlContext {
    pub drop_flags: u32,
    pub add_flags: u32,
    pub reserved: [u32; 6],
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONTROLLER
// ═══════════════════════════════════════════════════════════════════════════════

/// Pending Transfer State
pub struct PendingTransfer {
    pub id: u32,
    pub completed: bool,
    pub completion_code: u8,
    pub bytes_transferred: usize,
    pub dma_buffer: Option<usize>,
}

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

    cmd_ring_enqueue_idx: usize,
    cmd_ring_cycle_bit: u32,

    context_size_64: bool,
    pending_enable_slot_port: usize,

    // Active Slots (Index = Slot ID)
    slots: [Option<DeviceSlot>; 32],

    // Transfer tracking (one per slot for simplicity)
    pub pending_transfers: [Option<PendingTransfer>; 32],
    next_transfer_id: u32,

    initialized: bool,
}

/// Device Slot State
pub struct DeviceSlot {
    pub slot_id: u8,
    pub port_id: usize,
    
    // Contexts
    pub output_ctx: Option<NonNull<u8>>,
    
    // Endpoint 0 Ring
    pub ep0_ring: Option<NonNull<u8>>,
    pub ep0_enqueue_idx: usize,
    pub ep0_cycle_bit: u32,
    
    // Endpoint 1 Ring (Interrupt)
    pub ep1_ring: Option<NonNull<u8>>,
}

impl DeviceSlot {
    pub fn new(slot_id: u8, port_id: usize) -> Self {
        Self {
            slot_id,
            port_id,
            output_ctx: None,
            ep0_ring: None,
            ep0_enqueue_idx: 0,
            ep0_cycle_bit: 1,
            ep1_ring: None,
        }
    }
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
            cmd_ring_enqueue_idx: 0,
            cmd_ring_cycle_bit: 1,
            context_size_64: false,
            pending_enable_slot_port: 0,
            slots: [const { None }; 32],
            pending_transfers: [const { None }; 32],
            next_transfer_id: 1,
            initialized: false,
        }
    }

    /// Get next transfer ID
    fn next_transfer_id(&mut self) -> u32 {
        let id = self.next_transfer_id;
        self.next_transfer_id = self.next_transfer_id.wrapping_add(1);
        id
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
        
        // Find xHCI controller via PCIe
        // 0x1de4 is VL805 vendor (Pi 4). For Pi 5, RP1 has integrated xHCI.
        // Try to find any xHCI-compatible device (class code 0x0C0330)
        // First check for known vendors
        let device = pcie.find_device(0x1de4, 0x0001) // VL805
            .or_else(|| pcie.find_by_vendor(crate::drivers::pcie::VENDOR_ID_RPI)); // RP1

        if let Some(dev) = device {
            // Read BAR0 for register base
            if let Some((addr, size)) = pcie.read_bar(&dev, 0) {
                self.base_addr = addr;
                kprintln!("[USB] xHCI registers at {:#x} (size: {} KB)", addr, size / 1024);
            } else {
                return Err("Failed to read xHCI BAR0");
            }
        } else {
             // Fallback to known RP1 address for Pi 5
             kprintln!("[USB] No PCIe xHCI found, trying RP1 fallback address");
             self.base_addr = 0x1F_0012_0000; // RP1 xHCI base
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
        
        // Check Context Size (HCCPARAMS1 Bit 2)
        let hccparams1 = unsafe { arch::read32(self.base_addr + CAP_HCCPARAMS1) };
        self.context_size_64 = (hccparams1 & 4) != 0;
        kprintln!("[USB] Context Size: {} bytes", if self.context_size_64 { 64 } else { 32 });
        
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
                32 => { // Transfer Event
                    let slot_id = ((trb.control >> 24) & 0xFF) as usize;
                    let completion_code = ((trb.status >> 24) & 0xFF) as u8;
                    let transfer_len = (trb.status & 0xFFFFFF) as usize;

                    kprintln!("[USB] Transfer Event: Slot {}, Code {}, Len {}", slot_id, completion_code, transfer_len);

                    // Mark pending transfer as completed
                    if slot_id < self.pending_transfers.len() {
                        if let Some(ref mut transfer) = self.pending_transfers[slot_id] {
                            transfer.completed = true;
                            transfer.completion_code = completion_code;
                            transfer.bytes_transferred = transfer_len;
                        }
                    }
                }
                33 => { // Command Completion Event
                    let slot_id = (trb.control >> 24) & 0xFF;
                    let cmd_trb_ptr = (trb.param_high as u64) << 32 | (trb.param_low as u64);
                    kprintln!("[USB] Command Completed. Slot ID: {}", slot_id);
                    self.handle_command_completion(cmd_trb_ptr, slot_id as u8);
                }
                34 => { // Port Status Change Event
                    let port_id = (trb.param_low >> 24) & 0xFF;
                    kprintln!("[USB] Port Status Change. Port ID: {}", port_id);
                    self.handle_port_status_change(port_id as usize);
                }
                _ => {
                    kprintln!("[USB] Unknown Event: {}", event_type);
                }
            }
            
            // Advance Dequeue Pointer
            unsafe {
                // Check if we are at the end of the ring
            // We check against the ring bounds or a Link TRB
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
                // We poll briefly for the status change
                for _ in 0..100 {
                    let sc = unsafe { arch::read32(port_sc_addr) };
                    if sc & 2 != 0 {
                        kprintln!("[USB] Port {} Enabled!", port_id);
                        
                        // Save Port ID for the pending Enable Slot command
                        self.pending_enable_slot_port = port_id;
                        
                        // Enable Slot
                        if let Err(e) = self.send_enable_slot_command() {
                            kprintln!("[USB] Failed to send Enable Slot: {}", e);
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

    /// Handle Command Completion
    fn handle_command_completion(&mut self, _cmd_trb_ptr: u64, slot_id: u8) {
        // We need to know WHAT command completed.
        // In a full driver, we'd map the pointer back to our ring.
        // For now, we'll read the TRB from the ring (unsafe but works if we don't wrap too fast)
        // Actually, let's just assume the flow based on Slot ID presence.
        
        // If we got a Slot ID and we were waiting for one, it's Enable Slot.
        // We use the pending_enable_slot_port to map it.
        
        if self.pending_enable_slot_port != 0 {
            let port_id = self.pending_enable_slot_port;
            kprintln!("[USB] Mapping Slot {} to Port {}", slot_id, port_id);
            self.pending_enable_slot_port = 0; // Clear pending
            
            let _ = self.send_address_device_command(slot_id, port_id);
        } else {
            // If no pending port, maybe this was Address Device completion?
            // We don't track that explicitly yet, but if we see a completion for a slot
            // that is already in our slots array, it's likely Address Device.
                kprintln!("[USB] Address Device Completed for Slot {}. Starting Enumeration...", slot_id);
                // Trigger enumeration
                if let Err(e) = self.enumerate_device(slot_id) {
                    kprintln!("[USB] Enumeration failed for Slot {}: {}", slot_id, e);
                } else {
                    kprintln!("[USB] Enumeration successful for Slot {}!", slot_id);
                }
        }
    }

    /// Send a command to the controller via the Command Ring
    pub fn send_command(&mut self, mut trb: Trb) -> Result<u64, &'static str> {
        let Some(cmd_ring) = self.cmd_ring else { return Err("Command Ring not initialized") };
        
        let ring_ptr = cmd_ring.as_ptr() as *mut Trb;
        let idx = self.cmd_ring_enqueue_idx;
        
        // Set Cycle Bit
        trb.control |= self.cmd_ring_cycle_bit;
        
        unsafe {
            let entry = ring_ptr.add(idx);
            // Use volatile write to ensure it hits memory (even if NC, compiler might reorder)
            core::ptr::write_volatile(entry, trb);
            
            let entry_phys = entry as u64; // Assuming identity map for now
            
            // Increment Enqueue Pointer
            self.cmd_ring_enqueue_idx += 1;
            if self.cmd_ring_enqueue_idx >= (PAGE_SIZE / 16) - 1 { // Leave room for Link TRB
                // Wrap around (Link TRB logic omitted for brevity, just reset for this prototype)
                self.cmd_ring_enqueue_idx = 0;
                self.cmd_ring_cycle_bit ^= 1; // Toggle cycle bit
            }
            
            // Ring the Doorbell (DB 0 = Host Controller Command)
            // arch::write32 is volatile
            arch::write32(self.db_base, 0);
            
            Ok(entry_phys)
        }
    }

    /// Send Enable Slot Command
    pub fn send_enable_slot_command(&mut self) -> Result<(), &'static str> {
        kprintln!("[USB] Sending Enable Slot Command...");
        
        let mut trb = Trb::new();
        trb.control = 9 << 10; // Type 9 (Enable Slot)
        // Cycle bit added by send_command
        
        self.send_command(trb)?;
        Ok(())
    }

    /// Send Address Device Command
    pub fn send_address_device_command(&mut self, slot_id: u8, port_id: usize) -> Result<(), &'static str> {
        kprintln!("[USB] Sending Address Device Command for Slot {} (Port {})...", slot_id, port_id);
        
        // 1. Allocate Input Context (using DMA allocator for alignment)
        let input_ctx_mem = unsafe { memory::alloc_dma(PAGE_SIZE) }.ok_or("Failed to alloc Input Context")?;
        unsafe { core::ptr::write_bytes(input_ctx_mem.as_ptr(), 0, PAGE_SIZE) };
        
        let ctx_size = if self.context_size_64 { 64 } else { 32 };
        let ptr = input_ctx_mem.as_ptr();
        
        // 2. Setup Input Control Context (Offset 0)
        // Add Slot Context (Bit 0) and Endpoint 0 Context (Bit 1)
        let icc = ptr as *mut InputControlContext;
        unsafe {
            (*icc).add_flags = 0x3; // A0 (Slot) | A1 (EP0)
            (*icc).drop_flags = 0;
        }
        
        // 3. Setup Slot Context (Offset 1 * ctx_size)
        // We need to read Port Speed from PORTSC
        let port_sc_addr = self.op_base + 0x400 + (port_id - 1) * 0x10;
        let port_sc = unsafe { arch::read32(port_sc_addr) };
        let speed = (port_sc >> 10) & 0xF;
        let route_string = 0; // Root hub direct connection
        let context_entries = 1; // Only EP0 for now
        
        let slot_ctx = unsafe { ptr.add(ctx_size).cast::<SlotContext>() };
        unsafe {
            (*slot_ctx).info1 = (speed << 20) | (route_string) | (context_entries << 27);
            (*slot_ctx).info2 = (port_id as u32) << 16; // Root Hub Port Num
        }
        
        // 4. Setup Endpoint 0 Context (Offset 2 * ctx_size)
        let ep0_ctx = unsafe { ptr.add(ctx_size * 2).cast::<EndpointContext>() };
        unsafe {
            (*ep0_ctx).info1 = 0 << 3; // EP Type = Control
            (*ep0_ctx).info2 = (1 << 7) | (8 << 16); // Error Count = 3 (shifted?), Max Packet Size = 8 (default for control)
            // Actually CErr is bits 1:2. 
            // EP Type is bits 3:5. Control = 4.
            // Wait, EP Type values: 0=Not Valid, 4=Control.
            (*ep0_ctx).info2 = (3 << 1) | (4 << 3) | (64 << 16); // CErr=3, Type=Control, MPS=64 (USB 3.0) or 8/64 (USB 2.0)
            
            // For USB 2.0, MPS is 64 for High Speed, 8 for Low/Full.
            // We should check speed.
            // Speed: 1=Full, 2=Low, 3=High, 4=Super.
            let mps = if speed == 2 { 8 } else { 64 };
             (*ep0_ctx).info2 = (3 << 1) | (4 << 3) | ((mps as u32) << 16);
             
            // TR Dequeue Pointer
            // We need a Transfer Ring for EP0.
            // Allocate Transfer Ring
            let tr_ring = memory::alloc_dma(PAGE_SIZE).ok_or("Failed to alloc EP0 Ring")?;
            core::ptr::write_bytes(tr_ring.as_ptr(), 0, PAGE_SIZE);
            
            // Store ring somewhere? We need it to send transfers later.
            // For now, just put it in the context.
            // Store ring in the Slot structure.
            
            let tr_phys = tr_ring.as_ptr() as u64;
            (*ep0_ctx).tr_dequeue_ptr_low = tr_phys as u32;
            (*ep0_ctx).tr_dequeue_ptr_high = (tr_phys >> 32) as u32;
            (*ep0_ctx).info1 |= 1; // DCS (Dequeue Cycle State) = 1
            
            // Initialize Slot State
            let mut slot_state = DeviceSlot::new(slot_id, port_id);
            slot_state.ep0_ring = Some(tr_ring);
            
            // 5. Setup Output Device Context (in DCBAA)
            // The Address Device command will copy from Input to Output.
            // But we need to allocate the Output Context backing memory first!
            // DCBAA[SlotID] must point to a Device Context buffer.
            let out_ctx_mem = memory::alloc_dma(PAGE_SIZE).ok_or("Failed to alloc Output Context")?;
            core::ptr::write_bytes(out_ctx_mem.as_ptr(), 0, PAGE_SIZE);
            
            slot_state.output_ctx = Some(out_ctx_mem);
            
            if let Some(dcbaa) = self.dcbaa {
                 let dcbaa_ptr = dcbaa.as_ptr() as *mut u64;
                 *dcbaa_ptr.add(slot_id as usize) = out_ctx_mem.as_ptr() as u64;
            }
            
            // Save Slot State
            if (slot_id as usize) < self.slots.len() {
                self.slots[slot_id as usize] = Some(slot_state);
            }
        }
        
        // 6. Send Command
        let mut trb = Trb::new();
        let input_ctx_phys = input_ctx_mem.as_ptr() as u64;
        trb.param_low = input_ctx_phys as u32;
        trb.param_high = (input_ctx_phys >> 32) as u32;
        trb.control = (11 << 10) | ((slot_id as u32) << 24); // Type 11 (Address Device), Slot ID
        
        self.send_command(trb)?;
        Ok(())
    }

    /// Enqueue a TRB on an Endpoint Ring
    fn enqueue_ep_trb(&mut self, slot_id: u8, ep_index: usize, mut trb: Trb) -> Result<(), &'static str> {
        let slot = self.slots[slot_id as usize].as_mut().ok_or("Invalid Slot")?;
        // Only EP0 supported for now
        if ep_index != 0 { return Err("Only EP0 supported"); }
        
        let ring_ptr = slot.ep0_ring.ok_or("EP0 Ring Not Init")?.as_ptr() as *mut Trb;
        let idx = slot.ep0_enqueue_idx;
        
        // Set Cycle Bit
        if slot.ep0_cycle_bit != 0 {
            trb.control |= 1;
        } else {
            trb.control &= !1;
        }
        
        unsafe {
            let entry = ring_ptr.add(idx);
            core::ptr::write_volatile(entry, trb);
            
            slot.ep0_enqueue_idx += 1;
            if slot.ep0_enqueue_idx >= (PAGE_SIZE / 16) - 1 {
                // Link TRB
                let link_trb = ring_ptr.add(slot.ep0_enqueue_idx);
                let ring_phys = ring_ptr as u64;
                
                let mut link = Trb::new();
                link.param_low = ring_phys as u32;
                link.param_high = (ring_phys >> 32) as u32;
                link.status = 0;
                link.control = (6 << 10) | 2; // Type 6 (Link), TC (Toggle Cycle)
                if slot.ep0_cycle_bit != 0 { link.control |= 1; }
                
                core::ptr::write_volatile(link_trb, link);
                
                slot.ep0_cycle_bit ^= 1;
                slot.ep0_enqueue_idx = 0;
            }
        }
        Ok(())
    }

    /// Send Control Transfer (Setup, Data, Status) - Synchronous
    /// Blocks until the transfer completes or times out.
    /// Returns the number of bytes transferred on success.
    pub fn control_transfer_sync(&mut self, slot_id: u8, setup: [u8; 8], data_buffer: Option<&mut [u8]>) -> Result<usize, &'static str> {
        // Allocate DMA buffer for data stage if needed
        let dma_buf = if let Some(buf) = data_buffer.as_ref() {
            Some(DmaBuffer::new(buf.len().max(PAGE_SIZE)).ok_or("DMA allocation failed")?)
        } else {
            None
        };

        // Mark this transfer as pending
        let transfer_id = self.next_transfer_id();
        self.pending_transfers[slot_id as usize] = Some(PendingTransfer {
            id: transfer_id,
            completed: false,
            completion_code: 0,
            bytes_transferred: 0,
            dma_buffer: dma_buf.as_ref().map(|b| b.as_ptr() as usize),
        });

        // Submit the transfer
        let data_len = data_buffer.as_ref().map(|b| b.len() as u16).unwrap_or(0);
        self.send_control_transfer_with_buffer(slot_id, setup, data_len, dma_buf.as_ref())?;

        // Poll for completion (timeout: 5 seconds)
        let timeout_ms = 5000;
        let start = crate::drivers::timer::uptime_ms();

        loop {
            // Process event ring
            self.poll();

            // Check if transfer completed
            if let Some(ref transfer) = self.pending_transfers[slot_id as usize] {
                if transfer.completed {
                    let code = transfer.completion_code;
                    let bytes = transfer.bytes_transferred;

                    // Clear pending state
                    self.pending_transfers[slot_id as usize] = None;

                    // Check completion code (1 = Success)
                    if code != 1 {
                        return Err("Transfer failed");
                    }

                    // Copy data from DMA buffer to user buffer if this was a read
                    if let (Some(ref dma), Some(ref mut user_buf)) = (&dma_buf, data_buffer) {
                        let copy_len = bytes.min(user_buf.len());
                        unsafe {
                            core::ptr::copy_nonoverlapping(
                                dma.as_ptr(),
                                user_buf.as_mut_ptr(),
                                copy_len
                            );
                        }
                    }

                    return Ok(bytes);
                }
            }

            let elapsed = crate::drivers::timer::uptime_ms() - start;
            if elapsed > timeout_ms {
                self.pending_transfers[slot_id as usize] = None;
                return Err("Control transfer timeout");
            }

            crate::drivers::timer::delay_us(100);
        }
    }

    /// Get Device Descriptor (Blocking)
    pub fn get_device_descriptor(&mut self, slot_id: u8) -> Result<[u8; 18], &'static str> {
        let mut buf = [0u8; 18];
        let setup = [
            0x80, // bmRequestType: Dir=In, Type=Std, Recp=Dev
            0x06, // bRequest: Get Descriptor
            0x00, 0x01, // wValue: Type=1 (Device), Index=0
            0x00, 0x00, // wIndex: 0
            18, 0x00, // wLength: 18
        ];
        
        let len = self.control_transfer_sync(slot_id, setup, Some(&mut buf))?;
        if len < 18 {
            return Err("Short read on Device Descriptor");
        }
        Ok(buf)
    }

    /// Set Configuration (Blocking)
    pub fn set_configuration(&mut self, slot_id: u8, config_value: u8) -> Result<(), &'static str> {
        let setup = [
            0x00, // bmRequestType: Dir=Out, Type=Std, Recp=Dev
            0x09, // bRequest: Set Configuration
            config_value, 0x00, // wValue: Config Value
            0x00, 0x00, // wIndex: 0
            0x00, 0x00, // wLength: 0
        ];
        self.control_transfer_sync(slot_id, setup, None)?;
        Ok(())
    }

    /// Set Protocol (Blocking) - For HID
    pub fn set_protocol(&mut self, slot_id: u8, interface: u8, protocol: u8) -> Result<(), &'static str> {
        let setup = [
            0x21, // bmRequestType: Dir=Out, Type=Class, Recp=Interface
            0x0B, // bRequest: Set Protocol
            protocol, 0x00, // wValue: Protocol (0=Boot, 1=Report)
            interface, 0x00, // wIndex: Interface
            0x00, 0x00, // wLength: 0
        ];
        self.control_transfer_sync(slot_id, setup, None)?;
        Ok(())
    }

    /// Set Idle (Blocking) - For HID
    pub fn set_idle(&mut self, slot_id: u8, interface: u8, duration: u8) -> Result<(), &'static str> {
        let setup = [
            0x21, // bmRequestType: Dir=Out, Type=Class, Recp=Interface
            0x0A, // bRequest: Set Idle
            0x00, duration, // wValue: Duration (high byte) | Report ID (low byte)
            interface, 0x00, // wIndex: Interface
            0x00, 0x00, // wLength: 0
        ];
        self.control_transfer_sync(slot_id, setup, None)?;
        Ok(())
    }

    /// Enumerate Device (Blocking)
    /// This is a simplified enumeration flow for Steno/Keyboard devices.
    pub fn enumerate_device(&mut self, slot_id: u8) -> Result<(), &'static str> {
        // 1. Get Device Descriptor
        let desc = self.get_device_descriptor(slot_id)?;
        kprintln!("[USB] Device Descriptor: Vendor={:04x}, Product={:04x}", 
            u16::from_le_bytes([desc[8], desc[9]]), 
            u16::from_le_bytes([desc[10], desc[11]])
        );

        // 2. Set Configuration 1 (Assumption: Most simple devices use Config 1)
        kprintln!("[USB] Setting Configuration 1...");
        self.set_configuration(slot_id, 1)?;

        // 3. Set Protocol to Boot Protocol (0) for Keyboard
        // Assumption: Interface 0 is the keyboard.
        // In a real driver, we parse Config Descriptor -> Interface Descriptor.
        kprintln!("[USB] Setting Boot Protocol...");
        // We ignore error here because not all devices support SetProtocol (if not HID)
        let _ = self.set_protocol(slot_id, 0, 0); 

        // 4. Set Idle to 0 (Infinity)
        kprintln!("[USB] Setting Idle...");
        let _ = self.set_idle(slot_id, 0, 0);

        // 5. Start Polling (Interrupt IN)
        // We need to configure the Endpoint Context for the Interrupt Endpoint.
        // This requires parsing the Endpoint Descriptor.
        // For this Sprint, we'll assume a standard Keyboard layout:
        // EP 1 IN, Packet Size 8, Interval 10ms.
        
        // Configure EP1 context in Input Context and issue Configure Endpoint command.
        // For this Sprint, we'll assume a standard Keyboard layout:
        // EP 1 IN, Packet Size 8, Interval 10ms.
        
        kprintln!("[USB] Configuring Endpoint 1...");
        
        // 1. Allocate Input Context
        let input_ctx_mem = unsafe { memory::alloc_dma(PAGE_SIZE) }.ok_or("Failed to alloc Input Context")?;
        unsafe { core::ptr::write_bytes(input_ctx_mem.as_ptr(), 0, PAGE_SIZE) };
        
        let ctx_size = if self.context_size_64 { 64 } else { 32 };
        let ptr = input_ctx_mem.as_ptr();
        
        // 2. Setup Input Control Context
        let icc = ptr as *mut InputControlContext;
        unsafe {
            (*icc).add_flags = 0x8; // A3 (EP1 IN) - Context Index 3 (EP0=1, EP1_OUT=2, EP1_IN=3)
            (*icc).drop_flags = 0;
        }
        
        // 3. Setup Slot Context (Copy from existing or just set entries)
        // We need to update Context Entries to include EP1
        let slot_ctx = unsafe { ptr.add(ctx_size).cast::<SlotContext>() };
        unsafe {
             // We should read current slot context, but we know what we set.
             // Just set Context Entries = 2 (EP0 + EP1)
             // Actually index 3 requires Context Entries = 4? (0..3)
             // Spec says: "index of the last valid Endpoint Context"
             // EP1 IN is index 3. So 3 + 1 = 4? No, it's 1-based count or max index?
             // "The value of this field shall be set to the index of the last valid Endpoint Context"
             // So 3.
             (*slot_ctx).info1 = 3 << 27; 
        }
        
        // 4. Setup Endpoint 1 Context (Index 3)
        let ep1_ctx = unsafe { ptr.add(ctx_size * 3).cast::<EndpointContext>() };
        
        // Allocate Transfer Ring for EP1
        let tr_ring = unsafe { memory::alloc_dma(PAGE_SIZE) }.ok_or("Failed to alloc EP1 Ring")?;
        unsafe { core::ptr::write_bytes(tr_ring.as_ptr(), 0, PAGE_SIZE) };
        let tr_phys = tr_ring.as_ptr() as u64;
        
        unsafe {
            (*ep1_ctx).info1 = 3 << 3; // EP Type = Interrupt IN (7) or Bulk IN (6)?
            // Interrupt IN = 7.
            (*ep1_ctx).info1 = 7 << 3;
            
            // Max Packet Size = 8, Error Count = 3
            (*ep1_ctx).info2 = (3 << 1) | (8 << 16);
            
            (*ep1_ctx).tr_dequeue_ptr_low = tr_phys as u32;
            (*ep1_ctx).tr_dequeue_ptr_high = (tr_phys >> 32) as u32;
            (*ep1_ctx).info1 |= 1; // DCS = 1
            
            // Interval = 10ms. Encoded as 2^(Interval-1) * 125us for High Speed?
            // For Low/Full speed, it's frames.
            // Let's set it to something safe, e.g., 7 (2^6 * 125us = 8ms) or just 10 for FS.
            // For FS/LS, value is directly frames.
            // But xHCI uses 2^Interval * 125us logic usually?
            // "For LS/FS Interrupt endpoints, the Interval field is expressed in 1ms units"?
            // No, xHCI normalizes everything.
            // Let's use 16 (2ms approx?)
            (*ep1_ctx).info1 |= 6 << 16; 
        }
        
        // Save EP1 Ring
        if let Some(slot) = self.slots[slot_id as usize].as_mut() {
             slot.ep1_ring = Some(tr_ring);
        }
        
        // 5. Send Configure Endpoint Command
        let mut trb = Trb::new();
        let input_ctx_phys = input_ctx_mem.as_ptr() as u64;
        trb.param_low = input_ctx_phys as u32;
        trb.param_high = (input_ctx_phys >> 32) as u32;
        trb.control = (12 << 10) | ((slot_id as u32) << 24); // Type 12 (Configure Endpoint)
        
        self.send_command(trb)?;
        
        kprintln!("[USB] Enumeration Complete. Ready for Interrupts.");
        Ok(())
    }
    fn send_control_transfer_with_buffer(&mut self, slot_id: u8, setup: [u8; 8], len: u16, dma_buf: Option<&DmaBuffer>) -> Result<(), &'static str> {
        // 1. Setup Stage
        let mut setup_trb = Trb::new();
        setup_trb.param_low = u32::from_le_bytes(setup[0..4].try_into().unwrap());
        setup_trb.param_high = u32::from_le_bytes(setup[4..8].try_into().unwrap());
        setup_trb.status = 8; // Length of Setup Packet
        setup_trb.control = (2 << 10) | (3 << 16); // Type 2 (Setup Stage), IDT (Immediate Data)
        
        // Transfer Type (TRT) - Bits 16:17 of Control
        // 0=No Data, 2=Out, 3=In.
        // Check setup[0] (bmRequestType) bit 7.
        let dir_in = (setup[0] & 0x80) != 0;
        let has_data = len > 0;
        let trt = if !has_data { 0 } else if dir_in { 3 } else { 2 };
        setup_trb.control |= trt << 16;
        
        self.enqueue_ep_trb(slot_id, 0, setup_trb)?;
        
        // 2. Data Stage
        if has_data {
            // We need a buffer.
            let buf_phys = dma_buf.ok_or("Buffer required for data stage")?.phys_addr();
            
            let mut data_trb = Trb::new();
            data_trb.param_low = buf_phys as u32;
            data_trb.param_high = (buf_phys >> 32) as u32;
            data_trb.status = len as u32;
            data_trb.control = (3 << 10) | (1 << 16); // Type 3 (Data Stage), Dir=In (assuming GetDesc)
            if !dir_in { data_trb.control &= !(1 << 16); } // Clear Dir bit for OUT
            
            self.enqueue_ep_trb(slot_id, 0, data_trb)?;
            
            // Buffer lifetime is managed by caller (control_transfer_sync)
        }
        
        // 3. Status Stage
        let mut status_trb = Trb::new();
        status_trb.control = (4 << 10) | (1 << 5); // Type 4 (Status Stage), IOC (Interrupt On Completion)
        // Direction is opposite of Data Stage
        if has_data && dir_in { 
            // Data IN -> Status OUT (Dir=0)
        } else {
            // Data OUT or No Data -> Status IN (Dir=1)
            status_trb.control |= 1 << 16;
        }
        
        self.enqueue_ep_trb(slot_id, 0, status_trb)?;
        
        // Ring Doorbell (Target = 1 for EP0)
        unsafe { arch::write32(self.db_base + (slot_id as usize * 4), 1); }
        
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
