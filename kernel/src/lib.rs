//! ╔═══════════════════════════════════════════════════════════════════════════╗
//! ║                    INTENT KERNEL - LIBRARY ROOT                           ║
//! ║                 The Bridge Between Intent and Silicon                     ║
//! ╚═══════════════════════════════════════════════════════════════════════════╝
//!
//! This is the library root that exposes all kernel functionality for testing
//! and reuse. The binary entry point is in main.rs.

#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

// ═══════════════════════════════════════════════════════════════════════════════
// PUBLIC MODULES
// ═══════════════════════════════════════════════════════════════════════════════

pub mod arch;
pub mod drivers;
pub mod kernel;
pub mod intent;
pub mod perception;
pub mod fs;

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

/// Initialize the kernel subsystems (for testing)
pub fn init_for_tests() {
    // Initialize UART for test output
    drivers::uart::early_init();
    serial_println!("[TEST] UART initialized");
    
    // Initialize timer
    drivers::timer::init();
    serial_println!("[TEST] Timer initialized");
    
    // Initialize memory (with fixed seed for reproducibility)
    unsafe { kernel::memory::init(0x1234567890ABCDEF); }
    serial_println!("[TEST] Memory initialized");
    
    // Initialize capability system
    kernel::capability::init();
    serial_println!("[TEST] Capabilities initialized");
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
