//! ╔═══════════════════════════════════════════════════════════════════════════╗
//! ║                    INTENT KERNEL - LIBRARY ROOT                           ║
//! ║                 The Bridge Between Intent and Silicon                     ║
//! ╚═══════════════════════════════════════════════════════════════════════════╝
//!
//! This is the library root that exposes all kernel functionality for testing
//! and reuse. The binary entry point is in main.rs.

#![no_std]
// #![feature(custom_test_frameworks)]
// #![test_runner(crate::test_runner)]
// #![reexport_test_harness_main = "test_main"]

extern crate alloc;

// ═══════════════════════════════════════════════════════════════════════════════
// PUBLIC MODULES
// ═══════════════════════════════════════════════════════════════════════════════

pub mod arch;
pub mod drivers;
pub mod kernel;
pub mod intent;
pub mod steno;      // Stenographic input - strokes are the semantic primitive
pub mod english;    // English I/O layer - natural language interface to steno-native kernel
pub mod perception;
pub mod fs;
pub mod net;

// ═══════════════════════════════════════════════════════════════════════════════
// TESTING FRAMEWORK
// ═══════════════════════════════════════════════════════════════════════════════

pub trait Testable {
    fn run(&self);
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
    // If QEMU fails to exit, loop with wfi to save power
    loop {
        crate::arch::wfi();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use core::arch::asm;
    // On AArch64, semihosting parameters are 64-bit words
    let block = [0x20026, exit_code as u64];
    unsafe {
        // QEMU semihosting exit
        asm!(
            "mov x0, #0x18",      // ADP_Stopped_ApplicationExit
            "mov x1, {0}",
            "hlt #0xf000",
            in(reg) &block as *const _ as u64
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// LIBRARY INITIALIZATION (for tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Raw UART output - bypass all locks and abstractions
#[inline(always)]
pub fn raw_uart(c: u8) {
    unsafe {
        #[cfg(test)]
        core::ptr::write_volatile(0x0900_0000 as *mut u32, c as u32);
        #[cfg(not(test))]
        core::ptr::write_volatile(0x1_0020_1000 as *mut u32, c as u32);
    }
}

/// Initialize the kernel subsystems (for testing)
pub fn init_for_tests() {
    raw_uart(b'0');
    // Initialize UART for test output - skip for now, use raw
    // drivers::uart::early_init();
    raw_uart(b'1');
    
    // Initialize timer
    drivers::timer::init();
    raw_uart(b'2');
    
    // Initialize memory (with fixed seed for reproducibility)
    unsafe { kernel::memory::init(0x1234567890ABCDEF); }
    raw_uart(b'3');
    
    // Initialize capability system
    kernel::capability::init();
    raw_uart(b'4');
}

// ═══════════════════════════════════════════════════════════════════════════════
// MACROS (re-export for convenience)
// ═══════════════════════════════════════════════════════════════════════════════

#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {
        $crate::drivers::uart::print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! kprintln {
    () => { $crate::kprint!("\n") };
    ($($arg:tt)*) => {
        $crate::kprint!("{}\n", format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::drivers::uart::print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! serial_println {
    () => { $crate::serial_print!("\n") };
    ($($arg:tt)*) => {
        $crate::serial_print!("{}\n", format_args!($($arg)*))
    };
}
