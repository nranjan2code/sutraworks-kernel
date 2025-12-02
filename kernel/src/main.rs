//! ╔═══════════════════════════════════════════════════════════════════════════╗
//! ║                         INTENT KERNEL - MAIN                              ║
//! ║                    The Bridge Between Intent and Silicon                  ║
//! ╚═══════════════════════════════════════════════════════════════════════════╝
//!
//! A capability-based microkernel where humans express intent, not instructions.

#![no_std]
#![no_main]

// Use the library
use intent_kernel::*;

use core::panic::PanicInfo;

// ═══════════════════════════════════════════════════════════════════════════════
// KERNEL ENTRY POINT
// ═══════════════════════════════════════════════════════════════════════════════

/// Main kernel entry - called from boot.s after hardware initialization
#[no_mangle]
pub extern "C" fn kernel_main() -> ! {
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
    
    // Initialize GPIO
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
    
    // Initialize Stenographic Input Engine
    kprintln!("[INIT] Stenographic Input Engine...");
    steno::init();
    kprintln!("       23 keys. 150 years of compression. Now in silicon.");

    // Initialize Perception Layer (Adaptive Hardware Support)
    kprintln!("[INIT] Perception Cortex...");
    let perception_mgr = perception::PerceptionManager::new();
    match perception_mgr.backend_type() {
        perception::BackendType::HailoHardware => {
            kprintln!("       Backend: Hailo-8 AI Accelerator (26 TOPS)");
        },
        perception::BackendType::CpuFallback => {
            kprintln!("       Backend: CPU Fallback (No Accelerator Found)");
        }
    }

    // Initialize HUD
    kprintln!("[INIT] Heads-Up Display...");
    perception::hud::init();

    // Initialize Filesystem (RamDisk + Overlay)
    kprintln!("[INIT] Filesystem (RamDisk + Overlay)...");
    unsafe { fs::init(); }
    if let Some(_fs) = fs::get().as_ref() {
        kprintln!("       Mounted at {:#010x}", drivers::ramdisk::RAMDISK_BASE);
    } else {
        kprintln!("       Failed to mount filesystem");
    }

    // Initialize PCIe
    drivers::pcie::init();

    // Initialize Scheduler
    kprintln!("[INIT] Scheduler...");

    // Spawn Async Executor Agent (The "Main" Thread)
    kernel::scheduler::SCHEDULER.lock().spawn_simple(async_executor_agent);
    kprintln!("       Spawned Async Executor Agent");

    // Spawn User Task (EL0 Process)
    kernel::scheduler::SCHEDULER.lock().spawn_user_simple(user_task, 0);
    kprintln!("       Spawned User Task (EL0)");

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
    executor.run();
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
             
             // Update HUD
             if let Some(stroke) = steno::Stroke::from_steno(input) {
                 perception::hud::update(stroke, Some(&intent));
             }
             continue;
        }
        
        // 2. Try as Steno Notation
        if let Some(intent) = steno::process_steno(input) {
            kprintln!("[STENO] Processed: {} -> {}", input, intent.name);
            cprintln!("[INTENT] {}", intent.name);
            intent::execute(&intent);
            
            // Update HUD
            if let Some(stroke) = steno::Stroke::from_steno(input) {
                perception::hud::update(stroke, Some(&intent));
            }
            continue;
        }

        kprintln!("[STENO] No match for: {}", input);
        cprintln!("Unknown command or stroke: {}", input);
        
        // Update HUD for unrecognized stroke
        if let Some(stroke) = steno::Stroke::from_steno(input) {
            perception::hud::update(stroke, None);
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
    kprintln!("║        The Bridge Between Human Intent and Silicon     v0.2      ║");
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
