//! SMP (Symmetric Multiprocessing) Scheduler
//!
//! Multi-core scheduler with per-core run queues and work-stealing load balancing.
//!
//! # Architecture
//! - **Per-Core Run Queues**: Each core has its own lock-free run queue (minimizes contention)
//! - **Work Stealing**: Idle cores steal tasks from busy cores
//! - **Core Affinity**: Tasks can be pinned to specific cores (real-time steno processing)
//! - **Priority Scheduling**: High-priority tasks (perception, steno) run first
//!
//! # Design Goals
//! - **Scalability**: 4 cores on Raspberry Pi 5
//! - **Low Latency**: Sub-millisecond context switching
//! - **Fair**: Balance load across all cores
//! - **Real-Time Support**: Core 0 dedicated to steno input (< 100μs latency)

use alloc::collections::vec_deque::VecDeque;
use alloc::boxed::Box;
use crate::kernel::process::{Agent, AgentState, Context};
use crate::arch::{self, SpinLock};

/// Maximum number of cores supported (Raspberry Pi 5 has 4)
const MAX_CORES: usize = 4;

/// Agent priority levels
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Priority {
    Idle = 0,       // Background tasks
    Normal = 1,     // Standard user tasks
    High = 2,       // Perception, async I/O
    Realtime = 3,   // Steno input (< 100μs latency required)
}

/// Core affinity mask
#[derive(Clone, Copy, Debug)]
pub struct AffinityMask {
    pub mask: u8,  // Bits 0-3 represent cores 0-3
}

impl AffinityMask {
    /// Can run on any core
    pub const ANY: Self = AffinityMask { mask: 0b1111 };

    /// Pinned to core 0 (steno input)
    pub const CORE0: Self = AffinityMask { mask: 0b0001 };

    /// Pinned to core 1 (perception - vision)
    pub const CORE1: Self = AffinityMask { mask: 0b0010 };

    /// Pinned to core 2 (perception - audio)
    pub const CORE2: Self = AffinityMask { mask: 0b0100 };

    /// Pinned to core 3 (general purpose)
    pub const CORE3: Self = AffinityMask { mask: 0b1000 };

    /// Check if task can run on given core
    pub fn can_run_on(&self, core_id: usize) -> bool {
        (self.mask & (1 << core_id)) != 0
    }

    /// Create mask for specific core
    pub const fn core(id: usize) -> Self {
        AffinityMask { mask: 1 << id }
    }
}

/// Extended Agent with SMP metadata
pub struct SmpAgent {
    pub agent: Agent,
    pub priority: Priority,
    pub affinity: AffinityMask,
    pub last_core: usize,  // For cache affinity
    pub cpu_time: u64,     // Microseconds of CPU time consumed
}

impl SmpAgent {
    pub fn new(agent: Agent, priority: Priority, affinity: AffinityMask) -> Self {
        Self {
            agent,
            priority,
            affinity,
            last_core: 0,
            cpu_time: 0,
        }
    }
}

/// Per-Core Run Queue
pub struct CoreQueue {
    core_id: usize,
    queue: VecDeque<Box<SmpAgent>>,
    current: Option<Box<SmpAgent>>,
    idle_time: u64,  // Microseconds spent idle
}

impl CoreQueue {
    pub const fn new(core_id: usize) -> Self {
        Self {
            core_id,
            queue: VecDeque::new(),
            current: None,
            idle_time: 0,
        }
    }

    /// Add task to run queue (respects priority)
    pub fn enqueue(&mut self, mut task: Box<SmpAgent>) {
        task.last_core = self.core_id;

        // Find insertion point based on priority (higher priority goes first)
        let mut insert_idx = self.queue.len();
        for (i, existing) in self.queue.iter().enumerate() {
            if task.priority > existing.priority {
                insert_idx = i;
                break;
            }
        }

        self.queue.insert(insert_idx, task);
    }

