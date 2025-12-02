//! Host-Based Tests for Intent Queue
//!
//! Tests the priority queue implementation for deferred intents.

// ═══════════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════════

pub const QUEUE_SIZE: usize = 32;

// ═══════════════════════════════════════════════════════════════════════════════
// PRIORITY
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
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

#[derive(Clone, Debug)]
pub struct QueuedIntent {
    pub concept_id: u64,
    pub priority: Priority,
    pub sequence: u64,
    pub queued_at: u64,
    pub deadline: u64,
}

impl QueuedIntent {
    pub const EMPTY: Self = Self {
        concept_id: 0,
        priority: Priority::Low,
        sequence: 0,
        queued_at: 0,
        deadline: 0,
    };
    
    pub fn new(concept_id: u64, priority: Priority, sequence: u64, queued_at: u64, deadline: u64) -> Self {
        Self { concept_id, priority, sequence, queued_at, deadline }
    }
    
    pub fn is_empty(&self) -> bool {
        self.concept_id == 0
    }
    
    pub fn is_expired(&self, now: u64) -> bool {
        self.deadline > 0 && now > self.deadline
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTENT QUEUE
// ═══════════════════════════════════════════════════════════════════════════════

pub struct IntentQueue {
    entries: Vec<QueuedIntent>,
    next_sequence: u64,
}

impl IntentQueue {
    pub fn new() -> Self {
        Self {
            entries: Vec::with_capacity(QUEUE_SIZE),
            next_sequence: 0,
        }
    }
    
    pub fn push(&mut self, concept_id: u64, timestamp: u64) -> bool {
        self.push_with_priority(concept_id, Priority::Normal, timestamp, 0)
    }
    
    pub fn push_with_priority(
        &mut self,
        concept_id: u64,
        priority: Priority,
        timestamp: u64,
        deadline: u64,
    ) -> bool {
        if self.entries.len() >= QUEUE_SIZE {
            if !self.drop_lowest(priority) {
                return false;
            }
        }
        
        let queued = QueuedIntent::new(
            concept_id,
            priority,
            self.next_sequence,
            timestamp,
            deadline,
        );
        
        self.next_sequence += 1;
        self.entries.push(queued);
        self.sift_up(self.entries.len() - 1);
        
        true
    }
    
    pub fn pop(&mut self) -> Option<QueuedIntent> {
        if self.entries.is_empty() {
            return None;
        }
        
        let len = self.entries.len();
        self.entries.swap(0, len - 1);
        let result = self.entries.pop();
        
        if !self.entries.is_empty() {
            self.sift_down(0);
        }
        
        result
    }
    
    pub fn peek(&self) -> Option<&QueuedIntent> {
        self.entries.first()
    }
    
    pub fn remove_expired(&mut self, now: u64) -> usize {
        let before = self.entries.len();
        self.entries.retain(|e| !e.is_expired(now));
        let removed = before - self.entries.len();
        
        if removed > 0 {
            self.heapify();
        }
        
        removed
    }
    
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    
    pub fn is_full(&self) -> bool {
        self.entries.len() >= QUEUE_SIZE
    }
    
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    
    fn compare(&self, a: usize, b: usize) -> bool {
        let ea = &self.entries[a];
        let eb = &self.entries[b];
        
        if ea.priority != eb.priority {
            ea.priority > eb.priority
        } else {
            ea.sequence < eb.sequence
        }
    }
    
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
    
    fn sift_down(&mut self, mut idx: usize) {
        loop {
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            let mut largest = idx;
            
            if left < self.entries.len() && self.compare(left, largest) {
                largest = left;
            }
            if right < self.entries.len() && self.compare(right, largest) {
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
    
    fn heapify(&mut self) {
        let len = self.entries.len();
        for i in (0..len / 2).rev() {
            self.sift_down(i);
        }
    }
    
    fn drop_lowest(&mut self, min_priority: Priority) -> bool {
        let mut lowest_idx = None;
        let mut lowest_priority = min_priority;
        let mut latest_sequence = 0u64;
        
        for (i, e) in self.entries.iter().enumerate() {
            if e.priority < lowest_priority || 
               (e.priority == lowest_priority && e.sequence > latest_sequence) {
                lowest_idx = Some(i);
                lowest_priority = e.priority;
                latest_sequence = e.sequence;
            }
        }
        
        if let Some(idx) = lowest_idx {
            self.entries.swap_remove(idx);
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
    fn test_queue_empty() {
        let queue = IntentQueue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert!(queue.peek().is_none());
    }
    
    #[test]
    fn test_queue_push_pop_single() {
        let mut queue = IntentQueue::new();
        
        assert!(queue.push(100, 1000));
        assert_eq!(queue.len(), 1);
        
        let item = queue.pop().unwrap();
        assert_eq!(item.concept_id, 100);
        assert!(queue.is_empty());
    }
    
    #[test]
    fn test_queue_fifo_same_priority() {
        let mut queue = IntentQueue::new();
        
        queue.push(1, 100);
        queue.push(2, 200);
        queue.push(3, 300);
        
        // Same priority = FIFO
        assert_eq!(queue.pop().unwrap().concept_id, 1);
        assert_eq!(queue.pop().unwrap().concept_id, 2);
        assert_eq!(queue.pop().unwrap().concept_id, 3);
    }
    
    #[test]
    fn test_queue_priority_ordering() {
        let mut queue = IntentQueue::new();
        
        queue.push_with_priority(1, Priority::Low, 100, 0);
        queue.push_with_priority(2, Priority::High, 200, 0);
        queue.push_with_priority(3, Priority::Normal, 300, 0);
        queue.push_with_priority(4, Priority::Critical, 400, 0);
        
        // Should pop in priority order: Critical, High, Normal, Low
        assert_eq!(queue.pop().unwrap().concept_id, 4);
        assert_eq!(queue.pop().unwrap().concept_id, 2);
        assert_eq!(queue.pop().unwrap().concept_id, 3);
        assert_eq!(queue.pop().unwrap().concept_id, 1);
    }
    
    #[test]
    fn test_queue_priority_with_fifo() {
        let mut queue = IntentQueue::new();
        
        // Two high priority items
        queue.push_with_priority(1, Priority::High, 100, 0);
        queue.push_with_priority(2, Priority::High, 200, 0);
        
        // One critical (should be first)
        queue.push_with_priority(3, Priority::Critical, 300, 0);
        
        // Critical first
        assert_eq!(queue.pop().unwrap().concept_id, 3);
        
        // Then high in FIFO order
        assert_eq!(queue.pop().unwrap().concept_id, 1);
        assert_eq!(queue.pop().unwrap().concept_id, 2);
    }
    
    #[test]
    fn test_queue_peek() {
        let mut queue = IntentQueue::new();
        
        queue.push(100, 1000);
        
        // Peek doesn't remove
        assert_eq!(queue.peek().unwrap().concept_id, 100);
        assert_eq!(queue.len(), 1);
        
        // Still there
        assert_eq!(queue.peek().unwrap().concept_id, 100);
    }
    
    #[test]
    fn test_queue_deadline_not_expired() {
        let mut queue = IntentQueue::new();
        
        queue.push_with_priority(1, Priority::Normal, 100, 500);
        queue.push_with_priority(2, Priority::Normal, 100, 0); // No deadline
        
        // At time 200, nothing expired
        let removed = queue.remove_expired(200);
        assert_eq!(removed, 0);
        assert_eq!(queue.len(), 2);
    }
    
    #[test]
    fn test_queue_deadline_expired() {
        let mut queue = IntentQueue::new();
        
        queue.push_with_priority(1, Priority::Normal, 100, 150); // Deadline 150
        queue.push_with_priority(2, Priority::Normal, 100, 250); // Deadline 250
        queue.push_with_priority(3, Priority::Normal, 100, 0);   // No deadline
        
        // At time 200, first should be expired
        let removed = queue.remove_expired(200);
        assert_eq!(removed, 1);
        assert_eq!(queue.len(), 2);
    }
    
    #[test]
    fn test_queue_full_drop_lowest() {
        let mut queue = IntentQueue::new();
        
        // Fill with low priority
        for i in 0..QUEUE_SIZE {
            queue.push_with_priority(i as u64, Priority::Low, i as u64, 0);
        }
        
        assert!(queue.is_full());
        
        // Push high priority - should succeed by dropping a low
        assert!(queue.push_with_priority(999, Priority::High, 1000, 0));
        
        // High priority should be at front
        assert_eq!(queue.peek().unwrap().concept_id, 999);
    }
    
    #[test]
    fn test_queue_full_cannot_insert_lower() {
        let mut queue = IntentQueue::new();
        
        // Fill with high priority
        for i in 0..QUEUE_SIZE {
            queue.push_with_priority(i as u64, Priority::High, i as u64, 0);
        }
        
        // Cannot insert low priority (nothing to drop)
        assert!(!queue.push_with_priority(999, Priority::Low, 1000, 0));
    }
    
    #[test]
    fn test_queue_clear() {
        let mut queue = IntentQueue::new();
        
        queue.push(1, 100);
        queue.push(2, 200);
        
        queue.clear();
        
        assert!(queue.is_empty());
        assert!(queue.pop().is_none());
    }
    
    #[test]
    fn test_queue_many_operations() {
        let mut queue = IntentQueue::new();
        
        // Interleaved push and pop
        queue.push_with_priority(1, Priority::Low, 100, 0);
        queue.push_with_priority(2, Priority::High, 200, 0);
        
        assert_eq!(queue.pop().unwrap().concept_id, 2); // High first
        
        queue.push_with_priority(3, Priority::Critical, 300, 0);
        queue.push_with_priority(4, Priority::Normal, 400, 0);
        
        assert_eq!(queue.pop().unwrap().concept_id, 3); // Critical
        assert_eq!(queue.pop().unwrap().concept_id, 4); // Normal
        assert_eq!(queue.pop().unwrap().concept_id, 1); // Low
    }
}
