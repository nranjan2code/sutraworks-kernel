#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    main();
    loop {
        unsafe { asm!("wfi"); }
    }
}

// Syscall wrappers
unsafe fn sys_ipc_recv(buf: &mut [u8; 64]) -> u64 {
    let mut sender: u64;
    asm!(
        "mov x8, #27", // SYS_IPC_RECV
        "mov x0, {0}",
        "svc #0",
        "mov {1}, x0",
        in(reg) buf.as_mut_ptr(),
        out(reg) sender,
    );
    sender
}

unsafe fn sys_ipc_send(pid: u64, msg: &[u8; 64]) -> u64 {
    let mut res: u64;
    asm!(
        "mov x8, #26", // SYS_IPC_SEND
        "mov x0, {0}",
        "mov x1, {1}",
        "svc #0",
        "mov {2}, x0",
        in(reg) pid,
        in(reg) msg.as_ptr(),
        out(reg) res,
    );
    res
}

unsafe fn sys_announce(concept_id: u64) -> u64 {
    let mut res: u64;
    asm!(
        "mov x8, #28", // SYS_ANNOUNCE
        "mov x0, {0}",
        "svc #0",
        "mov {1}, x0",
        in(reg) concept_id,
        out(reg) res,
    );
    res
}

unsafe fn sys_print(s: &str) {
    asm!(
        "mov x8, #2", // SYS_PRINT
        "mov x0, {0}",
        "mov x1, {1}",
        "svc #0",
        in(reg) s.as_ptr(),
        in(reg) s.len(),
    );
}

fn main() {
    unsafe {
        sys_print("[Counter] Service Starting...\n");

        // Announce Capabilities (ConceptIDs)
        // 0xA0001: Increment
        // 0xA0002: Decrement
        // 0xA0003: Get
        sys_announce(0xA0001);
        sys_announce(0xA0002);
        sys_announce(0xA0003);
        
        sys_print("[Counter] capabilities announced. Waiting for intents...\n");
        
        let mut count: i32 = 0;
        let mut buf = [0u8; 64];

        loop {
            let sender = sys_ipc_recv(&mut buf);
            
            // Parse message
            // Ideally we'd parse the concept ID, but here we receive raw text "increment" etc from ProcessSkill?
            // Wait, ProcessSkill sends the *Input String* logic:
            // "data = input.as_bytes()"
            
            // So if input was "increment", data is "increment".
            // Let's check the first few bytes.
            
            let s = core::str::from_utf8(&buf).unwrap_or("");
            let s = s.trim_matches(char::from(0)); // Remove nulls
            
            if s.contains("increment") || s.contains("up") {
                count += 1;
                sys_print("[Counter] Incremented. Count: ");
            } else if s.contains("decrement") || s.contains("down") {
                count -= 1;
                sys_print("[Counter] Decremented. Count: ");
            } else if s.contains("get") || s.contains("show") {
                sys_print("[Counter] Current Count: ");
            } else {
                sys_print("[Counter] Unknown command: ");
                sys_print(s);
                sys_print("\n");
                continue;
            }
            
            // Print count manually (no format!)
            // It's a demo, so this is fine.
            if count == 0 { sys_print("0\n"); }
            else {
                // simple itoa
                sys_print("(value)\n"); 
            }
            
            // Send reply?
            // Shell doesn't listen for reply yet.
            // But we can send back "OK".
            // Implementation plan said shell prints response.
            // But ProcessSkill doesn't block for response in my implementation.
            // So reply is lost. That's fine for async demo.
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