    /// Remove and return highest priority task
    pub fn dequeue(&mut self) -> Option<Box<SmpAgent>> {
        self.queue.pop_front()
    }

    /// Peek at next task without removing
    pub fn peek(&self) -> Option<&SmpAgent> {
        self.queue.front().map(|b| b.as_ref())
    }

    /// Get queue length
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Steal half the tasks from this queue (for work stealing)
    /// Returns tasks from the back (lower priority)
    pub fn steal_half(&mut self) -> VecDeque<Box<SmpAgent>> {
        let steal_count = self.queue.len() / 2;
        if steal_count == 0 {
            return VecDeque::new();
        }

        let mut stolen = VecDeque::new();
        for _ in 0..steal_count {
            if let Some(task) = self.queue.pop_back() {
                stolen.push_front(task);
            }
        }

        stolen
    }
}

/// SMP Scheduler (Global State)
pub struct SmpScheduler {
    cores: [SpinLock<CoreQueue>; MAX_CORES],
    num_cores: usize,
    total_tasks: usize,
}

impl SmpScheduler {
    pub const fn new() -> Self {
        Self {
            cores: [
                SpinLock::new(CoreQueue::new(0)),
                SpinLock::new(CoreQueue::new(1)),
                SpinLock::new(CoreQueue::new(2)),
                SpinLock::new(CoreQueue::new(3)),
            ],
            num_cores: 1,  // Will be updated during init
            total_tasks: 0,
        }
    }

    /// Initialize SMP scheduler and wake secondary cores
    pub fn init(&mut self) {
        // Detect number of cores (read MPIDR_EL1 or use known value for Pi 5)
        self.num_cores = 4;

        // Wake secondary cores (cores 1-3)
        for core_id in 1..self.num_cores {
            crate::kprintln!("[SMP] Waking core {}...", core_id);
            arch::start_core(core_id, secondary_core_entry);
        }

        crate::kprintln!("[SMP] Scheduler initialized with {} cores", self.num_cores);
    }

    /// Spawn a new task
    pub fn spawn(&mut self, agent: Agent, priority: Priority, affinity: AffinityMask) {
        let smp_agent = Box::new(SmpAgent::new(agent, priority, affinity));

        // Find best core for this task
        let target_core = self.select_core(priority, affinity);

        // Enqueue task
        self.cores[target_core].lock().enqueue(smp_agent);
        self.total_tasks += 1;

        crate::kprintln!("[SMP] Spawned task on core {} (priority: {:?})", target_core, priority);
    }

    /// Select best core for a new task
    fn select_core(&self, priority: Priority, affinity: AffinityMask) -> usize {
        // Filter cores by affinity
        let mut candidates = Vec::new();
        for core_id in 0..self.num_cores {
            if affinity.can_run_on(core_id) {
                candidates.push(core_id);
            }
        }

        if candidates.is_empty() {
            return 0;  // Fallback to core 0
        }

        // For real-time tasks, prefer core 0 (if allowed)
        if priority == Priority::Realtime && affinity.can_run_on(0) {
            return 0;
        }

        // Otherwise, choose core with smallest queue
        let mut best_core = candidates[0];
        let mut best_len = self.cores[best_core].lock().len();

        for &core_id in &candidates[1..] {
            let len = self.cores[core_id].lock().len();
            if len < best_len {
                best_len = len;
                best_core = core_id;
            }
        }

        best_core
    }

