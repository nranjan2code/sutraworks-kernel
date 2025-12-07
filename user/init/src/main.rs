#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::arch::asm;

const SYS_EXIT: u64 = 0;
const SYS_READ: u64 = 1;
const SYS_PRINT: u64 = 2;
const SYS_PARSE_INTENT: u64 = 22;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Banner using string literal (tests .rodata)
    print("\n╔════════════════════════════════════╗\n");
    print("║   Intent Kernel Shell v0.3        ║\n");
    print("║   Type 'help' for commands        ║\n");
    print("╚════════════════════════════════════╝\n\n");

    // ═══════════════════════════════════════════════════════════════════════════════
    // AUTO-TEST: Prove Intent-native architecture works on boot
    // This executes 'help' via SYS_PARSE_INTENT without requiring UART input
    // ═══════════════════════════════════════════════════════════════════════════════
    print("[AUTOTEST] Executing 'help' via SYS_PARSE_INTENT (22)...\n");
    let help_cmd = b"help";
    let res = unsafe { syscall(22, help_cmd.as_ptr() as u64, help_cmd.len() as u64, 0, 0) };
    
    if res == 0 {
         print("[AUTOTEST] SUCCESS: Intent flow verified!\n");
    } else {
         print("[AUTOTEST] FAILED to verify intent flow.\n");
    }
    
    // AUTO-TEST: Directory Listing
    print("[AUTOTEST] Listing root directory via Intent (ls)...\n");
    let ls_cmd = b"ls";
    let ls_res = unsafe { syscall(22, ls_cmd.as_ptr() as u64, ls_cmd.len() as u64, 0, 0) };
    
    if ls_res == 0 {
         print("[AUTOTEST] Intent 'ls' dispatched successfully.\n");
    } else {
         print("[AUTOTEST] 'ls' failed.\n");
    }

    let mut shell = Shell::new();
    shell.run();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    print("PANIC!\n");
    unsafe { syscall(SYS_EXIT, 1, 0, 0, 0); }
    loop { unsafe { asm!("wfi"); } }
}

// ══════════════════════════════════════════════════════════════════════════════
// SHELL
// ══════════════════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════════════════
// SHELL
// ══════════════════════════════════════════════════════════════════════════════

struct Shell {
    cwd: [u8; 64],
    cwd_len: usize,
}

#[repr(C, packed)]
struct LinuxDirent64 {
    d_ino: u64,
    d_off: i64,
    d_reclen: u16,
    d_type: u8,
    d_name: [u8; 0],
}

impl Shell {
    fn new() -> Self {
        let mut s = Shell { cwd: [0u8; 64], cwd_len: 1 };
        s.cwd[0] = b'/';
        s
    }
    
    fn run(&mut self) -> ! {
        let mut line_buf = [0u8; 128];
        loop {
            // Print prompt: "/> "
            self.print_cwd();
            print("> ");
            
            // Read line
            let len = self.read_line(&mut line_buf);
            
            if len > 0 {
                self.execute(&line_buf[..len]);
            }
        }
    }
    
    fn print_cwd(&self) {
        unsafe { syscall(SYS_PRINT, self.cwd.as_ptr() as u64, self.cwd_len as u64, 0, 0); }
    }
    
    fn read_line(&self, buf: &mut [u8]) -> usize {
        let mut idx = 0;
        loop {
            if idx >= buf.len() { break; }
            
            // Read 1 byte from Stdin (FD 0)
            let mut c_buf = [0u8; 1];
            let n = unsafe { syscall(SYS_READ, 0, c_buf.as_mut_ptr() as u64, 1, 0) };
            
            if n > 0 {
                let c = c_buf[0];
                if c == b'\n' || c == b'\r' {
                    print("\n");
                    break;
                }
                
                // Echo
                unsafe { syscall(SYS_PRINT, c_buf.as_ptr() as u64, 1, 0, 0); }
                
                buf[idx] = c;
                idx += 1;
            } else {
               unsafe { asm!("wfi"); } 
            }
        }
        idx
    }
    
    fn execute(&mut self, line: &[u8]) {
        // Guard: Skip if line is empty or all whitespace/zeros
        if line.is_empty() {
            return;
        }
        
        // Check if line has any printable ASCII
        let has_printable = line.iter().any(|&b| b > 32 && b < 127);
        if !has_printable {
            return; // Skip garbage input
        }
        
        let cmd_str = core::str::from_utf8(line).unwrap_or("");
        let mut parts = cmd_str.split_whitespace();
        let cmd = parts.next().unwrap_or("");
        
        
        // Intent-Native: Route to kernel's Intent system via SYS_PARSE_INTENT
        // The kernel will:
        // 1. Parse using EnglishParser
        // 2. Create Intent with ConceptID
        // 3. Broadcast to handlers (1:N)
        // 4. Execute via IntentExecutor
        
        const SYS_PARSE_INTENT_ID: u64 = 22; // Renamed to avoid const conflict
        
        let result = unsafe {
            syscall(SYS_PARSE_INTENT_ID, line.as_ptr() as u64, line.len() as u64, 0, 0)
        };
        
        // Result: 0 = Intent executed, 1 = Unknown command, MAX = Error
        if result == 1 {
            print("Unknown command. Type 'help' for available commands.\n");
        } else if result == u64::MAX {
            print("Error processing command.\n");
        }
        // result == 0: Intent executed successfully (kernel handled output)
    }
    
}

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
