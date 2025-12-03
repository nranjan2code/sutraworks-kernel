//! Exception Handling and Crash Reporting
//!
//! Handles synchronous exceptions (Data Abort, Instruction Abort), IRQs, and System Errors.
//! Provides detailed decoding of ESR_EL1 to diagnose faults.


use crate::arch;
use crate::drivers;

// ═══════════════════════════════════════════════════════════════════════════════
// EXCEPTION FRAME
// ═══════════════════════════════════════════════════════════════════════════════

/// Exception frame pushed by the assembly stubs in boot.s
#[repr(C)]
pub struct ExceptionFrame {
    pub x: [u64; 30],       // x0-x29
    pub x30: u64,           // Link register
    pub elr: u64,           // Exception link register
    pub spsr: u64,          // Saved program status
    pub esr: u64,           // Exception syndrome
    pub far: u64,           // Fault address
}

impl ExceptionFrame {
    pub fn dump(&self) {
        crate::kprintln!("Register dump:");
        crate::kprintln!("  x0:  {:#018x}  x1:  {:#018x}", self.x[0], self.x[1]);
        crate::kprintln!("  x2:  {:#018x}  x3:  {:#018x}", self.x[2], self.x[3]);
        crate::kprintln!("  x4:  {:#018x}  x5:  {:#018x}", self.x[4], self.x[5]);
        crate::kprintln!("  x6:  {:#018x}  x7:  {:#018x}", self.x[6], self.x[7]);
        crate::kprintln!("  x8:  {:#018x}  x9:  {:#018x}", self.x[8], self.x[9]);
        crate::kprintln!("  x10: {:#018x}  x11: {:#018x}", self.x[10], self.x[11]);
        crate::kprintln!("  x12: {:#018x}  x13: {:#018x}", self.x[12], self.x[13]);
        crate::kprintln!("  x14: {:#018x}  x15: {:#018x}", self.x[14], self.x[15]);
        crate::kprintln!("  x16: {:#018x}  x17: {:#018x}", self.x[16], self.x[17]);
        crate::kprintln!("  x18: {:#018x}  x19: {:#018x}", self.x[18], self.x[19]);
        crate::kprintln!("  x20: {:#018x}  x21: {:#018x}", self.x[20], self.x[21]);
        crate::kprintln!("  x22: {:#018x}  x23: {:#018x}", self.x[22], self.x[23]);
        crate::kprintln!("  x24: {:#018x}  x25: {:#018x}", self.x[24], self.x[25]);
        crate::kprintln!("  x26: {:#018x}  x27: {:#018x}", self.x[26], self.x[27]);
        crate::kprintln!("  x28: {:#018x}  x29: {:#018x}", self.x[28], self.x[29]);
        crate::kprintln!("  x30: {:#018x}", self.x30);
        crate::kprintln!("  ELR: {:#018x}", self.elr);
        crate::kprintln!("  SPSR:{:#018x}", self.spsr);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ESR DECODING
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ExceptionClass {
    Unknown = 0b000000,
    TrappedWFI = 0b000001,
    IllegalState = 0b001110,
    SVC = 0b010101,
    InstrAbortLower = 0b100000,
    InstrAbortSame = 0b100001,
    PCAlignment = 0b100010,
    DataAbortLower = 0b100100,
    DataAbortSame = 0b100101,
    SPAlignment = 0b100110,
    SError = 0b101111,
    BreakpointLower = 0b110000,
    BreakpointSame = 0b110001,
    SoftwareStepLower = 0b110010,
    SoftwareStepSame = 0b110011,
    WatchpointLower = 0b110100,
    WatchpointSame = 0b110101,
    BRK = 0b111100,
}

impl From<u64> for ExceptionClass {
    fn from(esr: u64) -> Self {
        let ec = (esr >> 26) & 0x3f;
        match ec {
            0b000001 => ExceptionClass::TrappedWFI,
            0b001110 => ExceptionClass::IllegalState,
            0b010101 => ExceptionClass::SVC,
            0b100000 => ExceptionClass::InstrAbortLower,
            0b100001 => ExceptionClass::InstrAbortSame,
            0b100010 => ExceptionClass::PCAlignment,
            0b100100 => ExceptionClass::DataAbortLower,
            0b100101 => ExceptionClass::DataAbortSame,
            0b100110 => ExceptionClass::SPAlignment,
            0b101111 => ExceptionClass::SError,
            0b110000 => ExceptionClass::BreakpointLower,
            0b110001 => ExceptionClass::BreakpointSame,
            0b110010 => ExceptionClass::SoftwareStepLower,
            0b110011 => ExceptionClass::SoftwareStepSame,
            0b110100 => ExceptionClass::WatchpointLower,
            0b110101 => ExceptionClass::WatchpointSame,
            0b111100 => ExceptionClass::BRK,
            _ => ExceptionClass::Unknown,
        }
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(u64)]
pub enum DataFaultStatusCode {
    AddressSizeLevel0 = 0b000000,
    AddressSizeLevel1 = 0b000001,
    AddressSizeLevel2 = 0b000010,
    AddressSizeLevel3 = 0b000011,
    TranslationLevel0 = 0b000100,
    TranslationLevel1 = 0b000101,
    TranslationLevel2 = 0b000110,
    TranslationLevel3 = 0b000111,
    AccessFlagLevel0 = 0b001000,
    AccessFlagLevel1 = 0b001001,
    AccessFlagLevel2 = 0b001010,
    AccessFlagLevel3 = 0b001011,
    PermissionLevel0 = 0b001100,
    PermissionLevel1 = 0b001101,
    PermissionLevel2 = 0b001110,
    PermissionLevel3 = 0b001111,
    AlignmentFault = 0b100001,
    TLBConflict = 0b110000,
    Unknown(u64),
}

impl From<u64> for DataFaultStatusCode {
    fn from(iss: u64) -> Self {
        match iss & 0x3F {
            0b000000 => DataFaultStatusCode::AddressSizeLevel0,
            0b000001 => DataFaultStatusCode::AddressSizeLevel1,
            0b000010 => DataFaultStatusCode::AddressSizeLevel2,
            0b000011 => DataFaultStatusCode::AddressSizeLevel3,
            0b000100 => DataFaultStatusCode::TranslationLevel0,
            0b000101 => DataFaultStatusCode::TranslationLevel1,
            0b000110 => DataFaultStatusCode::TranslationLevel2,
            0b000111 => DataFaultStatusCode::TranslationLevel3,
            0b001000 => DataFaultStatusCode::AccessFlagLevel0,
            0b001001 => DataFaultStatusCode::AccessFlagLevel1,
            0b001010 => DataFaultStatusCode::AccessFlagLevel2,
            0b001011 => DataFaultStatusCode::AccessFlagLevel3,
            0b001100 => DataFaultStatusCode::PermissionLevel0,
            0b001101 => DataFaultStatusCode::PermissionLevel1,
            0b001110 => DataFaultStatusCode::PermissionLevel2,
            0b001111 => DataFaultStatusCode::PermissionLevel3,
            0b100001 => DataFaultStatusCode::AlignmentFault,
            0b110000 => DataFaultStatusCode::TLBConflict,
            code => DataFaultStatusCode::Unknown(code),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// HANDLERS
// ═══════════════════════════════════════════════════════════════════════════════

#[no_mangle]
pub extern "C" fn handle_exception(frame: *mut ExceptionFrame) {
    let frame_ref = unsafe { &*frame };
    let ec = ExceptionClass::from(frame_ref.esr);
    
    crate::kprintln!();
    crate::kprintln!("╔═══════════════════════════════════════════════════════════╗");
    crate::kprintln!("║                   EXCEPTION CAUGHT                        ║");
    crate::kprintln!("╚═══════════════════════════════════════════════════════════╝");
    crate::kprintln!();
    
    crate::kprintln!("Exception Class: {:?}", ec);
    crate::kprintln!("ESR: {:#018x}", frame_ref.esr);
    crate::kprintln!("FAR: {:#018x}", frame_ref.far);
    
    match ec {
        ExceptionClass::DataAbortLower | ExceptionClass::DataAbortSame => {
            let iss = frame_ref.esr & 0x1FFFFFF;
            let wnr = (iss >> 6) & 1;
            let dfsc = DataFaultStatusCode::from(iss);
            
            crate::kprintln!("Data Abort:");
            crate::kprintln!("  Operation: {}", if wnr == 1 { "WRITE" } else { "READ" });
            crate::kprintln!("  Status:    {:?}", dfsc);
            
            match dfsc {
                DataFaultStatusCode::TranslationLevel0 |
                DataFaultStatusCode::TranslationLevel1 |
                DataFaultStatusCode::TranslationLevel2 |
                DataFaultStatusCode::TranslationLevel3 => {
                    crate::kprintln!("  -> Page not mapped or invalid address.");
                }
                DataFaultStatusCode::PermissionLevel0 |
                DataFaultStatusCode::PermissionLevel1 |
                DataFaultStatusCode::PermissionLevel2 |
                DataFaultStatusCode::PermissionLevel3 => {
                    crate::kprintln!("  -> Permission violation (e.g. writing to RO).");
                }
                _ => {}
            }
        }
        ExceptionClass::SVC => {
            // System Call
            // x8 = syscall number
            // x0-x7 = arguments
            
            // We need mutable access to the frame to set return value and advance PC
            let frame_mut = unsafe { &mut *frame };
            
            let syscall_num = frame_mut.x[8];
            let arg0 = frame_mut.x[0];
            let arg1 = frame_mut.x[1];
            let arg2 = frame_mut.x[2];
            
            let ret = crate::kernel::syscall::dispatcher(syscall_num, arg0, arg1, arg2);
            
            // Set return value
            frame_mut.x[0] = ret;
            
            // Advance PC to next instruction (SVC is 4 bytes)
            frame_mut.elr += 4;
            
            // Return immediately (don't halt)
            return;
        }
        ExceptionClass::InstrAbortLower | ExceptionClass::InstrAbortSame => {
            let iss = frame_ref.esr & 0x1FFFFFF;
            let dfsc = DataFaultStatusCode::from(iss);
            crate::kprintln!("Instruction Abort:");
            crate::kprintln!("  Status: {:?}", dfsc);
        }
        _ => {}
    }
    
    frame_ref.dump();
    
    crate::kprintln!();
    crate::kprintln!("System halted due to fatal exception.");
    loop {
        arch::wfi();
    }
}

#[no_mangle]
pub extern "C" fn handle_irq(_frame: *mut ExceptionFrame) {
    let irq = drivers::interrupts::gic().acknowledge();
    
    // Check for spurious interrupt (1023)
    if irq == 1023 {
        return;
    }

    // Timer Interrupt (PPI 30)
    if irq == 30 {
        crate::kernel::scheduler::tick();
    } else {
        // Dispatch to other handlers
        drivers::interrupts::dispatch(irq);
    }
    
    drivers::interrupts::gic().end_of_interrupt(irq);
}

#[no_mangle]
pub extern "C" fn handle_fiq(_frame: *mut ExceptionFrame) {
    // FIQ not used
}

#[no_mangle]
pub extern "C" fn handle_serror(frame: *mut ExceptionFrame) {
    crate::kprintln!("!!! SYSTEM ERROR (SError) !!!");
    handle_exception(frame);
}

#[no_mangle]
pub extern "C" fn handle_sync_lower(frame: *mut ExceptionFrame) {
    handle_exception(frame);
}

#[no_mangle]
pub extern "C" fn handle_irq_lower(_frame: *mut ExceptionFrame) {
    // Handle IRQ from Lower EL (EL0) same as EL1
    handle_irq(_frame);
}

// ═══════════════════════════════════════════════════════════════════════════════
// PANIC HANDLER
// ═══════════════════════════════════════════════════════════════════════════════


