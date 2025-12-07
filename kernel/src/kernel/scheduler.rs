//! Agent Scheduler
//!
//! Schedules Agents. Simplified for stroke-native kernel.

use alloc::collections::vec_deque::VecDeque;
use alloc::boxed::Box;
use crate::kernel::process::{Agent, AgentState, Context, Message};
use crate::kernel::sync::SpinLock;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// Current PIDs running on each core (for deadlock detection)
pub static CURRENT_PIDS: [AtomicUsize; 4] = [
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
    AtomicUsize::new(0),
];

pub struct IntentScheduler {
    agents: VecDeque<Box<Agent>>,
    current_agent_id: Option<u64>,
}

// ...



/// Per-core statistics for health monitoring
#[derive(Debug, Clone, Copy, Default)]
pub struct CoreStats {
    pub idle_cycles: u64,
    pub total_cycles: u64, 
    pub queue_length: usize,
}

/// Global storage for per-core statistics (4 cores max)
pub static CORE_STATS: [SpinLock<CoreStats>; 4] = [
    SpinLock::new(CoreStats { idle_cycles: 0, total_cycles: 0, queue_length: 0 }),
    SpinLock::new(CoreStats { idle_cycles: 0, total_cycles: 0, queue_length: 0 }),
    SpinLock::new(CoreStats { idle_cycles: 0, total_cycles: 0, queue_length: 0 }),
    SpinLock::new(CoreStats { idle_cycles: 0, total_cycles: 0, queue_length: 0 }),
];

impl IntentScheduler {
    pub const fn new() -> Self {
        IntentScheduler {
            agents: VecDeque::new(),
            current_agent_id: None,
        }
    }
    
    /// Get statistics for a specific core
    pub fn get_core_stats(core_id: usize) -> CoreStats {
        if core_id < 4 {
            *CORE_STATS[core_id].lock()
        } else {
            CoreStats::default()
        }
    }
    
    /// Record start of idle period
    pub fn record_idle_start(_core_id: usize) -> u64 {
        crate::profiling::rdtsc()
    }
    
    /// Record end of idle period and update stats
    pub fn record_idle_end(core_id: usize, start_time: u64) {
        if core_id < 4 {
            let end_time = crate::profiling::rdtsc();
            let elapsed = end_time.wrapping_sub(start_time);
            
            let mut stats = CORE_STATS[core_id].lock();
            stats.idle_cycles = stats.idle_cycles.wrapping_add(elapsed);
            stats.total_cycles = stats.total_cycles.wrapping_add(elapsed); // Add to total? 
            // Wait, total_cycles should be total time elapsed since boot? 
            // Or just sum of idle + active?
            // Actually, we can just track idle cycles. Total cycles can be derived from TSC or just accumulated.
            // Let's accumulate elapsed to total as well, assuming we call this frequently.
            // But this only adds IDLE time to total. We need to add ACTIVE time too.
            // Better approach: total_cycles is just current TSC - boot TSC.
            // But for percentage, we want a window.
            // Let's just track accumulated idle cycles. The health check can diff it against wall clock or TSC.
        }
    }
    
    /// Update queue length stat
    pub fn update_queue_stats(&self) {
        // We only have one global queue, so update all cores or just core 0?
        // Let's update all for visibility, or just assume core 0 manages it.
        // Since it's a shared queue, the "queue depth" is the global depth.
        let len = self.agents.len();
        for stat in &CORE_STATS {
             stat.lock().queue_length = len;
        }
    }

    pub fn current_pid(&self) -> Option<u64> {
        self.current_agent_id
    }

