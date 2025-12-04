//! ╔═══════════════════════════════════════════════════════════════════════════╗
//! ║                         INTENT KERNEL - MAIN                              ║
//! ║                    The Bridge Between Intent and Silicon                  ║
//! ╚═══════════════════════════════════════════════════════════════════════════╝
//!
//! A capability-based microkernel where humans express intent, not instructions.

#![no_std]
#![no_main]

#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

// Use the library
use intent_kernel::*;

extern crate alloc;
use alloc::sync::Arc;
use core::panic::PanicInfo;

#[cfg(test)]
use intent_kernel::{exit_qemu, QemuExitCode};

// ═══════════════════════════════════════════════════════════════════════════════
// KERNEL ENTRY POINT
// ═══════════════════════════════════════════════════════════════════════════════

/// Main kernel entry - called from boot.s after hardware initialization
#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
    #[cfg(test)]
    test_main();

    // Phase 1: Early initialization (serial for debugging)
    drivers::uart::early_init();
    
    // Banner
    print_banner();
    
    // Phase 2: Core system initialization
    kprintln!("[BOOT] Initializing Intent Kernel...");
    kprintln!();
    
    // Get boot info
    let core_id = arch::core_id();
    let el = arch::exception_level();
    kprintln!("[CPU]  Core {} running at EL{}", core_id, el);
    
    // Initialize timer
    kprintln!("[INIT] System timer...");
    drivers::timer::init();
    let freq = drivers::timer::frequency();
    kprintln!("       Frequency: {} MHz", freq / 1_000_000);
    
    // Initialize interrupt controller
    kprintln!("[INIT] Interrupt controller...");
    drivers::interrupts::init();

    // Initialize RNG and Security
    kprintln!("[INIT] Security Subsystem...");
    drivers::rng::init();
    let seed = drivers::rng::next_u64();
    kprintln!("       RNG Seed: {:#018x}", seed);
    
    // Initialize Pointer Guard
    kernel::capability::init_security(seed);
    
    // Initialize memory subsystem (Polymorphic Heap)
    kprintln!("[INIT] Memory allocator...");
    unsafe { kernel::memory::init(seed); }
    let heap_avail = kernel::memory::heap_available();
    kprintln!("       Heap: {} MB available", heap_avail / (1024 * 1024));
    
    // Initialize PCIe (Root Complex)
    kprintln!("[INIT] PCIe Subsystem...");
    drivers::pcie::init();

    // Initialize RP1 (I/O Controller)
    kprintln!("[INIT] RP1 I/O Controller...");
    drivers::rp1::init();

    // Initialize GPIO (via RP1)
    kprintln!("[INIT] GPIO...");
    drivers::gpio::init();
    
    // Initialize mailbox (GPU communication)
    kprintln!("[INIT] VideoCore mailbox...");
    drivers::mailbox::init();
    
    // Get hardware info via mailbox
    if let Some(info) = drivers::mailbox::get_board_info() {
        kprintln!("       Board: {:08x}, Rev: {:08x}", info.board_model, info.board_revision);
        kprintln!("       Memory: {} MB", info.arm_memory / (1024 * 1024));
    }
    
    // Initialize framebuffer
    kprintln!("[INIT] Framebuffer...");
    if drivers::framebuffer::init(1920, 1080, 32).is_ok() {
        kprintln!("       Display: 1920x1080x32");
        // Initialize console on framebuffer
        drivers::console::init();
        cprintln!("Intent Kernel v0.2 - Framebuffer Console Active");
    } else {
        kprintln!("       Display: Not available (serial only mode)");
    }
    
    // Initialize capability registry  
    kprintln!("[INIT] Capability system...");
    kernel::capability::init();
    
    // Enable interrupts
    kprintln!("[INIT] Enabling interrupts...");
    arch::irq_enable();
    
    // System ready
    kprintln!();
    kprintln!("╔═══════════════════════════════════════════════════════════╗");
    kprintln!("║              INTENT KERNEL READY                          ║");
    kprintln!("║                                                           ║");
    kprintln!("║  Type 'help' for commands, or just say what you want.     ║");
    kprintln!("╚═══════════════════════════════════════════════════════════╝");
    kprintln!();
    
    // Blink LED to show we're alive
    drivers::gpio::activity_led(true);
    drivers::timer::delay_ms(100);
    drivers::gpio::activity_led(false);
    
    // Initialize Semantic Engine
    kprintln!("[INIT] Semantic Intent Engine...");
    intent::init();
    
    // Initialize Input Engine (Steno - fastest path)
    kprintln!("[INIT] Input Engine (Steno Path)...");
    steno::init();

    // Initialize Visual Layer (Semantic GUI)
    kprintln!("[INIT] Visual Projection Layer...");
    visual::init();
    // Register as a wildcard listener (Priority 10 = Normal)
    intent::register_wildcard(visual::create_handler(), "VisualLayer", 10);

    // Demo Neural Memory (HDC Edition)
    kprintln!("[INIT] Neural Memory Demo (HDC)...");
    unsafe {
        use kernel::memory::neural::{NEURAL_ALLOCATOR, bind, hamming_similarity};
        use intent::ConceptID;
        let mut allocator = NEURAL_ALLOCATOR.lock();
        
        // Create Hypervectors (1024-bit)
        // In a real system, these would be generated by a sensory encoder or random projection.
        // Here we simulate them with simple patterns for the demo.
        
        // CAT: Alternating bits
        let hv_cat = [0xAAAAAAAAAAAAAAAA; 16]; 
        
        // DOG: Similar to CAT (flip a few bits)
        let mut hv_dog = hv_cat;
        hv_dog[0] ^= 0x000000000000FFFF; // Flip low 16 bits of first word
        
        // CAR: Orthogonal (Random-ish different pattern)
        let hv_car = [0x5555555555555555; 16];
        
        // Allocate with semantic tags
        allocator.alloc(1024, ConceptID(0xCA7), hv_cat); // CAT
        allocator.alloc(1024, ConceptID(0xD06), hv_dog); // DOG
        allocator.alloc(1024, ConceptID(0xCA12), hv_car); // CAR
        
        kprintln!("       Allocated: CAT, DOG, CAR");
        
        // Query: "Kitten" (Similar to CAT)
        // Let's make a query vector that is CAT with some noise
        let mut hv_kitten = hv_cat;
        hv_kitten[1] ^= 0x00000000000000FF; // Flip 8 bits
        
        kprintln!("       Query: 'Kitten' (Hypervector similar to CAT)");
        if let Some(ptr) = allocator.retrieve_nearest(&hv_kitten) {
            kprintln!("       Result: Found Concept {:#x} (Sim: {:.4})", ptr.id.0, ptr.similarity);
            if ptr.id.0 == 0xCA7 {
                kprintln!("       SUCCESS: Retrieved CAT!");
            }
        }
        
        // Demo Binding: "Cat" + "Action"
        kprintln!("       Demo: Binding (Cognitive Algebra)");
        let hv_action = [0xF0F0F0F0F0F0F0F0; 16]; // "Action" concept
        let hv_running_cat = bind(&hv_cat, &hv_action);
        
        // The bound vector should be dissimilar to both CAT and ACTION (Orthogonal)
        let sim_cat = hamming_similarity(&hv_running_cat, &hv_cat);
        let sim_action = hamming_similarity(&hv_running_cat, &hv_action);
        kprintln!("       Bound 'Running Cat' similarity to Cat: {:.4} (Should be ~0.5)", sim_cat);
        kprintln!("       Bound 'Running Cat' similarity to Action: {:.4} (Should be ~0.5)", sim_action);
        
        // But if we unbind "Action", we should get "Cat" back
        let hv_unbound = bind(&hv_running_cat, &hv_action); // XOR is its own inverse
        let sim_recovery = hamming_similarity(&hv_unbound, &hv_cat);
        kprintln!("       Unbound 'Running Cat' * 'Action' -> Cat Sim: {:.4} (Should be 1.0)", sim_recovery);
    }

    // Initialize Perception Layer (Adaptive Hardware Support)
    kprintln!("[INIT] Perception Cortex...");
    // Initialize Perception Layer (Adaptive Hardware Support)
    kprintln!("[INIT] Perception Cortex...");
    perception::init();
    
    let perception_mgr = perception::PERCEPTION_MANAGER.lock();
    kprintln!("       Active Sensors:");
    for sensor in perception_mgr.sensors() {
        kprintln!("       - {}", sensor.backend_name());
    }

    // Initialize HUD (Legacy - now handled by Visual Layer)
    // kprintln!("[INIT] Heads-Up Display...");
    // perception::hud::init();

    // Initialize Filesystem
    kprintln!("[INIT] Filesystem...");
    fs::init();
    
    // Initialize SD Card
    kprintln!("[INIT] SD Card Driver...");
    drivers::sd::init();
    
    // Mount FAT32 on SD
    kprintln!("[INIT] Mounting FAT32 on SD...");
    {
        // Get SD Driver instance
        // Note: In a real system we'd have a BlockDevice registry.
        // Here we just use the static instance wrapped in an Arc-like adapter or just pass it if we change Fat32 to take a reference?
        // Fat32FileSystem takes Arc<dyn BlockDevice>.
        // We need to implement BlockDevice for Arc<SpinLock<SdCardDriver>> or similar.
        // Or just implement it for the static?
        // Let's create a wrapper struct that implements BlockDevice and calls the static SD_DRIVER.
        
        struct SdWrapper;
        impl fs::vfs::BlockDevice for SdWrapper {
            fn read_sector(&self, sector: u32, buf: &mut [u8]) -> Result<(), &'static str> {
                drivers::sd::SD_DRIVER.lock().read_sector(sector, buf)
            }
            fn write_sector(&self, sector: u32, buf: &[u8]) -> Result<(), &'static str> {
                drivers::sd::SD_DRIVER.lock().write_sector(sector, buf)
            }
        }
        
        let sd_dev = Arc::new(SdWrapper);
        // Wrap in Cache (512 sectors = 256KB cache)
        let cached_dev = Arc::new(fs::cache::CachedDevice::new(sd_dev, 512));
        
        match fs::fat32::Fat32FileSystem::new(cached_dev) {
            Ok(fat32) => {
                let mut vfs = fs::VFS.lock();
                if let Err(e) = vfs.mount("/sd", fat32.clone()) {
                    kprintln!("       Mount failed: {}", e);
                } else {
                    kprintln!("       Mounted FAT32 at /sd");
                    
                    // Test: List Root Directory
                    kprintln!("       Listing /sd:");
                    if let Ok(entries) = vfs.read_dir("/sd") {
                        for entry in entries {
                            kprintln!("       - {} ({})", entry.name, entry.size);
                        }
                    } else {
                        kprintln!("       Failed to read directory");
                    }
                }
            },
            Err(e) => kprintln!("       Failed to initialize FAT32: {}", e),
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════════
    // SYSCALL TEST
    // ═══════════════════════════════════════════════════════════════════════════════
    kprintln!("\n[KERNEL] Spawning Syscall Test Task...");
    kprintln!("\n[KERNEL] Spawning Syscall Test Task...");
    let _ = crate::kernel::scheduler::SCHEDULER.lock().spawn_simple(syscall_test_task);
    

    // Initialize PCIe
    drivers::pcie::init();

    // Initialize USB
    kprintln!("[INIT] USB Subsystem...");
    drivers::usb::init();

    // Initialize Scheduler
    kprintln!("[INIT] Scheduler...");

    // Spawn Async Executor Agent (The "Main" Thread)
    // Spawn Async Executor Agent (The "Main" Thread)
    let _ = kernel::scheduler::SCHEDULER.lock().spawn_simple(async_executor_agent);
    kprintln!("       Spawned Async Executor Agent");

    // Spawn User Task (EL0 Process) from init.elf
    kprintln!("[INIT] Loading /init.elf...");
    {
        // Read init.elf
        let vfs = fs::VFS.lock();
        if let Ok(file) = vfs.open("/sd/init.elf", 0) {
            let mut file = file.lock();
            let size = file.stat().map(|s| s.size).unwrap_or(0);
            if size > 0 {
                // Allocate buffer (using Vec for convenience)
                let mut buf = alloc::vec::Vec::with_capacity(size as usize);
                buf.resize(size as usize, 0);
                
                if let Ok(n) = file.read(&mut buf) {
                    kprintln!("       Read {} bytes", n);
                    
                    // Spawn User Process
                    let mut scheduler = kernel::scheduler::SCHEDULER.lock();
                    match scheduler.spawn_user_elf(&buf) {
                        Ok(_) => kprintln!("       Spawned User Process 1 (init)"),
                        Err(e) => kprintln!("       Failed to spawn user process 1: {}", e),
                    }
                    match scheduler.spawn_user_elf(&buf) {
                        Ok(_) => kprintln!("       Spawned User Process 2 (init)"),
                        Err(e) => kprintln!("       Failed to spawn user process 2: {}", e),
                    }
                } else {
                    kprintln!("       Failed to read /init.elf");
                }
            } else {
                kprintln!("       /init.elf is empty");
            }
        } else {
            kprintln!("       Failed to open /init.elf");
            kprintln!("       Failed to open /init.elf");
            // Fallback to internal test
            let _ = kernel::scheduler::SCHEDULER.lock().spawn_user_simple(user_task, 0);
            kprintln!("       Spawned Fallback User Task (EL0)");
        }
    }

    // Enable Timer Interrupt (10ms)
    kprintln!("[INIT] Enabling Preemption...");
    drivers::timer::set_timer_interrupt(10_000);
    
    // Enable GIC for Timer (PPI 30)
    drivers::interrupts::enable(30);

    // Enable Global Interrupts (DAIF)
    unsafe { arch::enable_interrupts(); }

    kprintln!();
    kprintln!("╔═══════════════════════════════════════════════════════════╗");
    kprintln!("║              INTENT KERNEL READY (USER MODE)              ║");
    kprintln!("╚═══════════════════════════════════════════════════════════╝");
    kprintln!();

    // Main loop (Idle task)
    loop {
        arch::wfi();
    }
}

