#![no_std]
#![no_main]

use core::panic::PanicInfo;

// Syscall Numbers
const SYS_EXIT: u64 = 0;
const SYS_YIELD: u64 = 1;
const SYS_PRINT: u64 = 2;
const SYS_SLEEP: u64 = 3;

#[no_mangle]
#[link_section = ".text._start"]
pub extern "C" fn _start() -> ! {
    print("Hello from User Mode!\n");
    
    let mut counter = 0;
    loop {
        print("User Tick: ");
        print_num(counter);
        print("\n");
        
        sleep(1000);
        counter += 1;
        
        if counter > 5 {
            print("Exiting...\n");
            exit(0);
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    print("User Panic!\n");
    exit(1);
}

// Syscall Wrappers

fn syscall(num: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    let ret: u64;
    unsafe {
        core::arch::asm!(
            "svc #0",
            in("x0") num,
            in("x1") arg0,
            in("x2") arg1,
            in("x3") arg2,
            lateout("x0") ret,
        );
    }
    ret
}

fn print(s: &str) {
    syscall(SYS_PRINT, s.as_ptr() as u64, s.len() as u64, 0);
}

fn sleep(ms: u64) {
    syscall(SYS_SLEEP, ms, 0, 0);
}

fn exit(code: i32) -> ! {
    syscall(SYS_EXIT, code as u64, 0, 0);
    loop {}
}

fn print_num(mut n: u64) {
    if n == 0 {
        print("0");
        return;
    }
    
    let mut buf = [0u8; 20];
    let mut i = 20;
    
    while n > 0 {
        i -= 1;
        buf[i] = (n % 10) as u8 + b'0';
        n /= 10;
    }
    
    let s = core::str::from_utf8(&buf[i..]).unwrap();
    print(s);
}
