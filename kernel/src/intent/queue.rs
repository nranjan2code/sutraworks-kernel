//! Intent Queue
//!
//! Priority queue for pending intents. Enables:
//! - Deferred intent execution
//! - Priority-based ordering
//! - Rate limiting
//!
//! # Design
//! Fixed-size heap-free priority queue. Higher priority intents
//! execute first. Same priority = FIFO order.

use crate::intent::{ConceptID, Intent, IntentData};

// ═══════════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum queued intents
pub const QUEUE_SIZE: usize = 32;

/// Priority levels
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Priority {
    /// Background tasks
    Low = 0,
    /// Normal user actions  
    Normal = 1,
    /// Time-sensitive actions
    High = 2,
    /// System-critical (interrupts, errors)
    Critical = 3,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// QUEUED INTENT
// ═══════════════════════════════════════════════════════════════════════════════

/// An intent waiting in the queue
#[derive(Clone)]
pub struct QueuedIntent {
    /// The intent
    pub intent: Intent,
    /// Priority level
    pub priority: Priority,
    /// Sequence number (for FIFO within same priority)
    pub sequence: u64,
    /// Timestamp when queued
    pub queued_at: u64,
    /// Optional deadline (0 = no deadline)
    pub deadline: u64,
}

impl QueuedIntent {
    /// Empty placeholder
    pub const EMPTY: Self = Self {
        intent: Intent {
            concept_id: ConceptID(0),
            confidence: 0.0,
            data: IntentData::None,
        },
        priority: Priority::Low,
        sequence: 0,
        queued_at: 0,
        deadline: 0,
    };
    
    /// Check if this entry is empty
    pub fn is_empty(&self) -> bool {
        self.intent.concept_id.0 == 0
    }
    
    /// Check if deadline has passed
    pub fn is_expired(&self, now: u64) -> bool {
        self.deadline > 0 && now > self.deadline
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTENT QUEUE
// ═══════════════════════════════════════════════════════════════════════════════

/// Priority queue for intents
pub struct IntentQueue {
    /// Queue storage
    entries: [QueuedIntent; QUEUE_SIZE],
    /// Number of valid entries
    count: usize,
    /// Sequence counter (for FIFO ordering)
    next_sequence: u64,
}

impl IntentQueue {
    /// Create an empty queue
    pub const fn new() -> Self {
        Self {
            entries: [QueuedIntent::EMPTY; QUEUE_SIZE],
            count: 0,
            next_sequence: 0,
        }
    }
    
    /// Push an intent with default priority
    pub fn push(&mut self, intent: Intent, timestamp: u64) -> bool {
        self.push_with_priority(intent, Priority::Normal, timestamp, 0)
    }
    
    /// Push an intent with specific priority
    pub fn push_with_priority(
        &mut self,
        intent: Intent,
        priority: Priority,
        timestamp: u64,
        deadline: u64,
    ) -> bool {
        if self.count >= QUEUE_SIZE {
            // Queue full - try to drop lowest priority
            if !self.drop_lowest(priority) {
                return false;
            }
        }
        
        let queued = QueuedIntent {
            intent,
            priority,
            sequence: self.next_sequence,
            queued_at: timestamp,
            deadline,
        };
        
        self.next_sequence += 1;
        
        // Find insertion point (maintain heap property)
        self.entries[self.count] = queued;
        self.count += 1;
        self.sift_up(self.count - 1);
        
        true
    }
    
    /// Pop the highest priority intent
    pub fn pop(&mut self) -> Option<QueuedIntent> {
        if self.count == 0 {
            return None;
        }
        
        // Swap root with last element
        self.entries.swap(0, self.count - 1);
        self.count -= 1;
        
        // Get the removed element
        let result = self.entries[self.count].clone();
        self.entries[self.count] = QueuedIntent::EMPTY;
        
        // Restore heap property
        if self.count > 0 {
            self.sift_down(0);
        }
        
        Some(result)
    }
    
    /// Peek at the highest priority intent without removing
    pub fn peek(&self) -> Option<&QueuedIntent> {
        if self.count == 0 {
            None
        } else {
            Some(&self.entries[0])
        }
    }
    
    /// Remove expired intents
    pub fn remove_expired(&mut self, now: u64) -> usize {
        let mut removed = 0;
        let mut i = 0;
        
        while i < self.count {
            if self.entries[i].is_expired(now) {
                // Swap with last and reduce count
                self.entries.swap(i, self.count - 1);
                self.entries[self.count - 1] = QueuedIntent::EMPTY;
                self.count -= 1;
                removed += 1;
                // Don't increment i - check the swapped element
            } else {
                i += 1;
            }
        }
        
        // Rebuild heap if we removed anything
        if removed > 0 {
            self.heapify();
        }
        
        removed
    }
    
    /// Get queue length
    pub fn len(&self) -> usize {
        self.count
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    
    /// Check if full
    pub fn is_full(&self) -> bool {
        self.count >= QUEUE_SIZE
    }
    
    /// Clear the queue
    pub fn clear(&mut self) {
        for i in 0..self.count {
            self.entries[i] = QueuedIntent::EMPTY;
        }
        self.count = 0;
    }
    
    // ═══════════════════════════════════════════════════════════════════════════
    // HEAP OPERATIONS
    // ═══════════════════════════════════════════════════════════════════════════
    
    /// Compare two entries (higher priority or earlier sequence wins)
    fn compare(&self, a: usize, b: usize) -> bool {
        let ea = &self.entries[a];
        let eb = &self.entries[b];
        
        if ea.priority != eb.priority {
            ea.priority > eb.priority
        } else {
            ea.sequence < eb.sequence // Earlier sequence = higher priority
        }
    }
    
    /// Sift up to maintain heap property
    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.compare(idx, parent) {
                self.entries.swap(idx, parent);
                idx = parent;
            } else {
                break;
            }
        }
    }
    
    /// Sift down to maintain heap property
    fn sift_down(&mut self, mut idx: usize) {
        loop {
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            let mut largest = idx;
            
            if left < self.count && self.compare(left, largest) {
                largest = left;
            }
            if right < self.count && self.compare(right, largest) {
                largest = right;
            }
            
            if largest != idx {
                self.entries.swap(idx, largest);
                idx = largest;
            } else {
                break;
            }
        }
    }
    
    /// Rebuild entire heap
    fn heapify(&mut self) {
        for i in (0..self.count / 2).rev() {
            self.sift_down(i);
        }
    }
    
    /// Drop lowest priority entry to make room
    fn drop_lowest(&mut self, min_priority: Priority) -> bool {
        // Find lowest priority entry
        let mut lowest_idx = None;
        let mut lowest_priority = min_priority;
        let mut latest_sequence = 0u64;
        
        for i in 0..self.count {
            let e = &self.entries[i];
            if e.priority < lowest_priority || 
               (e.priority == lowest_priority && e.sequence > latest_sequence) {
                lowest_idx = Some(i);
                lowest_priority = e.priority;
                latest_sequence = e.sequence;
            }
        }
        
        if let Some(idx) = lowest_idx {
            // Remove by swapping with last
            self.entries.swap(idx, self.count - 1);
            self.entries[self.count - 1] = QueuedIntent::EMPTY;
            self.count -= 1;
            self.heapify();
            true
        } else {
            false
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_queue_push_pop() {
        let mut queue = IntentQueue::new();
        
        queue.push(Intent::new(ConceptID(1)), 100);
        queue.push(Intent::new(ConceptID(2)), 200);
        
        assert_eq!(queue.len(), 2);
        
        // FIFO for same priority
        let first = queue.pop().unwrap();
        assert_eq!(first.intent.concept_id.0, 1);
        
        let second = queue.pop().unwrap();
        assert_eq!(second.intent.concept_id.0, 2);
        
        assert!(queue.pop().is_none());
    }
    
    #[test]
    fn test_queue_priority() {
        let mut queue = IntentQueue::new();
        
        // Push low priority first
        queue.push_with_priority(Intent::new(ConceptID(1)), Priority::Low, 100, 0);
        
        // Push high priority second
        queue.push_with_priority(Intent::new(ConceptID(2)), Priority::High, 200, 0);
        
        // Push critical third
        queue.push_with_priority(Intent::new(ConceptID(3)), Priority::Critical, 300, 0);
        
        // Should pop in priority order: Critical, High, Low
        assert_eq!(queue.pop().unwrap().intent.concept_id.0, 3);
        assert_eq!(queue.pop().unwrap().intent.concept_id.0, 2);
        assert_eq!(queue.pop().unwrap().intent.concept_id.0, 1);
    }
    
    #[test]
    fn test_queue_deadline() {
        let mut queue = IntentQueue::new();
        
        queue.push_with_priority(Intent::new(ConceptID(1)), Priority::Normal, 100, 150);
        queue.push_with_priority(Intent::new(ConceptID(2)), Priority::Normal, 100, 0); // No deadline
        
        // At time 200, first should be expired
        let removed = queue.remove_expired(200);
        assert_eq!(removed, 1);
        assert_eq!(queue.len(), 1);
        
        // Remaining should be the one without deadline
        assert_eq!(queue.peek().unwrap().intent.concept_id.0, 2);
    }
    
    #[test]
    fn test_queue_full() {
        let mut queue = IntentQueue::new();
        
        // Fill the queue with low priority
        for i in 0..QUEUE_SIZE {
            queue.push_with_priority(
                Intent::new(ConceptID(i as u64)),
                Priority::Low,
                i as u64,
                0,
            );
        }
        
        assert!(queue.is_full());
        
        // Should be able to push high priority (drops a low)
        assert!(queue.push_with_priority(
            Intent::new(ConceptID(999)),
            Priority::High,
            1000,
            0,
        ));
        
        // High priority should be at front
        assert_eq!(queue.peek().unwrap().intent.concept_id.0, 999);
    }
}