fn async_executor_agent() {
    kprintln!("[Executor] Starting Steno-Native Async Core...");
    let mut executor = kernel::async_core::Executor::new();
    executor.spawn(steno_loop());
    executor.spawn(usb_loop());
    executor.run();
}

/// USB Input Loop - polls for strokes from steno machine
async fn usb_loop() {
    loop {
        // Poll USB HID driver
        // Note: In a real implementation with interrupts, this would be an awaitable future.
        // For now, we poll and yield.
        if let Some(stroke) = drivers::usb::hid::HID_DRIVER.lock().poll() {
            // Process stroke
            if let Some(intent) = steno::process_stroke(stroke) {
                kprintln!("[USB] Stroke: {:?} -> Intent: {}", stroke, intent.name);
                cprintln!("[USB] Intent: {}", intent.name);
                intent::execute(&intent);
                
                // Update Visual Layer
                visual::handle_stroke(&stroke);
            } else {
                // Unknown stroke
                visual::handle_stroke(&stroke);
            }
        }
        
        // Yield to let other tasks run
        kernel::async_core::yield_now().await;
    }
}

/// Main steno input loop - processes strokes as they arrive
async fn steno_loop() {
    kprintln!();
    kprintln!("╔═══════════════════════════════════════════════════════════╗");
    kprintln!("║           STENO INPUT READY                               ║");
    kprintln!("║                                                           ║");
    kprintln!("║  Input strokes directly. No characters. Pure semantic.    ║");
    kprintln!("║  Example: STPH (for 'sn'), KAT (for 'cat')                ║");
    kprintln!("╚═══════════════════════════════════════════════════════════╝");
    kprintln!();
    
    cprintln!("STENO INPUT READY");
    cprintln!("Type steno strokes (e.g. 'KAT') or English commands (e.g. 'help')");
    
    let mut input_buffer = [0u8; 64];
    
    loop {
        kprint!("steno> ");
        let len = drivers::uart::read_line_async(&mut input_buffer).await;
        if len == 0 { continue; }
        
        let input = core::str::from_utf8(&input_buffer[..len]).unwrap_or("");
        let input = input.trim();
        
        if input.is_empty() { continue; }
        
        cprintln!("> {}", input);
        
        // 1. Try as Steno Notation first
        // We check if it looks like steno (uppercase, valid keys) or just try it.
        // process_steno will return None if it's not valid steno bits (which is always valid u32, but maybe 0)
        // Actually parse_steno_to_bits returns 0 if invalid? No, it parses what it can.
        // Let's try English lookup first if it looks like a word, or Steno if it looks like steno.
        // But "KAT" is both steno notation and a word (maybe).
        // The user said "normal user type in normal english".
        // If I type "help", steno parser might ignore it or parse 'H', 'E', 'L', 'P' if they are keys.
        // 'H' is a key. 'E' is a key. 'L' is a key. 'P' is a key.
        // So "HELP" is a valid steno string "H-PB".
        // Wait, "HELP" in steno notation:
        // H -> H-
        // E -> -E
        // L -> -L
        // P -> P- or -P?
        // The parser is likely RTFCRE or similar.
        
        // Let's try English lookup FIRST for user friendliness.
        if let Some(intent) = steno::process_english(input) {
             kprintln!("[ENGLISH] Mapped '{}' -> Intent '{}'", input, intent.name);
             cprintln!("[INTENT] {}", intent.name);
             intent::execute(&intent);
             
             // Update Visual Layer
             if let Some(stroke) = steno::Stroke::from_steno(input) {
                 visual::handle_stroke(&stroke);
             }
             continue;
        }
        
        // 2. Try as Steno Notation
        if let Some(intent) = steno::process_steno(input) {
            kprintln!("[STENO] Processed: {} -> {}", input, intent.name);
            cprintln!("[INTENT] {}", intent.name);
            intent::execute(&intent);
            
            // Update Visual Layer
            if let Some(stroke) = steno::Stroke::from_steno(input) {
                visual::handle_stroke(&stroke);
            }
            continue;
        }

        kprintln!("[STENO] No match for: {}", input);
        cprintln!("Unknown command or stroke: {}", input);
        
        // Update Visual Layer for unrecognized stroke
        if let Some(stroke) = steno::Stroke::from_steno(input) {
            visual::handle_stroke(&stroke);
        }
    }
}

