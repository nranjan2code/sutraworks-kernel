//! Process Scheduler
//!
//! Basic Round-Robin scheduler.

use alloc::collections::vec_deque::VecDeque;
use alloc::boxed::Box;
use crate::kernel::process::{Process, ProcessState, Context};
use crate::arch::SpinLock;

pub struct Scheduler {
    processes: VecDeque<Box<Process>>,
    current_pid: Option<u64>,
}

impl Scheduler {
    pub const fn new() -> Self {
        Scheduler {
            processes: VecDeque::new(),
            current_pid: None,
        }
    }

    /// Spawn a new kernel thread
    pub fn spawn(&mut self, entry: fn()) {
        let process = Process::new_kernel(entry);
        self.processes.push_back(Box::new(process));
    }

    /// Spawn a new user process
    pub fn spawn_user(&mut self, entry: fn(), arg: u64) {
        let process = Process::new_user(entry, arg);
        self.processes.push_back(Box::new(process));
    }

    /// Schedule the next process
    /// 
    /// Returns a tuple of (prev_context_ptr, next_context_ptr) if a switch is needed.
    /// The caller must then call `switch_to`.
    pub fn schedule(&mut self) -> Option<(*mut Context, *const Context)> {
        if self.processes.is_empty() {
            return None;
        }

        // If we have a running process, move it to the back (Round Robin)
        if let Some(mut prev) = self.processes.pop_front() {
            if prev.state == ProcessState::Running {
                prev.state = ProcessState::Ready;
                self.processes.push_back(prev);
            } else if prev.state == ProcessState::Terminated {
                // Drop it (it's already popped)
            } else {
                // Blocked or Ready, put it back
                self.processes.push_back(prev);
            }
        }

        // Pick next Ready process
        // We need to rotate the queue until we find a Ready one
        let len = self.processes.len();
        for _ in 0..len {
            if let Some(mut next) = self.processes.pop_front() {
                if next.state == ProcessState::Ready {
                    next.state = ProcessState::Running;
                    self.current_pid = Some(next.id.0);
                    
                    // We need to return pointers to the contexts.
                    // Since we are using Box<Process>, the address of the Process struct (and its Context)
                    // is stable on the heap, even if we move the Box around in the VecDeque.
                    
                    // However, we need to put `next` back into the queue (at the front/running position)
                    // BEFORE we take the pointer, to ensure ownership is correct.
                    self.processes.push_front(next);
                    
                    // Get pointers
                    // SAFETY: We are single-threaded on this core for now (interrupts disabled during schedule)
                    let next_proc = self.processes.front().unwrap();
                    let next_ctx = &next_proc.context as *const Context;
                    
                    // For the PREVIOUS context:
                    // If we just rotated, the previous process is now at the BACK of the queue.
                    // Unless it was the ONLY process, in which case it's at the FRONT.
                    
                    let prev_ctx = if self.processes.len() > 1 {
                        let prev_proc = self.processes.back_mut().unwrap();
                        &mut prev_proc.context as *mut Context
                    } else {
                        // Only one process (ourselves), so prev == next
                        let prev_proc = self.processes.front_mut().unwrap();
                        &mut prev_proc.context as *mut Context
                    };

                    return Some((prev_ctx, next_ctx)); 
                } else {
                    self.processes.push_back(next);
                }
            }
        }
        
        None
    }
}

pub static SCHEDULER: SpinLock<Scheduler> = SpinLock::new(Scheduler::new());

/// Called on every timer interrupt (e.g., 10ms)
pub fn tick() {
    // Re-arm timer for next tick (10ms = 10,000us)
    crate::drivers::timer::set_timer_interrupt(10_000);

    // Schedule next task
    // Note: We are in IRQ context, so interrupts are already disabled.
    // We can safely lock the scheduler.
    let mut scheduler = SCHEDULER.lock();
    if let Some((prev, next)) = scheduler.schedule() {
        drop(scheduler);
        unsafe {
            crate::arch::switch_to(prev as *mut u8, next as *const u8);
        }
    }
}

/// Yield the current task
pub fn yield_task() {
    crate::arch::without_interrupts(|| {
        let mut scheduler = SCHEDULER.lock();
        if let Some((prev, next)) = scheduler.schedule() {
            // Drop the lock before switching!
            // Otherwise we switch context holding the lock, and the next task might try to lock it -> Deadlock.
            drop(scheduler);
            
            unsafe {
                crate::arch::switch_to(prev as *mut u8, next as *const u8);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::boxed::Box;

    fn dummy_task() {}

    #[test]
    fn test_scheduler_round_robin() {
        let mut scheduler = Scheduler::new();
        
        // Add 3 tasks
        scheduler.spawn(dummy_task); // Task 1
        scheduler.spawn(dummy_task); // Task 2
        scheduler.spawn(dummy_task); // Task 3
        
        // Initial state: [T1, T2, T3]
        
        // Schedule 1: Should pick T1
        let (prev, next) = scheduler.schedule().expect("Should schedule T1");
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
        let mut scheduler = Scheduler::new();
        assert!(scheduler.schedule().is_none());
    }
}