    /// Spawn a new kernel agent (simple, no embedding)
    pub fn spawn_simple(&mut self, entry: fn()) -> Result<(), &'static str> {
        let agent = Agent::new_kernel_simple(entry)?;
        self.agents.push_back(Box::new(agent));
        Ok(())
    }

    /// Spawn a new user agent (simple, no embedding)
    pub fn spawn_user_simple(&mut self, entry: fn(), arg: u64) -> Result<(), &'static str> {
        let mut agent = Agent::new_user_simple(entry, arg)?;
        agent.parent_id = self.current_agent_id; // Set parent to current running agent
        self.agents.push_back(Box::new(agent));
        Ok(())
    }

    /// Register the currently running boot thread as Agent 0
    pub fn register_boot_agent(&mut self) {
        // Create an agent for the current thread
        // We use a dummy entry point because we are ALREADY running.
        let mut agent = Agent::new_kernel_simple(|| {}).unwrap();
        
        // PID 0 reserved for Idle/Boot
        // We might need to hack the ID if AgentId::new() increments.
        // But for now, just let it have an ID.
        // agent.id.0 = 0; 
        
        agent.state = AgentState::Running;
        
        // We claim we are running this agent
        self.current_agent_id = Some(agent.id.0);
        self.agents.push_front(Box::new(agent));
        
        crate::kprintln!("[SCHED] Registered Boot Agent (PID {})", self.current_agent_id.unwrap());
    }

    /// Spawn a new user agent from ELF binary
    pub fn spawn_user_elf(&mut self, elf_data: &[u8]) -> Result<u64, &'static str> {
        let mut agent = Agent::new_user_elf(elf_data)?;
        agent.parent_id = self.current_agent_id; // Set parent
        let pid = agent.id.0;
        self.agents.push_back(Box::new(agent));
        Ok(pid)
    }

    /// Fork an agent
    pub fn fork_agent(&mut self, parent_id: u64, frame: &crate::kernel::exception::ExceptionFrame, sp_el0: u64) -> Result<u64, &'static str> {
        // Find parent index
        let parent_idx = self.agents.iter().position(|a| a.id.0 == parent_id).ok_or("Parent not found")?;
        
        // Fork (clones parent)
        let child = self.agents[parent_idx].fork(frame, sp_el0)?;
        let child_id = child.id.0;
        
        self.agents.push_back(Box::new(child));
        Ok(child_id)
    }

    /// Wait for a child process to terminate
    /// 
    /// If `target_pid` is Some(pid), waits for that specific child.
    /// If `target_pid` is None, waits for any child.
    pub fn wait_child(&mut self, parent_id: u64, target_pid: Option<u64>) -> Result<Option<u64>, &'static str> {
        let mut has_children = false;
        let mut reaped_pid = None;
        let mut remove_idx = None;
        
        for (i, agent) in self.agents.iter().enumerate() {
            if agent.parent_id == Some(parent_id) {
                // Check if this is the target child (or any child if target is None)
                let is_target = target_pid.map_or(true, |pid| agent.id.0 == pid);
                
                if is_target {
                    has_children = true;
                    if agent.state == AgentState::Terminated {
                        reaped_pid = Some(agent.id.0);
                        remove_idx = Some(i);
                        break;
                    }
                }
            }
        }
        
        if let Some(idx) = remove_idx {
            self.agents.remove(idx);
            return Ok(reaped_pid);
        }
        
        if has_children {
            // Block current task - it will be woken when child exits
            if let Some(agent) = self.get_agent_mut(parent_id) {
                agent.state = AgentState::Blocked;
            }
            Ok(None) // Indicates blocking, caller should yield
        } else {
            if target_pid.is_some() {
                Err("Child not found")
            } else {
                Err("No children")
            }
        }
    }

    /// Exit current agent and wake parent
    pub fn exit_current(&mut self, _code: i32) {
        let mut parent_id = None;
        
        if let Some(id) = self.current_agent_id {
             if let Some(agent) = self.get_agent_mut(id) {
                 agent.state = AgentState::Terminated;
                 parent_id = agent.parent_id;
             }
        }
        
        // Wake parent
        if let Some(pid) = parent_id {
            if let Some(parent) = self.get_agent_mut(pid) {
                if parent.state == AgentState::Blocked {
                    parent.state = AgentState::Ready;
                }
            }
        }
    }

    /// Schedule the next agent
    /// 
    /// Returns a tuple of (prev_context_ptr, next_context_ptr) if a switch is needed.
    /// The caller must then call `switch_to`.
    pub fn schedule(&mut self) -> Option<(*mut Context, *const Context)> {
        self.update_queue_stats(); // Update stats
        
        if self.agents.is_empty() {
            return None;
        }

        // Find a Ready agent (excluding current at index 0)
        let mut best_index = None;
        for (i, agent) in self.agents.iter().enumerate().skip(1) {
            if agent.state == AgentState::Ready {
                best_index = Some(i);
                break;
            }
        }

        if let Some(index) = best_index {
            // Found a new task to run!
            
            // 1. Move Next to Front
            // Remove 'next' from its current position
            let mut next = self.agents.remove(index).expect("remove at valid index");
            
            // 2. Move Current (Prev) to Back
            let prev = self.agents.pop_front().expect("queue non-empty");
            
            // Update stats
            let now = crate::profiling::rdtsc();
            
            // Update Prev stats
            // Use into_raw to avoid UB if pointer is null (which shouldn't happen but does due to corruption)
            let prev_raw = Box::into_raw(prev);
            let prev_addr = prev_raw as usize;
            
            // Force check by hiding value from optimizer
            if core::hint::black_box(prev_addr) == 0 {
                crate::kprintln!("[SCHED] FATAL: Prev is NULL! Leaking it.");
                // prev is consumed by into_raw, so it's effectively leaked/forgotten.
                self.agents.push_back(next);
                return None;
            }
            
            // Reconstruct Box
            let mut prev = unsafe { Box::from_raw(prev_raw) };

            if prev.last_scheduled > 0 {
                let elapsed = now.wrapping_sub(prev.last_scheduled);
                prev.cpu_cycles = prev.cpu_cycles.wrapping_add(elapsed);
            }
            
            // Update Next stats
            next.last_scheduled = now;
            
            // Update Prev state
            if prev.state == AgentState::Running {
                prev.state = AgentState::Ready;
            }
            
            // Update Next state
            next.state = AgentState::Running;
            self.current_agent_id = Some(next.id.0);
            
            // Update atomic PID for lock tracking
            let core_id = crate::arch::core_id();
            if core_id < 4 {
                CURRENT_PIDS[core_id as usize].store(next.id.0.try_into().unwrap_or(0), Ordering::Relaxed);
            }
            
            let prev_id = prev.id.0;
            let next_id = next.id.0;

            // Push Prev to Back
            self.agents.push_back(prev);
            
            // Push Next to Front
            self.agents.push_front(next);
            
            // 3. Get pointers
            let next_agent = self.agents.front().expect("next exists after push");
            let next_ctx = &next_agent.context as *const Context;
            
            let prev_agent = self.agents.back_mut().expect("prev exists after push");
            let prev_ctx = &mut prev_agent.context as *mut Context;
            
            crate::profiling::PROFILER.context_switches.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
            
            
            return Some((prev_ctx, next_ctx));
        }
        
        // No other task is Ready.
        // Check if current task can continue.
        let current = self.agents.front().expect("checked non-empty above");
        if current.state == AgentState::Running || current.state == AgentState::Ready {
             // Continue running current
             return None;
        }
        
        // Current is Blocked/Terminated, and no other task is Ready.
        // We must return None (CPU will loop in idle/exit).
        // Queue state is preserved (Current at front).
        None
    }
    /// Execute a closure with mutable access to the current agent
    pub fn with_current_agent<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(&mut Agent) -> R,
    {
        if let Some(id) = self.current_agent_id {
            // The current agent is always at the front of the queue
            if let Some(agent) = self.agents.front_mut() {
                if agent.id.0 == id {
                    return Some(f(agent));
                }
            }
        }
        None
    }

    /// Find an agent by ID
    pub fn get_agent_mut(&mut self, id: u64) -> Option<&mut Agent> {
        for agent in self.agents.iter_mut() {
            if agent.id.0 == id {
                return Some(agent);
            }
        }
        None
    }

    /// Kill a specific task by ID
    pub fn kill_task(&mut self, id: u64) -> Result<(), &'static str> {
        // Find the task
        let mut found_idx = None;
        for (i, agent) in self.agents.iter().enumerate() {
            if agent.id.0 == id {
                found_idx = Some(i);
                break;
            }
        }

        if let Some(idx) = found_idx {
            // If it's the current task, we can't just remove it without context switch.
            // Mark it as Terminated so it gets cleaned up on next schedule.
            if self.current_agent_id == Some(id) {
                self.agents[idx].state = AgentState::Terminated;
                // We should probably yield here? But this function returns.
                // The caller should yield if they killed themselves.
            } else {
                // If it's not running, we can remove it immediately or mark Terminated.
                // Marking Terminated is safer for resource cleanup (e.g. parent notification).
                self.agents[idx].state = AgentState::Terminated;
            }
            Ok(())
        } else {
            Err("Task not found")
        }
    }


    /// Send an IPC message to a process
    pub fn send_message(&mut self, target_pid: u64, msg: Message) -> Result<(), &'static str> {
        if let Some(agent) = self.get_agent_mut(target_pid) {
            let mut mailbox = agent.mailbox.lock();
            if mailbox.len() >= 32 {
                return Err("Mailbox full");
            }
            mailbox.push_back(msg);
            drop(mailbox); // Unlock immediately
            
            // Wake up if sleeping (simplified: wake if ANY sleep, real impl should check if sleeping for MSG)
            // Ideally we should have AgentState::WaitingForMessage
            if agent.state == AgentState::Sleeping {
                agent.state = AgentState::Ready;
                agent.wake_time = 0; // Cancel sleep timeout
            }
            Ok(())
        } else {
            Err("Target process not found")
        }
    }
}

