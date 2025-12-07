//! Intent Kernel Test Harness
//!
//! This is the main test runner for the Intent Kernel.
//! It uses a manual test registration system for no_std environment.

#![no_std]
#![no_main]

extern crate alloc;

use core::panic::PanicInfo;
use intent_kernel::*;

// ═══════════════════════════════════════════════════════════════════════════════
// MACROS FOR TEST OUTPUT
// ═══════════════════════════════════════════════════════════════════════════════

macro_rules! serial_print {
    ($($arg:tt)*) => {
        intent_kernel::drivers::uart::print(format_args!($($arg)*))
    };
}

macro_rules! serial_println {
    () => { serial_print!("\n") };
    ($($arg:tt)*) => {
        serial_print!("{}\n", format_args!($($arg)*))
    };
}

// ═══════════════════════════════════════════════════════════════════════════════
// TEST MODULES
// ═══════════════════════════════════════════════════════════════════════════════

mod unit {
    pub mod memory_tests;
    pub mod capability_tests;
    pub mod intent_tests;
    pub mod protection_tests;
}

// ═══════════════════════════════════════════════════════════════════════════════
// TEST RUNNER
// ═══════════════════════════════════════════════════════════════════════════════

fn run_test<F: Fn()>(name: &str, test: F) {
    serial_print!("{}...\t", name);
    test();
    serial_println!("[ok]");
}

fn run_all_tests() {
    serial_println!("Running tests...\n");
    
    // Memory tests
    run_test("memory::test_heap_stats", unit::memory_tests::test_heap_stats);
    run_test("memory::test_heap_available", unit::memory_tests::test_heap_available);
    run_test("memory::test_heap_regions", unit::memory_tests::test_heap_regions);
    run_test("memory::test_allocator_stats", unit::memory_tests::test_allocator_stats);
    
    // Capability tests
    run_test("capability::test_mint_root_capability", unit::capability_tests::test_mint_root_capability);
    run_test("capability::test_capability_validation", unit::capability_tests::test_capability_validation);
    run_test("capability::test_capability_permissions", unit::capability_tests::test_capability_permissions);
    run_test("capability::test_multiple_capabilities", unit::capability_tests::test_multiple_capabilities);
    
    // Intent tests
    run_test("intent::test_concept_id_hashing", unit::intent_tests::test_concept_id_hashing);
    run_test("intent::test_neural_memory_basic", unit::intent_tests::test_neural_memory_basic);
    run_test("intent::test_intent_system_initialization", unit::intent_tests::test_intent_system_initialization);

    // Protection tests
    run_test("protection::test_syscall_protection", unit::protection_tests::test_syscall_protection);
}

// ═══════════════════════════════════════════════════════════════════════════════
// RAW UART OUTPUT - bypass driver completely
// ═══════════════════════════════════════════════════════════════════════════════

#[inline(always)]
fn raw_uart(c: u8) {
    unsafe {
        core::ptr::write_volatile(0x0900_0000 as *mut u32, c as u32);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TEST ENTRY POINT
// ═══════════════════════════════════════════════════════════════════════════════

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    // Raw debug print to confirm entry - bypass all Rust abstractions
    raw_uart(b'R');
    raw_uart(b'I');

    // Initialize kernel subsystems for testing
    crate::init_for_tests();

    // Register boot agent so we have a valid current agent with Driver capabilities
    intent_kernel::kernel::scheduler::SCHEDULER.lock().register_boot_agent();
    
    crate::kprintln!("[TEST] Starting Tests...");
    
    // Run tests
    run_all_tests();

    // Exit QEMU with success
    intent_kernel::exit_qemu(intent_kernel::QemuExitCode::Success);
    
    loop {
        intent_kernel::arch::wfi();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PANIC HANDLER (for tests)
// ═══════════════════════════════════════════════════════════════════════════════

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("\n[FAILED]\n");
    serial_println!("╔═══════════════════════════════════════════════════════════╗");
    serial_println!("║                    TEST PANIC                             ║");
    serial_println!("╚═══════════════════════════════════════════════════════════╝\n");
    
    if let Some(location) = info.location() {
        serial_println!("Location: {}:{}", location.file(), location.line());
    }
    
    serial_println!("Error: {}", info.message());
    
    intent_kernel::exit_qemu(intent_kernel::QemuExitCode::Failed);
    
    loop {
        intent_kernel::arch::wfi();
    }
}