    /// Schedule next task on current core
    pub fn schedule(&mut self, core_id: usize) -> Option<(*mut Context, *const Context)> {
        if core_id >= self.num_cores {
            return None;
        }

        let mut core_queue = self.cores[core_id].lock();

        // Save current task back to queue if it's still running
        if let Some(mut current) = core_queue.current.take() {
            if current.agent.state == AgentState::Running {
                current.agent.state = AgentState::Ready;
                core_queue.enqueue(current);
            } else if current.agent.state == AgentState::Terminated {
                // Task finished, drop it
                self.total_tasks -= 1;
            } else {
                // Blocked or other state, re-enqueue
                core_queue.enqueue(current);
            }
        }

        // Get next task from local queue
        let mut next_task = core_queue.dequeue();

        // If local queue is empty, try work stealing
        if next_task.is_none() {
            drop(core_queue);  // Release lock before stealing
            next_task = self.steal_work(core_id);
            core_queue = self.cores[core_id].lock();  // Re-acquire
        }

        // If we found a task, prepare context switch
        if let Some(mut next) = next_task {
            next.agent.state = AgentState::Running;
            let next_ctx = &next.agent.context as *const Context;

            // Get previous context pointer
            let prev_ctx = if let Some(ref mut prev) = core_queue.current {
                &mut prev.agent.context as *mut Context
            } else {
                // First task on this core, use a dummy context
                // In practice, we'd have a per-core idle context
                core::ptr::null_mut()
            };

            core_queue.current = Some(next);

            if !prev_ctx.is_null() {
                return Some((prev_ctx, next_ctx));
            }
        }

        // No task available, core goes idle
        core_queue.idle_time += 10_000;  // 10ms tick
        None
    }

    /// Work stealing: steal tasks from other cores
    fn steal_work(&mut self, thief_core: usize) -> Option<Box<SmpAgent>> {
        // Try to steal from the busiest core
        let mut busiest_core = None;
        let mut max_len = 0;

        for core_id in 0..self.num_cores {
            if core_id == thief_core {
                continue;
            }

            let len = self.cores[core_id].lock().len();
            if len > max_len {
                max_len = len;
                busiest_core = Some(core_id);
            }
        }

        // Only steal if the busiest core has at least 2 tasks
        if let Some(victim_core) = busiest_core {
            if max_len >= 2 {
                let mut stolen_tasks = self.cores[victim_core].lock().steal_half();
                if let Some(task) = stolen_tasks.pop_front() {
                    // Put remaining stolen tasks in our queue
                    let mut our_queue = self.cores[thief_core].lock();
                    for task in stolen_tasks {
                        our_queue.enqueue(task);
                    }
                    return Some(task);
                }
            }
        }

        None
    }

    /// Get scheduler statistics
    pub fn stats(&self) -> SchedulerStats {
        let mut total_queue_len = 0;
        let mut per_core_lens = [0; MAX_CORES];

        for (i, core) in self.cores.iter().enumerate() {
            let len = core.lock().len();
            per_core_lens[i] = len;
            total_queue_len += len;
        }

        SchedulerStats {
            num_cores: self.num_cores,
            total_tasks: self.total_tasks,
            total_queue_len,
            per_core_lens,
        }
    }
}

pub struct SchedulerStats {
    pub num_cores: usize,
    pub total_tasks: usize,
    pub total_queue_len: usize,
    pub per_core_lens: [usize; MAX_CORES],
}

/// Global SMP scheduler instance
pub static SMP_SCHEDULER: SpinLock<SmpScheduler> = SpinLock::new(SmpScheduler::new());

/// Initialize SMP scheduler and wake secondary cores
pub fn init() {
    let mut scheduler = SMP_SCHEDULER.lock();
    scheduler.init();
}

/// Spawn a new task with priority and affinity
pub fn spawn_with_affinity(agent: Agent, priority: Priority, affinity: AffinityMask) {
    let mut scheduler = SMP_SCHEDULER.lock();
    scheduler.spawn(agent, priority, affinity);
}

/// Spawn a normal-priority task (can run on any core)
pub fn spawn(agent: Agent) {
    spawn_with_affinity(agent, Priority::Normal, AffinityMask::ANY);
}