fn user_task() {
    let msg = "[User] Hello from EL0!\n";
    loop {
        unsafe {
            // Syscall 2: Print
            core::arch::asm!(
                "mov x8, #2",
                "mov x0, {0}",
                "mov x1, {1}",
                "svc #0",
                in(reg) msg.as_ptr(),
                in(reg) msg.len(),
            );
            
            // Syscall 3: Sleep 100ms
            core::arch::asm!(
                "mov x8, #3",
                "mov x0, #100",
                "svc #0",
            );
        }
    }
}

/// Print the boot banner
fn print_banner() {
    kprintln!();
    kprintln!("╔═══════════════════════════════════════════════════════════════════╗");
    kprintln!("║                                                                   ║");
    kprintln!("║   ██╗███╗   ██╗████████╗███████╗███╗   ██╗████████╗               ║");
    kprintln!("║   ██║████╗  ██║╚══██╔══╝██╔════╝████╗  ██║╚══██╔══╝               ║");
    kprintln!("║   ██║██╔██╗ ██║   ██║   █████╗  ██╔██╗ ██║   ██║                  ║");
    kprintln!("║   ██║██║╚██╗██║   ██║   ██╔══╝  ██║╚██╗██║   ██║                  ║");
    kprintln!("║   ██║██║ ╚████║   ██║   ███████╗██║ ╚████║   ██║                  ║");
    kprintln!("║   ╚═╝╚═╝  ╚═══╝   ╚═╝   ╚══════╝╚═╝  ╚═══╝   ╚═╝                  ║");
    kprintln!("║                                                                   ║");
    kprintln!("║   ██╗  ██╗███████╗██████╗ ███╗   ██╗███████╗██╗                   ║");
    kprintln!("║   ██║ ██╔╝██╔════╝██╔══██╗████╗  ██║██╔════╝██║                   ║");
    kprintln!("║   █████╔╝ █████╗  ██████╔╝██╔██╗ ██║█████╗  ██║                   ║");
    kprintln!("║   ██╔═██╗ ██╔══╝  ██╔══██╗██║╚██╗██║██╔══╝  ██║                   ║");
    kprintln!("║   ██║  ██╗███████╗██║  ██║██║ ╚████║███████╗███████╗              ║");
    kprintln!("║   ╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝╚══════╝╚══════╝              ║");
    kprintln!("║                                                                   ║");
    kprintln!("║      A Perceptual Computing Platform               v0.2      ║");
    kprintln!("║                                                                   ║");
    kprintln!("║   Hardware: Raspberry Pi 5 (BCM2712)                              ║");
    kprintln!("║   CPU:      ARM Cortex-A76 x4 @ 2.4GHz                            ║");
    kprintln!("║   RAM:      8GB LPDDR4X                                           ║");
    kprintln!("║   GPU:      VideoCore VII                                         ║");
    kprintln!("║                                                                   ║");
    kprintln!("╚═══════════════════════════════════════════════════════════════════╝");
    kprintln!();
}

