//! Stroke History Buffer
//!
//! Maintains a circular buffer of recent strokes for:
//! - Undo/redo operations
//! - Multi-stroke context
//! - Stroke frequency tracking
//!
//! # Design
//! Fixed-size ring buffer (no alloc). Strokes are stored with timestamps
//! for temporal context. Target: Keep the strokes→intents flow pure.

use super::stroke::Stroke;
use crate::intent::Intent;

// ═══════════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum history entries (power of 2 for fast modulo)
pub const HISTORY_SIZE: usize = 64;

/// Mask for ring buffer indexing
const HISTORY_MASK: usize = HISTORY_SIZE - 1;

// ═══════════════════════════════════════════════════════════════════════════════
// HISTORY ENTRY
// ═══════════════════════════════════════════════════════════════════════════════

/// A single history entry
#[derive(Clone, Copy)]
pub struct HistoryEntry {
    /// The stroke that was processed
    pub stroke: Stroke,
    /// The intent it resolved to (if any)
    pub intent_id: Option<u64>,
    /// Timestamp (system ticks)
    pub timestamp: u64,
    /// Whether this stroke was undone
    pub undone: bool,
}

impl HistoryEntry {
    /// Empty entry (placeholder)
    pub const EMPTY: Self = Self {
        stroke: Stroke::EMPTY,
        intent_id: None,
        timestamp: 0,
        undone: false,
    };
    
    /// Create a new entry
    pub const fn new(stroke: Stroke, intent_id: Option<u64>, timestamp: u64) -> Self {
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

/// Ring buffer for stroke history
pub struct StrokeHistory {
    /// The buffer
    entries: [HistoryEntry; HISTORY_SIZE],
    /// Write position (next slot to write)
    head: usize,
    /// Number of valid entries
    count: usize,
    /// Undo cursor (for multi-undo)
    undo_cursor: usize,
}

impl StrokeHistory {
    /// Create an empty history
    pub const fn new() -> Self {
        Self {
            entries: [HistoryEntry::EMPTY; HISTORY_SIZE],
            head: 0,
            count: 0,
            undo_cursor: 0,
        }
    }
    
    /// Push a new stroke to history
    pub fn push(&mut self, stroke: Stroke, intent: Option<&Intent>, timestamp: u64) {
        let intent_id = intent.map(|i| i.concept_id.0);
        
        self.entries[self.head] = HistoryEntry::new(stroke, intent_id, timestamp);
        self.head = (self.head + 1) & HISTORY_MASK;
        
        if self.count < HISTORY_SIZE {
            self.count += 1;
        }
        
        // Reset undo cursor on new stroke
        self.undo_cursor = 0;
    }
    
    /// Get the most recent entry
    pub fn last(&self) -> Option<&HistoryEntry> {
        if self.count == 0 {
            return None;
        }
        let idx = (self.head + HISTORY_SIZE - 1) & HISTORY_MASK;
        Some(&self.entries[idx])
    }
    
    /// Get entry at offset from most recent (0 = most recent)
    pub fn at(&self, offset: usize) -> Option<&HistoryEntry> {
        if offset >= self.count {
            return None;
        }
        let idx = (self.head + HISTORY_SIZE - 1 - offset) & HISTORY_MASK;
        Some(&self.entries[idx])
    }
    
    /// Mark the most recent non-undone entry as undone
    /// Returns the undone entry if successful
    pub fn undo(&mut self) -> Option<&HistoryEntry> {
        // Find the next entry to undo
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
    
    /// Redo the most recently undone entry
    pub fn redo(&mut self) -> Option<&HistoryEntry> {
        if self.undo_cursor == 0 {
            return None;
        }
        
        // Find the most recently undone entry
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
    
    /// Get the number of entries
    pub fn len(&self) -> usize {
        self.count
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    
    /// Get the number of undone entries
    pub fn undo_count(&self) -> usize {
        self.entries[..self.count]
            .iter()
            .filter(|e| e.undone)
            .count()
    }
    
    /// Clear all history
    pub fn clear(&mut self) {
        self.head = 0;
        self.count = 0;
        self.undo_cursor = 0;
    }
    
    /// Get recent strokes (most recent first)
    pub fn recent(&self, max: usize) -> RecentStrokesIter<'_> {
        RecentStrokesIter {
            history: self,
            offset: 0,
            remaining: max.min(self.count),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ITERATOR
// ═══════════════════════════════════════════════════════════════════════════════

/// Iterator over recent strokes
pub struct RecentStrokesIter<'a> {
    history: &'a StrokeHistory,
    offset: usize,
    remaining: usize,
}

impl<'a> Iterator for RecentStrokesIter<'a> {
    type Item = &'a HistoryEntry;
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        
        let entry = self.history.at(self.offset);
        self.offset += 1;
        self.remaining -= 1;
        entry
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_history_push_and_last() {
        let mut history = StrokeHistory::new();
        
        let stroke1 = Stroke::from_raw(0x01);
        let stroke2 = Stroke::from_raw(0x02);
        
        history.push(stroke1, None, 100);
        assert_eq!(history.len(), 1);
        assert_eq!(history.last().unwrap().stroke, stroke1);
        
        history.push(stroke2, None, 200);
        assert_eq!(history.len(), 2);
        assert_eq!(history.last().unwrap().stroke, stroke2);
    }
    
    #[test]
    fn test_history_at() {
        let mut history = StrokeHistory::new();
        
        for i in 0..5 {
            history.push(Stroke::from_raw(i), None, i as u64 * 100);
        }
        
        // Most recent is index 4
        assert_eq!(history.at(0).unwrap().stroke.raw(), 4);
        assert_eq!(history.at(1).unwrap().stroke.raw(), 3);
        assert_eq!(history.at(4).unwrap().stroke.raw(), 0);
        assert!(history.at(5).is_none());
    }
    
    #[test]
    fn test_history_undo_redo() {
        let mut history = StrokeHistory::new();
        
        history.push(Stroke::from_raw(1), None, 100);
        history.push(Stroke::from_raw(2), None, 200);
        history.push(Stroke::from_raw(3), None, 300);
        
        // Undo most recent
        let undone = history.undo().unwrap();
        assert_eq!(undone.stroke.raw(), 3);
        assert!(undone.undone);
        
        // Undo next
        let undone = history.undo().unwrap();
        assert_eq!(undone.stroke.raw(), 2);
        
        // Redo
        let redone = history.redo().unwrap();
        assert_eq!(redone.stroke.raw(), 2);
        assert!(!redone.undone);
    }
    
    #[test]
    fn test_history_ring_buffer() {
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
}

impl Default for StrokeHistory {
    fn default() -> Self {
        Self::new()
    }
}
