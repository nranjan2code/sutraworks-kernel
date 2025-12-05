//! Integration Tests for Intent Kernel
//!
//! These tests verify the interaction between different kernel components.

#![no_std]
#![no_main]

extern crate alloc;

use core::panic::PanicInfo;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::collections::BTreeMap;

use intent_kernel::kernel::sync::SpinLock;
use intent_kernel::fs::vfs::{Filesystem, FileOps, FileStat, DirEntry, SeekFrom, VFS};
use intent_kernel::net::interface::{NetworkInterface, LoopbackInterface};
use intent_kernel::kernel::process::{Agent, AgentState};

// ═══════════════════════════════════════════════════════════════════════════════
// STARTUP CODE
// ═══════════════════════════════════════════════════════════════════════════════

core::arch::global_asm!(
    ".section .bss",
    ".align 16",
    "stack_bottom:",
    ".space 32768", // 32KB stack
    "stack_top:",
    ".text",
    ".global _start",
    "_start:",
    // Enable FP/SIMD (CPACR_EL1.FPEN = 0b11)
    "mrs x0, cpacr_el1",
    "orr x0, x0, #(3 << 20)",
    "msr cpacr_el1, x0",
    "isb",

    // Zero BSS
    "ldr x0, =__bss_start",
    "ldr x1, =__bss_end",
    "sub x2, x1, x0",
    "cbz x2, 2f",
    "1:",
    "str xzr, [x0], #8",
    "sub x2, x2, #8",
    "cbnz x2, 1b",
    "2:",
    
    "ldr x0, =stack_top",
    "mov sp, x0",
    "bl kernel_main",
    "b ."
);

// ═══════════════════════════════════════════════════════════════════════════════
// MISSING SYMBOLS (Stubs for Testing)
// ═══════════════════════════════════════════════════════════════════════════════

core::arch::global_asm!(
    // Memory Layout Symbols
    ".global __heap_start",
    ".global __heap_end",
    ".global __dtb_ptr",
    ".global __dma_start",
    ".global __dma_end",
    ".section .data",
    ".align 12",
    "__dtb_ptr:",
    ".quad 0",
    ".align 12",
    "__heap_start:",
    ".space 1024 * 1024 * 8", // 8MB Heap
    "__heap_end:",
    ".align 12",
    "__dma_start:",
    ".space 1024 * 1024 * 1", // 1MB DMA
    "__dma_end:",             // DMA region ends where heap starts (simplified)
    
    ".text",
    
    // Interrupts
    ".global enable_interrupts",
    "enable_interrupts:",
    "msr daifclr, #2",
    "ret",

    ".global disable_interrupts",
    "disable_interrupts:",
    "mrs x0, daif",
    "msr daifset, #2",
    "ret",

    ".global restore_interrupts",
    "restore_interrupts:",
    "msr daif, x0",
    "ret",
    
    ".global wait_for_interrupt",
    "wait_for_interrupt:",
    "wfi",
    "ret",

    // Barriers
    ".global data_sync_barrier",
    "data_sync_barrier:",
    "dsb sy",
    "ret",

    ".global instruction_barrier",
    "instruction_barrier:",
    "isb",
    "ret",
    
    ".global memory_barrier",
    "memory_barrier:",
    "dmb sy",
    "ret",

    // Timer
    ".global read_timer",
    "read_timer:",
    "mrs x0, cntpct_el0",
    "ret",

    ".global read_timer_freq",
    "read_timer_freq:",
    "mrs x0, cntfrq_el0",
    "ret",
    
    // Multicore stubs (if needed)
    ".global get_core_id",
    "get_core_id:",
    "mrs x0, mpidr_el1",
    "and x0, x0, #3",
    "ret",
    
    ".global get_exception_level",
    "get_exception_level:",
    "mrs x0, CurrentEL",
    "lsr x0, x0, #2",
    "ret"
);

// ═══════════════════════════════════════════════════════════════════════════════
// TEST INFRASTRUCTURE
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

fn run_test<F: Fn()>(name: &str, test: F) {
    serial_print!("{}...\t", name);
    test();
    serial_println!("[ok]");
}

// ═══════════════════════════════════════════════════════════════════════════════
// MOCK FILESYSTEM (RamFS)
// ═══════════════════════════════════════════════════════════════════════════════

struct RamFile {
    #[allow(dead_code)]
    name: String,
    data: Vec<u8>,
}

struct RamFsInner {
    files: BTreeMap<String, Arc<SpinLock<RamFile>>>,
}

struct RamFs {
    inner: SpinLock<RamFsInner>,
}

impl RamFs {
    fn new() -> Self {
        Self {
            inner: SpinLock::new(RamFsInner {
                files: BTreeMap::new(),
            }),
        }
    }
}

struct RamFileHandle {
    file: Arc<SpinLock<RamFile>>,
    pos: u64,
}

impl FileOps for RamFileHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, &'static str> {
        let file = self.file.lock();
        if self.pos >= file.data.len() as u64 {
            return Ok(0);
        }
        
        let available = file.data.len() as u64 - self.pos;
        let read_len = core::cmp::min(available as usize, buf.len());
        
        buf[0..read_len].copy_from_slice(&file.data[self.pos as usize..(self.pos as usize + read_len)]);
        self.pos += read_len as u64;
        
        Ok(read_len)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, &'static str> {
        let mut file = self.file.lock();
        
        // Simple append/overwrite logic
        let end_pos = self.pos as usize + buf.len();
        if end_pos > file.data.len() {
            file.data.resize(end_pos, 0);
        }
        
        file.data[self.pos as usize..end_pos].copy_from_slice(buf);
        self.pos += buf.len() as u64;
        
        Ok(buf.len())
    }

    fn seek(&mut self, pos: SeekFrom) -> Result<u64, &'static str> {
        let file = self.file.lock();
        let len = file.data.len() as u64;
        
        let new_pos = match pos {
            SeekFrom::Start(off) => off as i64,
            SeekFrom::Current(off) => self.pos as i64 + off,
            SeekFrom::End(off) => len as i64 + off,
        };
        
        if new_pos < 0 {
            return Err("Invalid seek");
        }
        
        self.pos = new_pos as u64;
        Ok(self.pos)
    }

    fn close(&mut self) -> Result<(), &'static str> {
        Ok(())
    }

    fn stat(&self) -> Result<FileStat, &'static str> {
        let file = self.file.lock();
        Ok(FileStat {
            size: file.data.len() as u64,
            mode: 0o644,
            inode: 0,
        })
    }
    
    fn as_any(&mut self) -> &mut dyn core::any::Any {
        self
    }
}

