use crate::kprintln;
use crate::profiling::{self, PROFILER};
use core::sync::atomic::Ordering;
use core::alloc::GlobalAlloc;

/// Run all benchmarks
pub fn run_all() {
    kprintln!("\n╔═══════════════════════════════════════════════════════════╗");
    kprintln!("║                 KERNEL BENCHMARKS                         ║");
    kprintln!("╚═══════════════════════════════════════════════════════════╝\n");

    kprintln!("[BENCH] Running kernel benchmarks...\n");
    
    bench_context_switch();
    bench_syscall_latency();
    bench_memory_alloc();
    bench_syscall_user();  // ✅ Re-enabled - bug fixed!
    
    kprintln!("\n[BENCH] All benchmarks completed.\n");
}

/// Measure Full Syscall Round-Trip (User Mode)
fn bench_syscall_user() {
    kprintln!("[BENCH] Running Syscall Round-Trip Benchmark (User Mode)...");
    kprintln!("  -> Spawning user task to measure 10,000 syscalls...");
    
    extern "C" {
        fn user_bench_entry();
    }
    
    let entry: fn() = unsafe { core::mem::transmute(user_bench_entry as unsafe extern "C" fn()) };
    
    crate::kernel::scheduler::SCHEDULER.lock().spawn_user_simple(entry, 0).expect("Failed to spawn user bench");
}

core::arch::global_asm!(
    ".align 12", // Page align
    ".global user_bench_entry",
    "user_bench_entry:",
    // 1. Get Start Time
    "mrs x19, cntvct_el0",
    
    // 2. Loop 10000 times
    "mov x22, #0",
    "mov x21, #10000",
    "1:",
    "cmp x22, x21",
    "b.ge 2f",
    
    // Syscall: GetPid (17)
    "mov x8, #17",
    "svc #0",
    
    "add x22, x22, #1",
    "b 1b",
    
    "2:",
    // 3. Get End Time
    "mrs x20, cntvct_el0",
    
    // 4. Calculate Diff
    "sub x0, x20, x19",
    
    // 5. Exit(diff)
    "mov x8, #0", // syscall = Exit
    "svc #0",
    
    "b .",
    ".align 12" // Pad to page boundary
);

/// Measure System Call Latency
fn bench_syscall_latency() {
    kprintln!("[BENCH] Running Syscall Latency Benchmark...");
    
    let iterations = 10_000;
    let start = profiling::rdtsc();
    
    for _ in 0..iterations {
        // We use a direct function call to the dispatcher to avoid userspace transition overhead for now,
        // effectively measuring the *kernel* side of the syscall latency.
        // To measure full latency, we'd need a user program.
        // But wait, we can't easily call dispatcher directly because it expects ExceptionFrame.
        // Instead, let's just measure a simple kernel function call overhead vs "simulated" syscall logic.
        
        // Actually, let's use the PROFILER stats we already have!
        // If we run a user program that does 10k getpids, we can see the average cycles.
        
        // For this micro-benchmark, we will simulate the work done in a syscall
        // by calling a function that does similar work to sys_getpid.
        let _ = crate::kernel::scheduler::SCHEDULER.lock().current_pid();
    }
    
    let end = profiling::rdtsc();
    let total_cycles = end.wrapping_sub(start);
    let avg_cycles = total_cycles / iterations;
    
    kprintln!("  -> Total Cycles: {}", total_cycles);
    kprintln!("  -> Avg Cycles:   {}", avg_cycles);
    kprintln!("  -> Iterations:   {}", iterations);
}

/// Measure Context Switch Latency (Stress Test)
fn bench_context_switch() {
    kprintln!("[BENCH] Running Context Switch Stress Test...");
    
    BENCH_COUNTER.store(0, Ordering::Relaxed);
    let start_switches = PROFILER.context_switches.load(Ordering::Relaxed);
    let start_cycles = profiling::rdtsc();
    
    // Spawn a worker task
    crate::kernel::scheduler::SCHEDULER.lock().spawn_simple(bench_worker).expect("Failed to spawn bench worker");
    
    // Yield in a loop until done
    while BENCH_COUNTER.load(Ordering::Relaxed) < 10_000 {
        crate::kernel::scheduler::yield_task();
        BENCH_COUNTER.fetch_add(1, Ordering::Relaxed);
    }
    
    let end_cycles = profiling::rdtsc();
    let end_switches = PROFILER.context_switches.load(Ordering::Relaxed);
    
    let total_switches = end_switches - start_switches;
    let total_cycles = end_cycles - start_cycles;
    
    let avg_cycles = if total_switches > 0 { total_cycles / total_switches } else { 0 };
    
    kprintln!("  -> Total Switches: {}", total_switches);
    kprintln!("  -> Total Cycles:   {}", total_cycles);
    kprintln!("  -> Avg Cycles/Switch: {}", avg_cycles);
}

static BENCH_COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);

fn bench_worker() {
    while BENCH_COUNTER.load(Ordering::Relaxed) < 10_000 {
        crate::kernel::scheduler::yield_task();
        BENCH_COUNTER.fetch_add(1, Ordering::Relaxed);
    }
}

/// Measure Memory Allocation Performance
fn bench_memory_alloc() {
    kprintln!("[BENCH] Running Memory Allocation Benchmark...");
    
    let iterations = 10_000;
    let layout = core::alloc::Layout::new::<u64>(); // 8 bytes (Slab)
    
    let start = profiling::rdtsc();
    
    unsafe {
        for _ in 0..iterations {
            let ptr = crate::kernel::memory::global_allocator().alloc(layout);
            if !ptr.is_null() {
                crate::kernel::memory::global_allocator().dealloc(ptr, layout);
            }
        }
    }
    
    let end = profiling::rdtsc();
    let total_cycles = end.wrapping_sub(start);
    let avg_cycles = total_cycles / iterations as u64;
    
    kprintln!("  -> Slab Alloc/Free (8 bytes)");
    kprintln!("  -> Total Cycles: {}", total_cycles);
    kprintln!("  -> Avg Cycles:   {}", avg_cycles);
    
    // Buddy Allocator (Page)
    let iterations_pages = 1_000;
    let layout_page = core::alloc::Layout::from_size_align(4096, 4096).unwrap();
    
    let start_page = profiling::rdtsc();
    
    unsafe {
        for _ in 0..iterations_pages {
            let ptr = crate::kernel::memory::global_allocator().alloc(layout_page);
            if !ptr.is_null() {
                crate::kernel::memory::global_allocator().dealloc(ptr, layout_page);
            }
        }
    }
    
    let end_page = profiling::rdtsc();
    let total_cycles_page = end_page.wrapping_sub(start_page);
    let avg_cycles_page = total_cycles_page / iterations_pages as u64;
    
    kprintln!("  -> Buddy Alloc/Free (4KB)");
    kprintln!("  -> Total Cycles: {}", total_cycles_page);
    kprintln!("  -> Avg Cycles:   {}", avg_cycles_page);
}
