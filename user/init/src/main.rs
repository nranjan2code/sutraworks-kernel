#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::arch::asm;

const SYS_EXIT: u64 = 0;
const SYS_YIELD: u64 = 1;
const SYS_PRINT: u64 = 2;
const SYS_READ: u64 = 6;
const SYS_PARSE_INTENT: u64 = 22;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Banner using string literal (tests .rodata)
    print("\n╔════════════════════════════════════╗\n");
    print("║   Intent Kernel Shell v0.3        ║\n");
    print("║   Type 'help' for commands        ║\n");
    print("╚════════════════════════════════════╝\n\n");
    
    print("[INIT] Starting Shell...\n");
    // Flattened Shell logic using MaybeUninit to prevent bad memset generation
    let mut cwd_mem = core::mem::MaybeUninit::<[u8; 64]>::uninit();
    let cwd = unsafe {
        let ptr = cwd_mem.as_mut_ptr() as *mut u8;
        for i in 0..64 {
            *ptr.add(i) = 0;
        }
        *ptr = b'/';
        cwd_mem.assume_init()
    };
    let mut cwd_len = 1;
    
    let mut line_buf_mem = core::mem::MaybeUninit::<[u8; 128]>::uninit();
    let mut line_buf = unsafe { line_buf_mem.assume_init() };
    
    print("[INIT] Shell Loop...\n");

    loop {
        // Print prompt: "/> "
        unsafe { syscall(SYS_PRINT, cwd.as_ptr() as u64, cwd_len as u64, 0, 0); }
        print("> ");
        
        // Read line
        let mut idx = 0;
        loop {
            if idx >= 128 { break; }
            
            let mut c_buf = [0u8; 1];
            // Attempt read
            let n = unsafe { syscall(SYS_READ, 0, c_buf.as_mut_ptr() as u64, 1, 0) };
            
            // Check for success ( > 0 and NOT error code)
            // Error is u64::MAX.
            if n != u64::MAX && n > 0 {
                let c = c_buf[0];
                if c == b'\n' || c == b'\r' {
                    print("\n");
                    break;
                }
                
                // Echo
                unsafe { syscall(SYS_PRINT, c_buf.as_ptr() as u64, 1, 0, 0); }
                
                line_buf[idx] = c;
                idx += 1;
            } else {
               // Yield or Wait
               // If error (MAX), it likely means no FD 0.
               // We should probably panic or print error once, but for now just wait.
               // unsafe { syscall(SYS_YIELD, 0, 0, 0, 0); }
               unsafe { asm!("wfi"); } 
            }
        }
        
        // Execute
        let len = idx;
        if len > 0 {
             let line = &line_buf[..len];
             
             // Check if printable
             let has_printable = line.iter().any(|&b| b > 32 && b < 127);
             if has_printable {
                 const SYS_PARSE_INTENT_ID: u64 = 22; 
                 let result = unsafe {
                    syscall(SYS_PARSE_INTENT_ID, line.as_ptr() as u64, line.len() as u64, 0, 0)
                 };
                 
                 if result == 1 {
                    print("Unknown command. Type 'help' for available commands.\n");
                 } else if result == u64::MAX {
                    print("Error processing command.\n");
                 }
             }
        }
    }
}

// Duplicate panic handler removed

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    print("PANIC!\n");
    unsafe { syscall(SYS_EXIT, 1, 0, 0, 0); }
    loop { unsafe { asm!("wfi"); } }
}

// ══════════════════════════════════════════════════════════════════════════════
// MEMORY BUILTINS (Fixes compiler generating calls to missing symbols)
// ══════════════════════════════════════════════════════════════════════════════

#[no_mangle]
pub unsafe extern "C" fn memset(dest: *mut u8, c: i32, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        *dest.add(i) = c as u8;
        i += 1;
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        *dest.add(i) = *src.add(i);
        i += 1;
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if src < dest as *const u8 {
        // Copy backwards
        let mut i = n;
        while i > 0 {
            i -= 1;
            *dest.add(i) = *src.add(i);
        }
    } else {
        // Copy forwards
        let mut i = 0;
        while i < n {
            *dest.add(i) = *src.add(i);
            i += 1;
        }
    }
    dest
}

// ══════════════════════════════════════════════════════════════════════════════
// End of file cleanup

// ══════════════════════════════════════════════════════════════════════════════
// HELPERS
// ══════════════════════════════════════════════════════════════════════════════

fn print(s: &str) {
    unsafe { syscall(SYS_PRINT, s.as_ptr() as u64, s.len() as u64, 0, 0); }
}

fn print_num(n: u64) {
    // Simple decimal printer
    if n == 0 {
        print("ZERO");
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = 19;
    let mut val = n;
    while val > 0 {
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
        if i > 0 { i -= 1; }
    }
    if let Ok(s) = core::str::from_utf8(&buf[i+1..]) {
        print(s);
    }
}

fn eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    for i in 0..a.len() {
        if a[i] != b[i] { return false; }
    }
    true
}

unsafe fn syscall(id: u64, arg1: u64, arg2: u64, arg3: u64, arg4: u64) -> u64 {
    let ret: u64;
    asm!(
        "svc #0",
        in("x8") id,
        in("x0") arg1,
        in("x1") arg2,
        in("x2") arg3,
        in("x3") arg4,
        lateout("x0") ret,
    );
    ret
}
