//! Intent Kernel Test Harness
//!
//! This is the main test runner for the Intent Kernel.
//! It uses a manual test registration system for no_std environment.

#![no_std]
#![no_main]

use core::panic::PanicInfo;
use intent_kernel::*;

// ═══════════════════════════════════════════════════════════════════════════════
// MOCK ASSEMBLY FUNCTIONS (for linking)
// ═══════════════════════════════════════════════════════════════════════════════

#[no_mangle]
pub extern "C" fn read_timer_freq() -> u64 {
    1_000_000 // 1 MHz mock frequency
}

#[no_mangle]
pub extern "C" fn read_timer() -> u64 {
    0 // Mock timer value
}

#[no_mangle]
pub extern "C" fn data_sync_barrier() {
    // Mock barrier - no-op for tests
}

#[no_mangle]
pub extern "C" fn instruction_barrier() {
    // Mock barrier - no-op for tests
}

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
    run_test("intent::test_embedding_creation", unit::intent_tests::test_embedding_creation);
    run_test("intent::test_embedding_similarity_identical", unit::intent_tests::test_embedding_similarity_identical);
    run_test("intent::test_embedding_similarity_different", unit::intent_tests::test_embedding_similarity_different);
    run_test("intent::test_neural_memory_basic", unit::intent_tests::test_neural_memory_basic);
    run_test("intent::test_neural_memory_threshold", unit::intent_tests::test_neural_memory_threshold);
}

// ═══════════════════════════════════════════════════════════════════════════════
// TEST ENTRY POINT
// ═══════════════════════════════════════════════════════════════════════════════

#[no_mangle]
#[unsafe(naked)]
#[link_section = ".text.boot"]
pub extern "C" fn _start() -> ! {
    core::arch::naked_asm!(
        "ldr x30, =__stack_top",
        "mov sp, x30",
        "bl test_main_entry",
        "b .",
    );
}

#[no_mangle]
pub extern "C" fn test_main_entry() -> ! {
    // Initialize kernel subsystems for testing
    intent_kernel::init_for_tests();
    
    serial_println!("\n╔═══════════════════════════════════════════════════════════╗");
    serial_println!("║           INTENT KERNEL TEST SUITE                       ║");
    serial_println!("╚═══════════════════════════════════════════════════════════╝\n");
    
    // Run all tests
    run_all_tests();
    
    serial_println!("\n╔═══════════════════════════════════════════════════════════╗");
    serial_println!("║           ALL TESTS PASSED                                ║");
    serial_println!("╚═══════════════════════════════════════════════════════════╝\n");
    
    // Exit QEMU
    intent_kernel::exit_qemu(intent_kernel::QemuExitCode::Success);
    
    loop {}
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
    
    loop {}
}
