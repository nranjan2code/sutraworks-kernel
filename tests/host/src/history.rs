//! Host-Based Tests for Stroke History Buffer
//!
//! Tests the ring buffer implementation for stroke history.

use crate::stroke::Stroke;

// ═══════════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum history entries (power of 2 for fast modulo)
pub const HISTORY_SIZE: usize = 64;

const HISTORY_MASK: usize = HISTORY_SIZE - 1;

// ═══════════════════════════════════════════════════════════════════════════════
// HISTORY ENTRY
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, Debug)]
pub struct HistoryEntry {
    pub stroke: Stroke,
    pub intent_id: Option<u64>,
    pub timestamp: u64,
    pub undone: bool,
}

impl HistoryEntry {
    pub const EMPTY: Self = Self {
        stroke: Stroke::from_raw(0),
        intent_id: None,
        timestamp: 0,
        undone: false,
    };
    
    pub fn new(stroke: Stroke, intent_id: Option<u64>, timestamp: u64) -> Self {
        Self {
            stroke,
            intent_id,
            timestamp,
            undone: false,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// STROKE HISTORY
// ═══════════════════════════════════════════════════════════════════════════════

pub struct StrokeHistory {
    entries: [HistoryEntry; HISTORY_SIZE],
    head: usize,
    count: usize,
    undo_cursor: usize,
}

impl StrokeHistory {
    pub fn new() -> Self {
        Self {
            entries: [HistoryEntry::EMPTY; HISTORY_SIZE],
            head: 0,
            count: 0,
            undo_cursor: 0,
        }
    }
    
    pub fn push(&mut self, stroke: Stroke, intent_id: Option<u64>, timestamp: u64) {
        self.entries[self.head] = HistoryEntry::new(stroke, intent_id, timestamp);
        self.head = (self.head + 1) & HISTORY_MASK;
        
        if self.count < HISTORY_SIZE {
            self.count += 1;
        }
        
        self.undo_cursor = 0;
    }
    
    pub fn last(&self) -> Option<&HistoryEntry> {
        if self.count == 0 {
            return None;
        }
        let idx = (self.head + HISTORY_SIZE - 1) & HISTORY_MASK;
        Some(&self.entries[idx])
    }
    
    pub fn at(&self, offset: usize) -> Option<&HistoryEntry> {
        if offset >= self.count {
            return None;
        }
        let idx = (self.head + HISTORY_SIZE - 1 - offset) & HISTORY_MASK;
        Some(&self.entries[idx])
    }
    
    pub fn undo(&mut self) -> Option<&HistoryEntry> {
        for offset in self.undo_cursor..self.count {
            let idx = (self.head + HISTORY_SIZE - 1 - offset) & HISTORY_MASK;
            if !self.entries[idx].undone {
                self.entries[idx].undone = true;
                self.undo_cursor = offset + 1;
                return Some(&self.entries[idx]);
            }
        }
        None
    }
    
    pub fn redo(&mut self) -> Option<&HistoryEntry> {
        if self.undo_cursor == 0 {
            return None;
        }
        
        for offset in (0..self.undo_cursor).rev() {
            let idx = (self.head + HISTORY_SIZE - 1 - offset) & HISTORY_MASK;
            if self.entries[idx].undone {
                self.entries[idx].undone = false;
                self.undo_cursor = offset;
                return Some(&self.entries[idx]);
            }
        }
        None
    }
    
    pub fn len(&self) -> usize {
        self.count
    }
    
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    
    pub fn clear(&mut self) {
        self.head = 0;
        self.count = 0;
        self.undo_cursor = 0;
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_history_empty() {
        let history = StrokeHistory::new();
        assert!(history.is_empty());
        assert_eq!(history.len(), 0);
        assert!(history.last().is_none());
    }
    
    #[test]
    fn test_history_push_single() {
        let mut history = StrokeHistory::new();
        let stroke = Stroke::from_raw(0x01);
        
        history.push(stroke, Some(100), 1000);
        
        assert_eq!(history.len(), 1);
        assert!(!history.is_empty());
        
        let last = history.last().unwrap();
        assert_eq!(last.stroke.raw(), 0x01);
        assert_eq!(last.intent_id, Some(100));
        assert_eq!(last.timestamp, 1000);
    }
    
    #[test]
    fn test_history_push_multiple() {
        let mut history = StrokeHistory::new();
        
        for i in 0..5 {
            history.push(Stroke::from_raw(i), Some(i as u64), i as u64 * 100);
        }
        
        assert_eq!(history.len(), 5);
        
        // Most recent is 4
        assert_eq!(history.last().unwrap().stroke.raw(), 4);
        
        // Check indexing
        assert_eq!(history.at(0).unwrap().stroke.raw(), 4);
        assert_eq!(history.at(1).unwrap().stroke.raw(), 3);
        assert_eq!(history.at(4).unwrap().stroke.raw(), 0);
        assert!(history.at(5).is_none());
    }
    
    #[test]
    fn test_history_ring_buffer_wraparound() {
        let mut history = StrokeHistory::new();
        
        // Fill beyond capacity
        for i in 0..100 {
            history.push(Stroke::from_raw(i), None, i as u64);
        }
        
        // Should only keep HISTORY_SIZE entries
        assert_eq!(history.len(), HISTORY_SIZE);
        
        // Most recent should be 99
        assert_eq!(history.last().unwrap().stroke.raw(), 99);
        
        // Oldest should be 100 - HISTORY_SIZE = 36
        assert_eq!(history.at(HISTORY_SIZE - 1).unwrap().stroke.raw(), 100 - HISTORY_SIZE as u32);
    }
    
    #[test]
    fn test_history_undo_single() {
        let mut history = StrokeHistory::new();
        
        history.push(Stroke::from_raw(1), Some(101), 100);
        history.push(Stroke::from_raw(2), Some(102), 200);
        history.push(Stroke::from_raw(3), Some(103), 300);
        
        // Undo most recent
        let undone = history.undo().unwrap();
        assert_eq!(undone.stroke.raw(), 3);
        assert!(undone.undone);
    }
    
    #[test]
    fn test_history_undo_multiple() {
        let mut history = StrokeHistory::new();
        
        history.push(Stroke::from_raw(1), None, 100);
        history.push(Stroke::from_raw(2), None, 200);
        history.push(Stroke::from_raw(3), None, 300);
        
        // Undo all three
        assert_eq!(history.undo().unwrap().stroke.raw(), 3);
        assert_eq!(history.undo().unwrap().stroke.raw(), 2);
        assert_eq!(history.undo().unwrap().stroke.raw(), 1);
        
        // No more to undo
        assert!(history.undo().is_none());
    }
    
    #[test]
    fn test_history_redo() {
        let mut history = StrokeHistory::new();
        
        history.push(Stroke::from_raw(1), None, 100);
        history.push(Stroke::from_raw(2), None, 200);
        history.push(Stroke::from_raw(3), None, 300);
        
        // Undo two
        history.undo();
        history.undo();
        
        // Redo one
        let redone = history.redo().unwrap();
        assert_eq!(redone.stroke.raw(), 2);
        assert!(!redone.undone);
    }
    
    #[test]
    fn test_history_undo_redo_interleave() {
        let mut history = StrokeHistory::new();
        
        history.push(Stroke::from_raw(1), None, 100);
        history.push(Stroke::from_raw(2), None, 200);
        history.push(Stroke::from_raw(3), None, 300);
        
        // Undo
        history.undo(); // 3 is undone
        
        // Redo
        history.redo(); // 3 is restored
        
        // Undo again
        history.undo(); // 3 is undone again
        history.undo(); // 2 is undone
        
        // Redo
        let redone = history.redo().unwrap();
        assert_eq!(redone.stroke.raw(), 2);
    }
    
    #[test]
    fn test_history_new_stroke_resets_undo() {
        let mut history = StrokeHistory::new();
        
        history.push(Stroke::from_raw(1), None, 100);
        history.push(Stroke::from_raw(2), None, 200);
        
        // Undo
        history.undo();
        
        // New stroke resets undo cursor
        history.push(Stroke::from_raw(3), None, 300);
        
        // Can't redo now (new stroke invalidated it)
        // The implementation resets undo_cursor to 0 on push
        assert!(history.redo().is_none());
    }
    
    #[test]
    fn test_history_clear() {
        let mut history = StrokeHistory::new();
        
        history.push(Stroke::from_raw(1), None, 100);
        history.push(Stroke::from_raw(2), None, 200);
        
        history.clear();
        
        assert!(history.is_empty());
        assert!(history.last().is_none());
    }
    
    #[test]
    fn test_history_with_intents() {
        let mut history = StrokeHistory::new();
        
        history.push(Stroke::from_raw(1), Some(0x0001), 100);
        history.push(Stroke::from_raw(2), None, 200); // No intent
        history.push(Stroke::from_raw(3), Some(0x0003), 300);
        
        assert_eq!(history.at(0).unwrap().intent_id, Some(0x0003));
        assert_eq!(history.at(1).unwrap().intent_id, None);
        assert_eq!(history.at(2).unwrap().intent_id, Some(0x0001));
    }
}
