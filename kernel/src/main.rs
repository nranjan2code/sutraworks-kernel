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

    kprintln!("Kernel Entry");

    // Phase 1: Early initialization (serial for debugging)
    drivers::uart::early_init();
    
    // Banner
    print_banner();

    
    // Phase 2: Core system initialization
    kprintln!("[BOOT] Initializing Intent Kernel...");
    kprintln!();
    
    unsafe {
        extern "C" {
            static __bss_start: u8;
            static __bss_end: u8;
        }
        let bss_start = &__bss_start as *const u8 as usize;
        let bss_end = &__bss_end as *const u8 as usize;
        kprintln!("[DEBUG] BSS: {:#x} - {:#x} (Size: {})", bss_start, bss_end, bss_end - bss_start);
        
        let neural_addr = &kernel::memory::neural::NEURAL_ALLOCATOR as *const _ as usize;
        kprintln!("[DEBUG] NEURAL_ALLOCATOR: {:#x}", neural_addr);
        
        // Check first few words of BSS
        let ptr = bss_start as *const u64;
        kprintln!("[DEBUG] BSS[0]: {:#x}", *ptr);
        kprintln!("[DEBUG] BSS[1]: {:#x}", *ptr.add(1));
    }
    
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
    
    // Detect Machine Type
    let machine = dtb::machine_type();
    kprintln!("[INIT] Machine Detection: {:?}", machine);

    // Initialize PCIe (Root Complex)
    if machine == dtb::MachineType::RaspberryPi5 {
        kprintln!("[INIT] PCIe Subsystem...");
        drivers::pcie::init();
    }

    // Initialize RP1 (I/O Controller)
    if machine == dtb::MachineType::RaspberryPi5 {
        kprintln!("[INIT] RP1 I/O Controller...");
        drivers::rp1::init();
    }

    // Initialize GPIO (via RP1)
    kprintln!("[INIT] GPIO...");
    if machine == dtb::MachineType::RaspberryPi5 {
        drivers::gpio::init();
    }

    // Initialize Ethernet
    kprintln!("[INIT] Ethernet...");
    // Use a default MAC (VirtIO will detect its own, RP1 needs one)
    let mac = drivers::ethernet::MacAddr::from_bytes([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
    drivers::ethernet::init(mac);
    
    // Initialize mailbox (GPU communication)
    if machine == dtb::MachineType::RaspberryPi5 {
        kprintln!("[INIT] VideoCore mailbox...");
        drivers::mailbox::init();
        
        // Get hardware info via mailbox
        if let Some(info) = drivers::mailbox::get_board_info() {
            kprintln!("       Board: {:08x}, Rev: {:08x}", info.board_model, info.board_revision);
            kprintln!("       Memory: {} MB", info.arm_memory / (1024 * 1024));
        }
    }
    
    // Initialize framebuffer
    if machine == dtb::MachineType::RaspberryPi5 {
        kprintln!("[INIT] Framebuffer...");
        // drivers::framebuffer::init(); // Uncomment when ready
    }
    
    // Try to init framebuffer (works on QEMU if ramfb is supported, but here we check machine)
    // Actually, QEMU virt might have a framebuffer if configured, but let's stick to Pi 5 logic for now.
    if machine == dtb::MachineType::RaspberryPi5 && drivers::framebuffer::init(1920, 1080, 32).is_ok() {
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
    
    // Initialize Semantic Intent Engine
    kprintln!("[INIT] Semantic Intent Engine...");
    intent::init();

    // Initialize Intent App Framework
    kprintln!("[INIT] Intent App Framework...");
    intent_kernel::apps::init();
    
    // Initialize Input Engine (Steno - fastest path)
    kprintln!("[INIT] Input Engine (Steno Path)...");
    steno::init();

    // Initialize Visual Layer (Semantic GUI)
    kprintln!("[INIT] Visual Projection Layer...");
    visual::init();
    // Register as a wildcard listener (Priority 10 = Normal)
    intent::register_wildcard(visual::create_handler(), "VisualLayer", 10);

    // Run Benchmarks (Sprint 11)
    // benchmarks::run_all();

    // Demo Neural Memory (ConceptID Edition) - STRESS TEST: 100,000 Concepts
    // unsafe {
    //     use kernel::memory::neural::{NEURAL_ALLOCATOR};
    //     use intent::ConceptID;
        
    //     // 5. Stress Test: Allocate 100,000 semantic concepts (verified up to 1,000,000)
    //     crate::kprintln!("\n       ════════════════════════════════════════════");
    //     crate::kprintln!("       🧠 STRESS TEST: 100,000 Concepts");
    //     crate::kprintln!("       ════════════════════════════════════════════");
        
    //     let start_cycles = profiling::rdtsc();
    //     let count = 10_000;
        
    //     let mut neura = NEURAL_ALLOCATOR.lock(); // Lock once for the loop
        
    //     // Clear previous allocations from benchmarks to free heap memory
    //     neura.clear();
        
    //     for i in 0..count {
    //         let id = ConceptID::new((i as u64) | 0xCAFE_0000);
            
    //         // Allocate 8 bytes per concept (minimal semantic block)
    //         neura.alloc(8, id);
            
    //         if i % 10_000 == 0 && i > 0 {
    //             crate::kprintln!("       Progress: {}/100k concepts allocated", i / 1_000);
    //         }
    //     }
        
    //     let end_cycles = profiling::rdtsc();
    //     let duration_cycles = end_cycles.wrapping_sub(start_cycles);
    //     // Approx cycles -> ms (assuming 1.5GHz for rough estimate)
    //     // 1.5M cycles = 1ms
    //     let ms = duration_cycles / 1_500_000;
        
    //     crate::kprintln!("       ════════════════════════════════════════════");
    //     crate::kprintln!("       ✅ Allocated {} concepts (ConceptID Index)", count);
    //     crate::kprintln!("       Time: {} ms", ms);
    //     crate::kprintln!("       ════════════════════════════════════════════");

    //     // 6. Verify Retrieval (Correctness)
    //     crate::kprintln!("       Index contains {} entries", neura.count());
    //     let target_id = ConceptID::new(50_000u64 | 0xCAFE_0000u64);
    //     crate::kprintln!("       Query Test: Retrieving Concept {:#x}...", target_id.0);
        
    //     let q_start = profiling::rdtsc();
    //     if let Some(ptr) = neura.retrieve(target_id) {
    //          let q_end = profiling::rdtsc();
    //          crate::kprintln!("       Found: Concept {:#x} in {} cycles (O(log N))", ptr.id.0, q_end.wrapping_sub(q_start));
    //     } else {
    //          crate::kprintln!("       ❌ Failed to retrieve concept!");
    //     }
    // }

    // ═══════════════════════════════════════════════════════════════════════════════
    // DEMO: LLM INFERENCE (Phase 8)
    // ═══════════════════════════════════════════════════════════════════════════════
    demo_llm(); 


    // ═══════════════════════════════════════════════════════════════════════════════
    // DEMO: INTENT-NATIVE APPS (Sprint 14)
    // ═══════════════════════════════════════════════════════════════════════════════
    {
        use intent_kernel::apps::{manifest, linker, registry, demo};
        
        kprintln!("\n       ════════════════════════════════════════════");
        kprintln!("       🚀 DEMO: Intent-Native App (Smart Doorknob)");
        kprintln!("       ════════════════════════════════════════════");
        
        // 1. Register Skills (Capabilities)
        demo::register_demo_skills();
        kprintln!("       [KERNEL] Registered Skills: 'Identify Person', 'Unlock Door'");
        
        // 2. Load Manifest (The "App")
        // Note: In a real system this comes from disk/network
        let manifest_source = r#"
app_name: "Smart Doorknob"
description: "Automatically unlocks for authorized users"
triggers:
  - input: "Face detected"
flow:
  - id: "chk_face"
    goal: "Identify Person"
    inputs: ["trigger.image"]
  
  - id: "action"
    goal: "Unlock Door"
    condition: "chk_face.result == 'Authorized User'"
"#;
        kprintln!("       [LOADER] Parsing 'Smart Doorknob' Manifest...");
        if let Ok(app) = manifest::AppManifest::parse(manifest_source) {
            kprintln!("       [PARSER] App: '{}' ({} triggers, {} steps)", 
                app.app_name, app.triggers.len(), app.flow.len());
                
            // 3. Simulate Run
            kprintln!("       [RUNTIME] Simulating Trigger: 'Face detected'...");
            
            // Step 1: Resolve "Identify Person"
            let step1 = &app.flow[0];
            kprintln!("       [LINKER] Resolving intent: '{}'...", step1.goal);
            if let Some(skill) = linker::SemanticLinker::resolve(&step1.goal) {
                kprintln!("       [LINKER] Bound to Capability: '{}'", skill.name());
                let ctx = registry::Context { user_id: 1 };
                match skill.execute("Face detected", &ctx) {
                    Ok(res) => kprintln!("       [EXEC] Output: '{}'", res),
                    Err(_) => kprintln!("       [EXEC] Failed"),
                }
            }
            
            // Step 2: Resolve "Unlock Door"
            let step2 = &app.flow[1];
            kprintln!("       [LINKER] Resolving intent: '{}'...", step2.goal);
             if let Some(skill) = linker::SemanticLinker::resolve(&step2.goal) {
                kprintln!("       [LINKER] Bound to Capability: '{}'", skill.name());
                let ctx = registry::Context { user_id: 1 };
                // Creating a mock context where previous step succeeded
                match skill.execute("Authorized User", &ctx) {
                    Ok(res) => kprintln!("       [EXEC] Output: '{}'", res),
                    Err(_) => kprintln!("       [EXEC] Failed"),
                }
            }
            
        } else {
             kprintln!("       [ERROR] Failed to parse manifest");
        }
        kprintln!("       ════════════════════════════════════════════");
    }


    // Initialize Perception Layer (Adaptive Hardware Support)
    if machine == dtb::MachineType::RaspberryPi5 {
        kprintln!("[INIT] Perception Cortex...");
        perception::init();
        
        let perception_mgr = perception::PERCEPTION_MANAGER.lock();
        kprintln!("       Active Sensors:");
        for sensor in perception_mgr.sensors() {
            kprintln!("       - {}", sensor.backend_name());
        }
    }

    // Initialize HUD (Legacy - now handled by Visual Layer)
    // kprintln!("[INIT] Heads-Up Display...");
    // perception::hud::init();

    // Initialize Filesystem
    kprintln!("[INIT] Filesystem...");
    fs::init();

    if machine == dtb::MachineType::RaspberryPi5 {
        // Initialize SD Card
        kprintln!("[INIT] SD Card Driver...");
        drivers::sd::init();
        
        // Mount FAT32 on SD
        kprintln!("[INIT] Mounting FAT32 on SD...");
        
        struct SdWrapper;
        impl fs::vfs::BlockDevice for SdWrapper {
            fn read_sector(&self, sector: u32, buffer: &mut [u8]) -> Result<(), &'static str> {
                if drivers::sd::read_block(sector, buffer).is_ok() {
                    Ok(())
                } else {
                    Err("IO Error")
                }
            }
            
            fn write_sector(&self, sector: u32, buffer: &[u8]) -> Result<(), &'static str> {
                if drivers::sd::write_block(sector, buffer).is_ok() {
                    Ok(())
                } else {
                    Err("IO Error")
                }
            }
        }
        
        let device = Arc::new(SdWrapper);
        let cached_dev = Arc::new(fs::cache::CachedDevice::new(device, 512));
        
        if let Ok(fs) = fs::fat32::Fat32FileSystem::mount(cached_dev) {
            let _ = fs::mount("/", fs);
            kprintln!("       Mounted SD Card at /");
        } else {
            kprintln!("       Failed to mount SD card");
        }
    } else {
        // QEMU / VirtIO Block
        kprintln!("[INIT] VirtIO Block Driver...");
        if drivers::virtio_blk::init().is_ok() {
             struct VirtioBlkWrapper;
             impl fs::vfs::BlockDevice for VirtioBlkWrapper {
                 fn read_sector(&self, sector: u32, buf: &mut [u8]) -> Result<(), &'static str> {
                     drivers::virtio_blk::read_sector(sector as u64, buf)
                 }
                 fn write_sector(&self, sector: u32, buf: &[u8]) -> Result<(), &'static str> {
                     drivers::virtio_blk::write_sector(sector as u64, buf)
                 }
             }
             let device = Arc::new(VirtioBlkWrapper);
             // Cache is useful
             let cached = Arc::new(fs::cache::CachedDevice::new(device, 512));
             if let Ok(fs) = fs::fat32::Fat32FileSystem::mount(cached) {
                 let _ = fs::mount("/", fs);
                 kprintln!("       Mounted VirtIO Block at /");
             } else {
                 kprintln!("       Failed to mount VirtIO Block FS");
             }
        } else {
            kprintln!("       VirtIO Block init failed (expected if no -drive)");
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════════
    // PERSISTENCE TEST
    // ═══════════════════════════════════════════════════════════════════════════════
    persistence_test();

    // ═══════════════════════════════════════════════════════════════════════════════
    // PERSISTENCE TEST
    // ═══════════════════════════════════════════════════════════════════════════════
    persistence_test();

    // ═══════════════════════════════════════════════════════════════════════════════
    // SYSCALL TEST
    // ═══════════════════════════════════════════════════════════════════════════════
    kprintln!("\n[KERNEL] Spawning Syscall Test Task...");
    // let _ = crate::kernel::scheduler::SCHEDULER.lock().spawn_simple(syscall_test_task);
    

    // Initialize PCIe (Already initialized earlier)
    // drivers::pcie::init();

    // Initialize USB
    if machine == dtb::MachineType::RaspberryPi5 {
        kprintln!("[INIT] USB Subsystem...");
        drivers::usb::init();
    }

    // Initialize Scheduler
    kprintln!("[INIT] Scheduler...");
    
    // Register Boot Thread as Agent (prevents corruption on first switch)
    crate::kernel::scheduler::SCHEDULER.lock().register_boot_agent();

    // Spawn Async Executor Agent (The "Main" Thread)
    // Spawn Async Executor Agent (The "Main" Thread)
    let _ = kernel::scheduler::SCHEDULER.lock().spawn_simple(async_executor_agent);
    kprintln!("       Spawned Async Executor Agent");

    // Spawn User Task (EL0 Process) from init.elf
    kprintln!("[INIT] Loading /init...");
    {
        // Read init
        let vfs = fs::VFS.lock();
        if let Ok(file) = vfs.open("/init", 0) { // Try plain /init first (as created by script)
            let mut file = file.lock();
            let size = file.stat().map(|s| s.size).unwrap_or(0);
            if size > 0 {
                // Allocate buffer (using Vec for convenience)
                let mut buf = alloc::vec![0u8; size as usize];
                
                if let Ok(n) = file.read(&mut buf) {
                    kprintln!("       Read {} bytes", n);
                    
                    // Spawn User Process
                    let mut scheduler = kernel::scheduler::SCHEDULER.lock();
                    match scheduler.spawn_user_elf(&buf) {
                        Ok(_) => kprintln!("       Spawned User Process 1 (init)"),
                        Err(e) => kprintln!("       Failed to spawn user process 1: {}", e),
                    }

                } else {
                    kprintln!("       Failed to read /init.elf");
                }
            } else {
                kprintln!("       /init.elf is empty");
            }
        } else {
            kprintln!("       Failed to open /init");
            // Fallback to embedded init
            kprintln!("       Loading embedded init...");
            // Embed the binary
            let init_bin = include_bytes!("../../user/init/target/aarch64-unknown-none/release/init");
            match kernel::scheduler::SCHEDULER.lock().spawn_user_elf(init_bin) {
                Ok(_) => kprintln!("       Spawned Embedded User Process (init)"),
                Err(e) => kprintln!("       Failed to spawn embedded init: {}", e),
            }
        }
    }

    // Enable Timer Interrupt (10ms)
    kprintln!("[INIT] Enabling Preemption...");
    drivers::timer::set_timer_interrupt(10_000);
    
    // Enable GIC for Timer (PPI 27 = Virtual Timer)
    drivers::interrupts::enable(27);

    // Enable Global Interrupts (DAIF)
    unsafe { arch::enable_interrupts(); }

    kprintln!();
    kprintln!("╔═══════════════════════════════════════════════════════════╗");
    kprintln!("║              INTENT KERNEL READY (USER MODE)              ║");
    kprintln!("╚═══════════════════════════════════════════════════════════╝");
    kprintln!();

    // Main loop (Idle task)
    loop {
        let core_id = arch::core_id();
        let start = kernel::scheduler::record_idle_start(core_id as usize);
        
        // Wait for interrupt
        // unsafe { crate::arch::wait_for_interrupt(); }
        kernel::scheduler::yield_task();
        
        kernel::scheduler::record_idle_end(core_id as usize, start);
    }
}