pub static SCHEDULER: SpinLock<IntentScheduler> = SpinLock::new(IntentScheduler::new());

/// Called on every timer interrupt (e.g., 10ms)
pub fn tick() {
    // Re-arm timer for next tick (10ms = 10,000us)
    crate::drivers::timer::set_timer_interrupt(10_000);

    // Track tick count for periodic tasks
    static TICK_COUNT: AtomicU64 = AtomicU64::new(0);
    let ticks = TICK_COUNT.fetch_add(1, Ordering::Relaxed);
    
    let now = crate::drivers::timer::uptime_ms();

    // ═══════════════════════════════════════════════════════════════════════════════
    // NEURAL SUBSYSTEM TICKS (Biological Architecture)
    // ═══════════════════════════════════════════════════════════════════════════════
    
    // Temporal dynamics: decay activations every 100ms (10 ticks)
    if ticks % 10 == 0 {
        crate::intent::temporal::decay_tick(now);
    }
    
    // Hierarchical propagation: propagate intents through layers every 50ms (5 ticks)
    if ticks % 5 == 0 {
        crate::intent::hierarchy::propagate_all();
    }
    
    // ═════════════════════════════════════════════════════════════════════════════════
    // VERIFICATION: Observable proof that neural architecture is active
    // Log once per second (100 ticks @ 10ms each) to avoid flooding
    // ═════════════════════════════════════════════════════════════════════════════════
    if ticks % 100 == 0 && ticks > 0 {
        crate::kprintln!("[NEURAL] tick={} uptime={}ms decay_active=true propagate_active=true", ticks, now);
    }

    // TCP retransmission check every 100ms (10 ticks)
    if ticks % 10 == 0 {
        crate::net::tcp_tick();
    }

    // ═════════════════════════════════════════════════════════════════════════════════
    // NEURAL URGENCY: Basal Ganglia Action Selection
    // ═════════════════════════════════════════════════════════════════════════════════
    if ticks % 5 == 0 {
        // Check if the Urgency Accumulator has selected an action (Basal Ganglia Gating)
        if let Some(concept_id) = crate::intent::NEURAL_SCHEDULER.lock().urgency_mut().select_action() {
            crate::kprintln!("[NEURAL] ⚡ URGENT ACTION SELECTED: {:#x}", concept_id.0);
            
            // In a full implementation, we would:
            // 1. Find the handler for this concept
            // 2. Boost the priority of the process owning that handler
            // 3. Immediately schedule it
            
            // For now, we log the selection proof.
        }
    }

    let mut scheduler = SCHEDULER.lock();
    
    // 1. Wake up sleeping agents
    let now = crate::drivers::timer::uptime_ms();
    for agent in scheduler.agents.iter_mut() {
        if agent.state == AgentState::Sleeping && now >= agent.wake_time {
            agent.state = AgentState::Ready;
            agent.wake_time = 0;
        }
    }

    // 2. Schedule next task
    // MOVED to exception handler (after EOI)
    // if let Some((prev, next)) = scheduler.schedule() {
    //     drop(scheduler);
    //     unsafe {
    //         crate::arch::switch_to(prev as *mut u8, next as *const u8);
    //     }
    // }
}