// ═══════════════════════════════════════════════════════════════════════════════
// EXCEPTION HANDLERS
// ═══════════════════════════════════════════════════════════════════════════════

// Handlers are now in kernel::exception
// The linker will find the #[no_mangle] symbols there.

// ═══════════════════════════════════════════════════════════════════════════════
// PANIC HANDLER
// ═══════════════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════════════
// TEST RUNNER
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Fn()]) {
    kprintln!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    exit_qemu(QemuExitCode::Success);
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kprintln!("[failed]\n");
    kprintln!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Disable interrupts to prevent further issues
    arch::irq_disable();
    
    kprintln!();
    kprintln!("╔═══════════════════════════════════════════════════════════╗");
    kprintln!("║                    KERNEL PANIC                           ║");
    kprintln!("╚═══════════════════════════════════════════════════════════╝");
    kprintln!();
    
    if let Some(location) = info.location() {
        kprintln!("Location: {}:{}", location.file(), location.line());
    }
    
    let message = info.message();
    kprintln!("PANIC: {}", message);
    kprintln!("Core {}, EL{}", arch::core_id(), arch::exception_level());
    kprintln!("Uptime: {} ms", drivers::timer::uptime_ms());
    kprintln!();
    kprintln!("System halted. Reset to restart.");
    
    // Blink LED rapidly to indicate panic
    loop {
        drivers::gpio::activity_led(true);
        for _ in 0..500_000 { core::hint::spin_loop(); }
        drivers::gpio::activity_led(false);
        for _ in 0..500_000 { core::hint::spin_loop(); }
    }
}

