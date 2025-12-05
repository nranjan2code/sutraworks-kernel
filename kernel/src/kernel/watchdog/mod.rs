//! Semantic Immune System - Watchdog Core
//!
//! Dedicated Core 3 monitoring system that watches over worker cores (0-2),
//! detects anomalies, and enables self-healing capabilities.

pub mod health;
pub mod deadlock;
pub mod recovery;

use alloc::vec::Vec;
use crate::arch::{self, SpinLock};
use crate::kprintln;
use core::sync::atomic::{AtomicU64, Ordering};

/// Watchdog core ID (always Core 3 on Pi 5)
pub const WATCHDOG_CORE: usize = 3;

/// Number of worker cores being monitored
pub const NUM_WORKER_CORES: usize = 3;

/// Heartbeat timeout in milliseconds
pub const HEARTBEAT_TIMEOUT_MS: u64 = 200;

/// Per-core heartbeat timestamps (atomic for lock-free updates)
static HEARTBEATS: [AtomicU64; NUM_WORKER_CORES] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];

/// Alert channel for watchdog notifications
pub enum Alert {
    CoreHung(usize),
    DeadlockDetected(Vec<u64>),
    MemoryLeak(usize),
    HighLoad(usize),
}

/// Watchdog Core - The kernel's immune system
pub struct WatchdogCore {
    core_id: usize,
    alert_queue: SpinLock<alloc::collections::VecDeque<Alert>>,
}

impl WatchdogCore {
    /// Create new watchdog instance
    pub const fn new() -> Self {
        Self {
            core_id: WATCHDOG_CORE,
            alert_queue: SpinLock::new(alloc::collections::VecDeque::new()),
        }
    }

    /// Main monitoring loop - runs forever on Core 3
    pub fn monitor_loop(&self) -> ! {
        kprintln!("[WATCHDOG] Core {} monitoring started", self.core_id);
        kprintln!("[WATCHDOG] Watching cores 0-{}", NUM_WORKER_CORES - 1);

        let mut cycle_count: u64 = 0;

        loop {
            cycle_count += 1;

            // Health check every cycle
            self.check_heartbeats();

            // Deadlock detection every 10 cycles
            if cycle_count % 10 == 0 {
                if let Some(deadlocked_tasks) = deadlock::detect_circular_wait() {
                    self.alert(Alert::DeadlockDetected(deadlocked_tasks));
                    self.handle_deadlock();
                }
            }

            // Memory check every 100 cycles
            if cycle_count % 100 == 0 {
                health::check_memory_leaks();
            }

            // Performance monitoring
            health::measure_health();

            // Process alerts
            self.process_alerts();

            // Yield lightly - we don't want to sleep, just let others run
            arch::wfi(); // Low power until next interrupt
        }
    }

    /// Check if all worker cores are sending heartbeats
    fn check_heartbeats(&self) {
        let now = crate::drivers::timer::uptime_ms();

        for core_id in 0..NUM_WORKER_CORES {
            let last_beat = HEARTBEATS[core_id].load(Ordering::Relaxed);

            if last_beat > 0 && (now - last_beat) > HEARTBEAT_TIMEOUT_MS {
                kprintln!("[WATCHDOG] ⚠️  Core {} heartbeat timeout!", core_id);
                self.alert(Alert::CoreHung(core_id));
                self.handle_hung_core(core_id);
            }
        }
    }

    /// Handle a core that has stopped responding
    fn handle_hung_core(&self, core_id: usize) {
        kprintln!("[WATCHDOG] 🔧 Attempting recovery for core {}", core_id);
        
        // Try recovery strategies
        if let Err(e) = recovery::recover_hung_core(core_id) {
            kprintln!("[WATCHDOG] ❌ Recovery failed: {}", e);
            kprintln!("[WATCHDOG] ⚠️  CRITICAL: Core {} unrecoverable", core_id);
        } else {
            kprintln!("[WATCHDOG] ✅ Core {} recovered", core_id);
        }
    }

    /// Handle detected deadlock
    fn handle_deadlock(&self) {
        kprintln!("[WATCHDOG] 🔧 Deadlock detected - initiating recovery");
        recovery::break_deadlock();
    }

    /// Queue an alert
    fn alert(&self, alert: Alert) {
        self.alert_queue.lock().push_back(alert);
    }

    /// Process queued alerts
    fn process_alerts(&self) {
        let mut queue = self.alert_queue.lock();
        
        while let Some(alert) = queue.pop_front() {
            match alert {
                Alert::CoreHung(id) => {
                    kprintln!("[WATCHDOG] Alert: Core {} hung", id);
                }
                Alert::DeadlockDetected(tasks) => {
                    kprintln!("[WATCHDOG] Alert: Deadlock involving {} tasks", tasks.len());
                }
                Alert::MemoryLeak(bytes) => {
                    kprintln!("[WATCHDOG] Alert: Memory leak detected ({} bytes)", bytes);
                }
                Alert::HighLoad(core) => {
                    kprintln!("[WATCHDOG] Alert: High load on core {}", core);
                }
            }
        }
    }
}

/// Global watchdog instance
static WATCHDOG: WatchdogCore = WatchdogCore::new();

/// Initialize watchdog on Core 3
pub fn init() {
    let current_core = arch::core_id();
    
    if current_core as usize != WATCHDOG_CORE {
        kprintln!("[WATCHDOG] Error: init() must be called from Core 3");
        return;
    }

    kprintln!("[WATCHDOG] Initializing on Core {}", WATCHDOG_CORE);
    
    // Enter monitoring loop (never returns)
    WATCHDOG.monitor_loop();
}

/// Record heartbeat from a worker core (called by workers)
pub fn heartbeat() {
    let core_id = arch::core_id() as usize;
    
    if core_id < NUM_WORKER_CORES {
        let now = crate::drivers::timer::uptime_ms();
        HEARTBEATS[core_id].store(now, Ordering::Relaxed);
    }
}

/// Start the watchdog core (called from kernel_main on Core 0)
pub fn start_watchdog() {
    use crate::arch::multicore;
    
    kprintln!("[WATCHDOG] Starting watchdog on Core {}...", WATCHDOG_CORE);
    
    unsafe {
        // Boot Core 3 with watchdog entry point
        multicore::start_core(WATCHDOG_CORE, watchdog_entry_addr())
            .expect("Failed to start watchdog core");
    }
    
    kprintln!("[WATCHDOG] Watchdog core started");
}

/// Get entry point address for watchdog core
fn watchdog_entry_addr() -> u64 {
    watchdog_entry as *const () as u64
}

/// Entry point for watchdog core (called by boot.s on Core 3)
extern "C" fn watchdog_entry() {
    kprintln!("[WATCHDOG] Core {} booted", arch::core_id());
    
    // Initialize watchdog (enters monitoring loop, never returns)
    init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heartbeat_update() {
        heartbeat();
        let core_id = arch::core_id() as usize;
        if core_id < NUM_WORKER_CORES {
            assert!(HEARTBEATS[core_id].load(Ordering::Relaxed) > 0);
        }
    }

    #[test]
    fn test_watchdog_constants() {
        assert_eq!(WATCHDOG_CORE, 3);
        assert_eq!(NUM_WORKER_CORES, 3);
        assert!(HEARTBEAT_TIMEOUT_MS > 0);
    }
}