impl Filesystem for RamFs {
    fn open(&self, path: &str, _flags: usize) -> Result<Arc<SpinLock<dyn FileOps>>, &'static str> {
        let inner = self.inner.lock();
        if let Some(file) = inner.files.get(path) {
            Ok(Arc::new(SpinLock::new(RamFileHandle {
                file: file.clone(),
                pos: 0,
            })))
        } else {
            Err("File not found")
        }
    }

    fn create(&self, path: &str) -> Result<Arc<SpinLock<dyn FileOps>>, &'static str> {
        let mut inner = self.inner.lock();
        let file = Arc::new(SpinLock::new(RamFile {
            name: String::from(path),
            data: Vec::new(),
        }));
        
        inner.files.insert(String::from(path), file.clone());
        
        Ok(Arc::new(SpinLock::new(RamFileHandle {
            file,
            pos: 0,
        })))
    }

    fn mkdir(&self, _path: &str) -> Result<(), &'static str> {
        Ok(())
    }

    fn remove(&self, path: &str) -> Result<(), &'static str> {
        let mut inner = self.inner.lock();
        inner.files.remove(path);
        Ok(())
    }

    fn read_dir(&self, _path: &str) -> Result<Vec<DirEntry>, &'static str> {
        let inner = self.inner.lock();
        let mut entries = Vec::new();
        for (name, file) in &inner.files {
            entries.push(DirEntry {
                name: name.clone(),
                is_dir: false,
                size: file.lock().data.len() as u64,
            });
        }
        Ok(entries)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

fn test_filesystem_lifecycle() {
    // 1. Initialize VFS (already done in init_for_tests, but we mount our own FS)
    let ramfs = Arc::new(RamFs::new());
    VFS.lock().mount("/mnt/ram", ramfs).expect("Failed to mount RamFS");
    
    // 2. Create File
    let file = VFS.lock().create("/mnt/ram/test.txt").expect("Failed to create file");
    
    // 3. Write Data
    let data = b"Hello, Integration World!";
    {
        let mut f = file.lock();
        let written = f.write(data).expect("Failed to write");
        assert_eq!(written, data.len());
    } // Unlock
    
    // 4. Close (implicit drop, but we can verify data persists)
    drop(file);
    
    // 5. Open File
    let file = VFS.lock().open("/mnt/ram/test.txt", 0).expect("Failed to open file");
    
    // 6. Read Data
    let mut buf = [0u8; 64];
    {
        let mut f = file.lock();
        let read = f.read(&mut buf).expect("Failed to read");
        assert_eq!(read, data.len());
        assert_eq!(&buf[0..read], data);
    }
}

fn test_network_loopback() {
    let mut loopback = LoopbackInterface::new();
    let packet = b"Ping Payload";
    
    // 1. Send
    loopback.send(packet).expect("Failed to send");
    
    // 2. Receive
    let received = loopback.receive().expect("Failed to receive");
    
    // 3. Verify
    assert_eq!(received.as_slice(), packet);
}

fn test_process_lifecycle() {
    // 1. Create Kernel Agent
    fn dummy_entry() {}
    let agent = Agent::new_kernel_simple(dummy_entry).expect("Failed to create agent");
    
    // 2. Verify Initial State
    assert_eq!(agent.state, AgentState::Ready);
    assert!(agent.id.0 > 0);
    
    // 3. Verify Stack Allocation
    assert!(agent.kernel_stack.top > agent.kernel_stack.bottom);
    
    // 4. Verify Context
    assert_eq!(agent.context.lr, dummy_entry as *const () as u64);
}

fn test_stress_memory() {
    // Allocate many small vectors to stress the allocator
    let mut vecs = Vec::new();
    for i in 0..1000 {
        let mut v = Vec::new();
        v.push(i);
        vecs.push(v);
    }
    
    // Verify
    for (i, v) in vecs.iter().enumerate() {
        assert_eq!(v[0], i);
    }
    
    // Free (drop)
    drop(vecs);
}

// ═══════════════════════════════════════════════════════════════════════════════
// ENTRY POINT
// ═══════════════════════════════════════════════════════════════════════════════

#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    // Initialize kernel subsystems
    intent_kernel::init_for_tests();
    
    serial_println!("[INTEGRATION] Starting Integration Tests...");
    
    run_test("integration::filesystem_lifecycle", test_filesystem_lifecycle);
    run_test("integration::network_loopback", test_network_loopback);
    run_test("integration::process_lifecycle", test_process_lifecycle);
    run_test("integration::stress_memory", test_stress_memory);
    
    intent_kernel::exit_qemu(intent_kernel::QemuExitCode::Success);
    
    loop {
        intent_kernel::arch::wfi();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("\n[FAILED]\n");
    serial_println!("Error: {}", info);
    intent_kernel::exit_qemu(intent_kernel::QemuExitCode::Failed);
    loop {
        intent_kernel::arch::wfi();
    }
}