#[allow(dead_code)]
fn async_executor_agent() {
    kprintln!("[Executor] Starting Steno-Native Async Core...");
    let mut executor = kernel::async_core::Executor::new();
    executor.spawn(steno_loop());
    executor.spawn(usb_loop());
    executor.run();
}

/// USB Input Loop - polls for strokes from steno machine
#[allow(dead_code)]
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
                
                // [NEURAL] Feed into temporal dynamics for prediction
                let now = drivers::timer::uptime_ms();
                let primed = intent::process_intent_activation(intent.concept_id, 1.0, now);
                if primed > 0 {
                    kprintln!("[NEURAL] Input primed {} next concepts", primed);
                }

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
#[allow(dead_code)]
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
             
             // [NEURAL] Feed into temporal dynamics for prediction
             let now = drivers::timer::uptime_ms();
             let _ = intent::process_intent_activation(intent.concept_id, 1.0, now);

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
            
            // [NEURAL] Feed into temporal dynamics for prediction
            let now = drivers::timer::uptime_ms();
            let _ = intent::process_intent_activation(intent.concept_id, 1.0, now);

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

#[allow(dead_code)]
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
            
            // Syscall 3: Sleep 5000ms
            core::arch::asm!(
                "mov x8, #3",
                "mov x0, #5000",
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
    kprintln!("║   ╚═╝╚═╝  ╚═══╝   ╚═╝   ╚══════╝╚═╝  ╚═══╝   ╚═╝                  ║");
    kprintln!("║                                                                   ║");
    kprintln!("║      A Perceptual Computing Platform               v0.2      ║");
    kprintln!("║      *** KERNEL MAIN UPDATED ***                                  ║");
    kprintln!();
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

#[allow(dead_code)]
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
            0,
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
            fd, 0, 0, 0,
            &mut frame
        );
    } else {
        kprintln!("[TASK] Open Failed");
    }
    
    // 3. Test Exit
    kprintln!("[TASK] Exiting...");
    crate::kernel::syscall::dispatcher(
        crate::kernel::syscall::SyscallNumber::Exit as u64,
        0, 0, 0, 0,
        &mut frame
    );
    
    // Should not reach here
    kprintln!("[TASK] ERROR: Continued after exit!");
    loop {
        crate::kernel::syscall::dispatcher(
            crate::kernel::syscall::SyscallNumber::Yield as u64,
            0, 0, 0, 0,
            &mut frame
        );
    }
}

