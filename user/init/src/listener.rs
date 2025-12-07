#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::ffi::CStr;

// Syscall numbers
const SYS_PRINT: u64 = 2;
const SYS_BIND_UDP: u64 = 23;
const SYS_RECVFROM_FD: u64 = 24;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    print("[LISTENER] Starting UDP Echo Server on port 8080...\n");

    // 1. Bind to port 8080
    let fd = unsafe { syscall1(SYS_BIND_UDP, 8080) };
    if fd == u64::MAX {
        print("[LISTENER] Failed to bind port 8080\n");
        loop {}
    }
    
    // Print FD
    // We don't have formatted print, just static message
    print("[LISTENER] Bound to port 8080. Waiting for packets...\n");

    let mut buf = [0u8; 128];
    // Buffer for Source Addr (IP=4, Port=2, Pad=2 = 8 bytes)
    let mut src_addr = [0u8; 8];

    loop {
        // 2. Receive Packet (Blocking/Polling)
        // Since we implemented non-blocking receive in kernel, we might need to retry.
        // But let's assume we busy loop or sleep.
        
        let res = unsafe { 
            syscall4(SYS_RECVFROM_FD, fd, buf.as_mut_ptr() as u64, buf.len() as u64, src_addr.as_mut_ptr() as u64) 
        };

        if res != u64::MAX {
            let len = res as usize;
            
            // Print Info
            print("[LISTENER] Received packet!\n");
            
            // Should parse IP/Port from src_addr?
            // Simple Echo
            // TODO: sendto (not implemented yet, kernel has no sys_sendto)
            // Just print content
            
            if let Ok(s) = core::str::from_utf8(&buf[0..len]) {
                print("Content: ");
                print(s);
                print("\n");
            } else {
                print("Content: [Binary]\n");
            }
        }
        
        // Sleep a bit to avoid burning CPU
        // syscall1(3, 100); // Sleep 100ms
    }
}

pub fn print(s: &str) {
    unsafe {
        syscall2(SYS_PRINT, s.as_ptr() as u64, s.len() as u64);
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    print("[LISTENER] Panicked!\n");
    loop {}
}

// Raw Syscall Wrappers
#[inline(always)]
unsafe fn syscall1(n: u64, arg0: u64) -> u64 {
    let res;
    core::arch::asm!(
        "svc #0",
        in("x8") n,
        in("x0") arg0,
        lateout("x0") res,
        options(nostack)
    );
    res
}

#[inline(always)]
unsafe fn syscall2(n: u64, arg0: u64, arg1: u64) -> u64 {
    let res;
    core::arch::asm!(
        "svc #0",
        in("x8") n,
        in("x0") arg0,
        in("x1") arg1,
        lateout("x0") res,
        options(nostack)
    );
    res
}

#[inline(always)]
unsafe fn syscall4(n: u64, arg0: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    let res;
    core::arch::asm!(
        "svc #0",
        in("x8") n,
        in("x0") arg0,
        in("x1") arg1,
        in("x2") arg2,
        in("x3") arg3,
        lateout("x0") res,
        options(nostack)
    );
    res
}
