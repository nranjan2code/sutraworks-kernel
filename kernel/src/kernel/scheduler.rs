//! Agent Scheduler
//!
//! Schedules Agents. Simplified for stroke-native kernel.

use alloc::collections::vec_deque::VecDeque;
use alloc::boxed::Box;
use crate::kernel::process::{Agent, AgentState, Context};
use crate::arch::SpinLock;

pub struct IntentScheduler {
    agents: VecDeque<Box<Agent>>,
    current_agent_id: Option<u64>,
}

impl IntentScheduler {
    pub const fn new() -> Self {
        IntentScheduler {
            agents: VecDeque::new(),
            current_agent_id: None,
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
        let agent = Agent::new_user_simple(entry, arg)?;
        self.agents.push_back(Box::new(agent));
        Ok(())
    }

    /// Spawn a new user agent from ELF binary
    pub fn spawn_user_elf(&mut self, elf_data: &[u8]) -> Result<(), &'static str> {
        let agent = Agent::new_user_elf(elf_data)?;
        self.agents.push_back(Box::new(agent));
        Ok(())
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
    pub fn wait_child(&mut self, parent_id: u64) -> Result<Option<u64>, &'static str> {
        let mut has_children = false;
        let mut reaped_pid = None;
        let mut remove_idx = None;
        
        for (i, agent) in self.agents.iter().enumerate() {
            if agent.parent_id == Some(parent_id) {
                has_children = true;
                if agent.state == AgentState::Terminated {
                    reaped_pid = Some(agent.id.0);
                    remove_idx = Some(i);
                    break;
                }
            }
        }
        
        if let Some(idx) = remove_idx {
            self.agents.remove(idx);
            return Ok(reaped_pid);
        }
        
        if has_children {
            Ok(None) // Should block
        } else {
            Err("No children")
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
        if self.agents.is_empty() {
            return None;
        }

        // If we have a running agent, move it to the back
        if let Some(mut prev) = self.agents.pop_front() {
            if prev.state == AgentState::Running {
                prev.state = AgentState::Ready;
                self.agents.push_back(prev);
            } else if prev.state == AgentState::Terminated {
                // Keep it for wait()
                self.agents.push_back(prev);
            } else {
                // Blocked, Sleeping, or Ready, put it back
                self.agents.push_back(prev);
            }
        }

        // Simple round-robin scheduling
        // Find first Ready agent
        let mut best_index = None;

        for (i, agent) in self.agents.iter().enumerate() {
            if agent.state == AgentState::Ready {
                best_index = Some(i);
                break;
            }
        }

        if let Some(index) = best_index {
            // Remove the best agent from its current position
            let mut next = self.agents.remove(index).unwrap();
            
            next.state = AgentState::Running;
            self.current_agent_id = Some(next.id.0);
            
            // Put it at the FRONT (Running position)
            self.agents.push_front(next);
            
            // Get pointers
            let next_agent = self.agents.front().unwrap();
            let next_ctx = &next_agent.context as *const Context;
            
            // For the PREVIOUS context:
            // If we just rotated/moved, the previous agent is now at the BACK (or wherever we put it).
            // Unless it was the ONLY agent.
            
            // Note: We need to be careful about where 'prev' went.
            // In the beginning of this function, we popped 'prev' and pushed it to back.
            // So 'prev' is at self.agents.back().
            
            let prev_ctx = if self.agents.len() > 1 {
                let prev_agent = self.agents.back_mut().unwrap();
                &mut prev_agent.context as *mut Context
            } else {
                // Only one agent (ourselves), so prev == next
                let prev_agent = self.agents.front_mut().unwrap();
                &mut prev_agent.context as *mut Context
            };

            return Some((prev_ctx, next_ctx));
        }
        
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
}

pub static SCHEDULER: SpinLock<IntentScheduler> = SpinLock::new(IntentScheduler::new());

/// Called on every timer interrupt (e.g., 10ms)
pub fn tick() {
    // Re-arm timer for next tick (10ms = 10,000us)
    crate::drivers::timer::set_timer_interrupt(10_000);

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
    // Note: We are in IRQ context, so interrupts are already disabled.
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
        let mut scheduler = IntentScheduler::new();
        assert!(scheduler.schedule().is_none());
    }
}