#[allow(dead_code)]
fn demo_llm() {
    use crate::llm;
    use alloc::vec::Vec;
    use alloc::vec;

    kprintln!("\n       ════════════════════════════════════════════");
    kprintln!("       🤖 DEMO: LLM Inference (System 2)");
    kprintln!("       ════════════════════════════════════════════");

    // Ownership holders
    let owned_model: Option<llm::loader::OwnedWeights>;
    let dummy_data: Option<Vec<f32>>;
    
    let config;

    // 1. Try to load model
    match llm::loader::load_model("model.bin") {
        Ok(model) => {
            kprintln!("       [LLM] ✅ Loaded 'model.bin' from filesystem!");
            config = model.config;
            owned_model = Some(model);
            dummy_data = None;
        },
        Err(e) => {
            kprintln!("       [LLM] ⚠️ Load failed: '{}'. Using dummy weights.", e);
            config = llm::Config {
                dim: 64,
                hidden_dim: 128,
                n_layers: 2,
                n_heads: 4,
                n_kv_heads: 4,
                vocab_size: 1024,
                seq_len: 32,
                shared_classifier: true,
            };
            owned_model = None;
            dummy_data = Some(vec![0.0f32; 65536]);
        }
    }
    
    kprintln!("       [LLM] Config: dim={}, layers={}, heads={}", config.dim, config.n_layers, config.n_heads);
    kprintln!("       [LLM] Allocating State...");
    
    // 2. Allocate State
    let mut state = llm::RunState::new(&config);
    kprintln!("       [LLM] State Allocated.");

    // 3. Get Weights Reference
    let weights = if let Some(ref m) = owned_model {
        m.as_weights()
    } else {
        let data = dummy_data.as_ref().unwrap();
        // Construct dummy weights
        // SAFETY: We use a small dummy buffer for all weights to avoid OOM. 
        // Logic will be garbage but it proves the engine runs.
        let slice = &data[..]; // Use the whole (small) buffer
        llm::Weights {
             token_embedding_table: slice,
             rms_att_weight: slice,
             rms_ffn_weight: slice,
             wq: slice,
             wk: slice,
             wv: slice,
             wo: slice,
             w1: slice,
             w2: slice,
             w3: slice,
             rms_final_weight: slice,
             w_cls: None,
        }
    };
    
    // 4. Run Inference Step
    kprintln!("       [LLM] Running Forward Pass (Token 1 -> ?)...");
    let start = crate::profiling::rdtsc();
    
    llm::inference::forward(1, 0, &config, &mut state, &weights);
    
    let end = crate::profiling::rdtsc();
    let cycles = end.wrapping_sub(start);
    
    kprintln!("       [LLM] Inference Complete.");
    kprintln!("       Cycles: {} ", cycles);
    
    kprintln!("       [LLM] Logits[0]: {}", state.logits.data[0]);
    kprintln!("       ✅ LLM Engine Online");
    kprintln!("       ════════════════════════════════════════════");
}

#[allow(dead_code)]
fn persistence_test() {
    kprintln!("\n[TEST] Testing Persistence...");
    let vfs = fs::VFS.lock();
    
    // 1. Create file
    match vfs.create("/persist.txt") {
        Ok(file) => {
            kprintln!("       Created /persist.txt");
            let mut file = file.lock();
            
            // 2. Write
            let data = b"Hello from Intent Kernel Persistence!";
            match file.write(data) {
                Ok(n) => kprintln!("       Written {} bytes", n),
                Err(e) => kprintln!("       Write Failed: {}", e),
            }
        }
        Err(e) => kprintln!("       Create Failed: {}", e),
    }
    
    // 3. Read back
    match vfs.open("/persist.txt", 0) {
        Ok(file) => {
            let mut file = file.lock();
            let mut buf = [0u8; 64];
            if let Ok(n) = file.read(&mut buf) {
                if let Ok(s) = core::str::from_utf8(&buf[..n]) {
                    kprintln!("       Read Back: '{}'", s);
                }
            }
        }
        Err(e) => kprintln!("       Open Failed: {}", e),
    }
}