/// Timer tick handler (called on each core)
pub fn tick() {
    let core_id = arch::core_id() as usize;

    // Re-arm timer for next tick (10ms = 10,000us)
    crate::drivers::timer::set_timer_interrupt(10_000);

    // Schedule next task on this core
    let mut scheduler = SMP_SCHEDULER.lock();
    if let Some((prev, next)) = scheduler.schedule(core_id) {
        drop(scheduler);  // Release lock before context switch!

        unsafe {
            crate::arch::switch_to(prev as *mut u8, next as *const u8);
        }
    }
}

/// Yield current task (voluntary preemption)
pub fn yield_task() {
    let core_id = arch::core_id() as usize;

    crate::arch::without_interrupts(|| {
        let mut scheduler = SMP_SCHEDULER.lock();
        if let Some((prev, next)) = scheduler.schedule(core_id) {
            drop(scheduler);

            unsafe {
                crate::arch::switch_to(prev as *mut u8, next as *const u8);
            }
        }
    });
}

/// Entry point for secondary cores (cores 1-3)
extern "C" fn secondary_core_entry() {
    let core_id = arch::core_id();
    crate::kprintln!("[SMP] Core {} started!", core_id);

    // Enable interrupts on this core
    unsafe { arch::enable_all_interrupts(); }

    // Enable timer interrupt for preemption
    crate::drivers::timer::set_timer_interrupt(10_000);

    // Idle loop - wait for tasks
    loop {
        arch::wfi();  // Wait for interrupt (timer or IPI)

        // Check if we have tasks to run
        let scheduler = SMP_SCHEDULER.lock();
        let has_tasks = !scheduler.cores[core_id as usize].lock().is_empty();
        drop(scheduler);

        if has_tasks {
            tick();  // Schedule a task
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_affinity_mask() {
        assert!(AffinityMask::ANY.can_run_on(0));
        assert!(AffinityMask::ANY.can_run_on(3));

        assert!(AffinityMask::CORE0.can_run_on(0));
        assert!(!AffinityMask::CORE0.can_run_on(1));

        assert!(AffinityMask::CORE2.can_run_on(2));
        assert!(!AffinityMask::CORE2.can_run_on(0));
    }

    #[test]
    fn test_work_stealing() {
        let mut scheduler = SmpScheduler::new();
        scheduler.num_cores = 2;

        // Load core 0 with 4 tasks
        for _ in 0..4 {
            let agent = Agent::new_kernel_simple(|| {});
            let smp_agent = Box::new(SmpAgent::new(agent, Priority::Normal, AffinityMask::ANY));
            scheduler.cores[0].lock().enqueue(smp_agent);
        }

        // Core 1 steals work
        let stolen = scheduler.steal_work(1);
        assert!(stolen.is_some());

        // Core 0 should have ~2 tasks left, core 1 should have received some
        let core0_len = scheduler.cores[0].lock().len();
        assert!(core0_len >= 1 && core0_len <= 3);
    }

    #[test]
    fn test_priority_scheduling() {
        let mut queue = CoreQueue::new(0);

        // Add tasks in mixed priority order
        queue.enqueue(Box::new(SmpAgent::new(
            Agent::new_kernel_simple(|| {}),
            Priority::Normal,
            AffinityMask::ANY,
        )));

        queue.enqueue(Box::new(SmpAgent::new(
            Agent::new_kernel_simple(|| {}),
            Priority::Realtime,
            AffinityMask::ANY,
        )));

        queue.enqueue(Box::new(SmpAgent::new(
            Agent::new_kernel_simple(|| {}),
            Priority::Idle,
            AffinityMask::ANY,
        )));

        // Dequeue should return Realtime first
        let first = queue.dequeue().unwrap();
        assert_eq!(first.priority, Priority::Realtime);

        // Then Normal
        let second = queue.dequeue().unwrap();
        assert_eq!(second.priority, Priority::Normal);

        // Finally Idle
        let third = queue.dequeue().unwrap();
        assert_eq!(third.priority, Priority::Idle);
    }
}
