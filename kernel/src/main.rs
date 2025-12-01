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
    
    // Spawn Task A (Kernel Thread)
    kernel::scheduler::SCHEDULER.lock().spawn(task_a);
    kprintln!("       Spawned Task A (Kernel)");

    // Spawn User Task (EL0 Process)
    kernel::scheduler::SCHEDULER.lock().spawn_user(user_task, 0);
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

fn task_a() {
    loop {
        crate::kprintln!("[Kernel] Working...");
        for _ in 0..50_000_000 { core::hint::spin_loop(); }
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
