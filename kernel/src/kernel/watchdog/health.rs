//! System Health Monitoring
//!
//! Tracks CPU utilization, memory pressure, task queue depth, and other
//! health metrics across worker cores.

/// System health metrics
pub struct SystemHealth {
    pub cpu_usage: [u8; 3],      // Per-core CPU usage (0-100%)
    pub memory_used: usize,       // Bytes allocated
    pub memory_free: usize,       // Bytes available
    pub task_queue_depth: [usize; 3], // Tasks queued per core
    pub interrupt_latency_us: u32, // Average interrupt latency
}

impl SystemHealth {
    pub const fn new() -> Self {
        Self {
            cpu_usage: [0; 3],
            memory_used: 0,
            memory_free: 0,
            task_queue_depth: [0; 3],
            interrupt_latency_us: 0,
        }
    }
}

/// Measure overall system health
pub fn measure_health() -> SystemHealth {
    let mut health = SystemHealth::new();

    // Collect real metrics from kernel subsystems
    
    // 1. Memory Metrics
    let mem_stats = crate::kernel::memory::stats();
    health.memory_used = mem_stats.allocated;
    health.memory_free = crate::kernel::memory::heap_available();
    
    // 2. CPU and Queue Metrics (Per Core)
    for core_id in 0..3 {
        let stats = crate::kernel::scheduler::get_core_stats(core_id);
        
        // Calculate CPU usage percentage
        // Usage = (Total - Idle) / Total * 100
        // Note: This is cumulative since boot. For instantaneous, we'd need to diff against last sample.
        // For this implementation, we'll return the cumulative average, which converges to "uptime usage".
        // To do instantaneous, we'd need static state in this function or a separate sampler.
        // Let's use a simplified approach: if total > 0, calc %.
        
        if stats.total_cycles > 0 {
            let active = stats.total_cycles.saturating_sub(stats.idle_cycles);
            // Use u128 to prevent overflow during multiply
            let usage = (active as u128 * 100) / (stats.total_cycles as u128);
            health.cpu_usage[core_id] = usage as u8;
        } else {
            health.cpu_usage[core_id] = 0;
        }
        
        health.task_queue_depth[core_id] = stats.queue_length;
    }

    health
}

/// Check for memory leaks by monitoring allocation rate
pub fn check_memory_leaks() {
    // Simple threshold check for now
    let free = crate::kernel::memory::heap_available();
    let total = free + crate::kernel::memory::stats().allocated;
    
    // If free memory is less than 5% of total, warn
    if total > 0 && free < (total / 20) {
        crate::kprintln!("[HEALTH] WARNING: Low memory! Free: {} bytes", free);
    }
}

/// Measure CPU utilization for a specific core
pub fn measure_cpu_usage(core_id: usize) -> u8 {
    let stats = crate::kernel::scheduler::get_core_stats(core_id);
    if stats.total_cycles > 0 {
        let active = stats.total_cycles.saturating_sub(stats.idle_cycles);
        ((active as u128 * 100) / (stats.total_cycles as u128)) as u8
    } else {
        0
    }
}

/// Measure task queue depth
pub fn measure_queue_depth(core_id: usize) -> usize {
    crate::kernel::scheduler::yield_task();
    crate::kernel::scheduler::get_core_stats(core_id).queue_length
}

/// Detect thermal throttling (Pi 5 specific)
pub fn check_thermal() -> Option<u32> {
    // Thermal monitoring requires hardware-specific BCM2712 register access
    // Pi 5 thermal register: 0x107C5200
    // We can try to read it if we are on Pi 5
    if crate::dtb::machine_type() == crate::dtb::MachineType::RaspberryPi5 {
        // Safety: This is a raw MMIO read. We assume the address is mapped.
        // In our identity mapped kernel, it should be accessible.
        // 0x10_7C52_0000 (Pi 5 base?) No, Pi 5 peripherals are at 0x10_0000_0000 + offsets.
        // The register 0x107C5200 is likely an offset or legacy address.
        // Let's skip for now to avoid Data Abort if address is wrong.
        None
    } else {
        None
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_measurement() {
        let health = measure_health();
        assert!(health.cpu_usage[0] <= 100);
        assert!(health.cpu_usage[1] <= 100);
        assert!(health.cpu_usage[2] <= 100);
    }

    #[test]
    fn test_cpu_usage_bounds() {
        for core in 0..3 {
            let usage = measure_cpu_usage(core);
            assert!(usage <= 100);
        }
    }
}


impl Default for SystemHealth {
    fn default() -> Self {
        Self::new()
    }
}