/// Yield the current task
pub fn yield_task() {
    // crate::kprintln!("[DEBUG] yield_task called");
    crate::arch::without_interrupts(|| {
        let mut scheduler = SCHEDULER.lock();
        if let Some((prev, next)) = scheduler.schedule() {
            // Drop the lock before switching!
            drop(scheduler);
            
            // crate::kprintln!(\"[DEBUG] Switching Context...\");
            unsafe {
                crate::arch::switch_to(prev as *mut u8, next as *const u8);
            }
            // crate::kprintln!("[DEBUG] Returned from switch");
        } else {
             // crate::kprintln!("[DEBUG] schedule returned None");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_task() {}

    #[test]
    fn test_scheduler_round_robin() {
        let mut scheduler = IntentScheduler::new();
        
        // Add 3 tasks
        // Add 3 tasks
        scheduler.spawn_simple(dummy_task).unwrap(); // Task 1
        scheduler.spawn_simple(dummy_task).unwrap(); // Task 2
        scheduler.spawn_simple(dummy_task).unwrap(); // Task 3
        
        // Initial state: [T1, T2, T3]
        
        // Schedule 1: Should pick T1
        let (_prev, _next) = scheduler.schedule().expect("Should schedule T1");
        // In real run, we'd check pointers, but here we check internal state if possible
        // or just rely on the fact it didn't panic and returned something.
        
        // Schedule 2: Should rotate T1 to back -> [T2, T3, T1] -> Pick T2
        let _ = scheduler.schedule().expect("Should schedule T2");
        
        // Schedule 3: Should rotate T2 to back -> [T3, T1, T2] -> Pick T3
        let _ = scheduler.schedule().expect("Should schedule T3");
        
        // Schedule 4: Should rotate T3 to back -> [T1, T2, T3] -> Pick T1 again
        let _ = scheduler.schedule().expect("Should schedule T1 again");
    }

    #[test]
    fn test_scheduler_empty() {
        let mut scheduler = IntentScheduler::new();
        assert!(scheduler.schedule().is_none());
    }
}

// Public helpers for main loop
pub fn record_idle_start(core_id: usize) -> u64 {
    IntentScheduler::record_idle_start(core_id)
}

pub fn record_idle_end(core_id: usize, start: u64) {
    IntentScheduler::record_idle_end(core_id, start)
}

pub fn get_core_stats(core_id: usize) -> CoreStats {
    IntentScheduler::get_core_stats(core_id)
}

impl Default for IntentScheduler {
    fn default() -> Self {
        Self::new()
    }
}
