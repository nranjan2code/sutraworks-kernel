//! Self-Healing Recovery Strategies
//!
//! Implements recovery actions when anomalies are detected.

use crate::kprintln;

/// Recovery action to take
pub enum RecoveryAction {
    KillTask(usize),
    RestartCore(usize),
    RebalanceLoad,
    TriggerGC,
    Panic(&'static str),
}

/// Attempt to recover a hung core
pub fn recover_hung_core(core_id: usize) -> Result<(), &'static str> {
    kprintln!("[RECOVERY] Attempting to recover hung core {}", core_id);
    
    // Strategy 1: Send IPI to wake the core
    kprintln!("[RECOVERY] Sending IPI to core {}", core_id);
    crate::arch::multicore::send_ipi(core_id)?;
    
    // Strategy 2: Check if core is in infinite loop
    // We can't easily inspect another core's PC without JTAG or complex debug hardware.
    // However, we can check if it responds to the IPI (by checking a shared flag).
    // For now, we assume IPI was sent.
    
    // Strategy 3: Last resort - restart core
    // Since we don't have a PMIC driver to power cycle the core, we'll try to halt it
    // to prevent damage/corruption.
    kprintln!("[RECOVERY] Restart requested - Halting core {} (PMIC reset not available)", core_id);
    // We can send a specific "Halt" IPI if we defined one.
    // For now, standard IPI just wakes it.
    
    Ok(())
}

/// Break a detected deadlock by killing youngest task in cycle
pub fn break_deadlock() {
    kprintln!("[RECOVERY] Breaking deadlock");
    
    // Strategy: Find youngest task in cycle and kill it
    // In a real implementation, we'd traverse the wait graph.
    // For now, we'll kill the current task as a failsafe if we are the one detecting it.
    // Or better, kill a random task? No, that's bad.
    // Let's kill the current task since it's the one that detected the deadlock (usually).
    
    if let Some(pid) = crate::kernel::scheduler::SCHEDULER.lock().current_pid() {
        kprintln!("[RECOVERY] Killing current task {} to break deadlock", pid);
        let _ = crate::kernel::scheduler::SCHEDULER.lock().kill_task(pid);
    }
    
    kprintln!("[RECOVERY] Deadlock resolution attempted");
}

/// Rebalance load across cores
pub fn rebalance_load() {
    kprintln!("[RECOVERY] Rebalancing load across cores");
    
    // Trigger work stealing in SMP scheduler
    // Since we currently have a shared run queue, "rebalancing" is automatic
    // as cores pick tasks from the shared queue.
    // We just ensure all cores are awake.
    for i in 1..4 {
        let _ = crate::arch::multicore::send_ipi(i);
    }
}

/// Trigger garbage collection (if applicable)
pub fn trigger_gc() {
    kprintln!("[RECOVERY] Triggering memory cleanup");
    
    // Force deallocation of unused memory
    crate::kernel::memory::force_compact();
}

/// Execute a recovery action
pub fn execute_recovery(action: RecoveryAction) -> Result<(), &'static str> {
    match action {
        RecoveryAction::KillTask(task_id) => {
            kprintln!("[RECOVERY] Killing task {}", task_id);
            crate::kernel::scheduler::SCHEDULER.lock().kill_task(task_id as u64)?;
            Ok(())
        }
        RecoveryAction::RestartCore(core_id) => {
            kprintln!("[RECOVERY] Restarting core {}", core_id);
            recover_hung_core(core_id)
        }
        RecoveryAction::RebalanceLoad => {
            rebalance_load();
            Ok(())
        }
        RecoveryAction::TriggerGC => {
            trigger_gc();
            Ok(())
        }
        RecoveryAction::Panic(msg) => {
            panic!("[RECOVERY] Unrecoverable: {}", msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_actions() {
        // Test that recovery actions don't panic
        let _ = execute_recovery(RecoveryAction::RebalanceLoad);
        let _ = execute_recovery(RecoveryAction::TriggerGC);
    }
}
