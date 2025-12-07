#![no_std]
#![no_main]

// ══════════════════════════════════════════════════════════════════════════════
// HELLO APP - VERIFIES DYNAMIC LOADING
// ══════════════════════════════════════════════════════════════════════════════

use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    print("\n\x1b[32m[HELLO] Hello from the SD Card! process is running!\x1b[0m\n");
    print("[HELLO] Exiting...\n");
    exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    exit(1);
}

// Minimal Syscall Wrappers
const SYS_EXIT: u64 = 0;
const SYS_PRINT: u64 = 2;

fn print(s: &str) {
    unsafe {
        core::arch::asm!(
            "svc #2",
            in("x0") s.as_ptr(),
            in("x1") s.len(),
        );
    }
}

fn exit(code: i32) -> ! {
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") code,
        );
    }
    loop {}
}