fn syscall_test_task() {
    kprintln!("[TASK] Syscall Test Task Started");
    
    // Create dummy frame for syscalls from kernel mode
    let mut frame: crate::kernel::exception::ExceptionFrame = unsafe { core::mem::zeroed() };
    
    // 1. Print
    let msg = "Hello from Syscall Task!\n";
    crate::kernel::syscall::dispatcher(
        crate::kernel::syscall::SyscallNumber::Print as u64,
        msg.as_ptr() as u64,
        msg.len() as u64,
        0,
        &mut frame
    );
    
    // 2. Open
    let path = "config.txt\0";
    let fd = crate::kernel::syscall::dispatcher(
        crate::kernel::syscall::SyscallNumber::Open as u64,
        path.as_ptr() as u64,
        0,
        0,
        &mut frame
    );
    
    if fd != u64::MAX {
        kprintln!("[TASK] Open Success! FD={}", fd);
        
        let mut buf = [0u8; 32];
        let read_len = crate::kernel::syscall::dispatcher(
            crate::kernel::syscall::SyscallNumber::Read as u64,
            fd,
            buf.as_mut_ptr() as u64,
            32,
            &mut frame
        );
        
        if read_len != u64::MAX {
             kprintln!("[TASK] Read Success! Len={}", read_len);
             if let Ok(s) = core::str::from_utf8(&buf[0..read_len as usize]) {
                 kprintln!("[TASK] Content: {:?}", s);
             }
        }
        
        crate::kernel::syscall::dispatcher(
            crate::kernel::syscall::SyscallNumber::Close as u64,
            fd, 0, 0,
            &mut frame
        );
    } else {
        kprintln!("[TASK] Open Failed");
    }
    
    // 3. Test Exit
    kprintln!("[TASK] Exiting...");
    crate::kernel::syscall::dispatcher(
        crate::kernel::syscall::SyscallNumber::Exit as u64,
        0, 0, 0,
        &mut frame
    );
    
    // Should not reach here
    kprintln!("[TASK] ERROR: Continued after exit!");
    loop {
        crate::kernel::syscall::dispatcher(
            crate::kernel::syscall::SyscallNumber::Yield as u64,
            0, 0, 0,
            &mut frame
        );
    }
}
