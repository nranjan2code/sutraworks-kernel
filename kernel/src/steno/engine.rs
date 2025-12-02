//! Steno Engine - The core stroke processor
//!
//! This is the heart of the stenographic kernel.
//! Strokes flow in, intents flow out.
//!
//! # Processing Pipeline
//! ```
//! Raw Bits → Stroke → Dictionary → Intent → Action
//!               ↓
//!         Stroke Buffer (for multi-stroke briefs)
//!               ↓
//!         Stroke History (for undo/redo)
//! ```
//!
//! # Multi-Stroke Briefs
//! When a stroke doesn't match a single-stroke entry, it's buffered.
//! The engine checks if the buffer matches any multi-stroke brief.
//! If no match after timeout (500ms) or max strokes (8), buffer is cleared.

use super::stroke::Stroke;
use super::dictionary::{StenoDictionary, StrokeSequence, concepts};
use super::history::StrokeHistory;
use crate::intent::{ConceptID, Intent, IntentData};

// ═══════════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Timeout for multi-stroke sequences in microseconds (500ms)
const MULTI_STROKE_TIMEOUT_US: u64 = 500_000;

/// Maximum strokes to buffer before giving up
#[allow(dead_code)]
const MAX_BUFFER_STROKES: usize = 8;

// ═══════════════════════════════════════════════════════════════════════════════
// STENO ENGINE
// ═══════════════════════════════════════════════════════════════════════════════

/// Engine state
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EngineState {
    /// Ready to process strokes
    Ready,
    /// Waiting for more strokes (multi-stroke brief in progress)
    Pending,
    /// Processing error
    Error,
}

/// The steno engine processes strokes and emits intents
pub struct StenoEngine {
    /// Dictionary for stroke lookup
    dictionary: StenoDictionary,
    /// Buffer for multi-stroke sequences
    stroke_buffer: StrokeSequence,
    /// Timestamp of first stroke in buffer (for timeout)
    buffer_start_time: u64,
    /// History for undo/redo
    history: StrokeHistory,
    /// Current state
    state: EngineState,
    /// Last processed stroke
    last_stroke: Option<Stroke>,
    /// Statistics
    stats: EngineStats,
    /// Current timestamp (updated externally)
    timestamp: u64,
}

/// Engine statistics
#[derive(Clone, Copy, Default)]
pub struct EngineStats {
    /// Total strokes processed
    pub strokes_processed: u64,
    /// Successful intent matches
    pub intents_matched: u64,
    /// Corrections (undo strokes)
    pub corrections: u64,
    /// Unrecognized strokes
    pub unrecognized: u64,
    /// Multi-stroke matches
    pub multi_stroke_matches: u64,
}

impl StenoEngine {
    /// Create a new engine
    pub const fn new() -> Self {
        Self {
            dictionary: StenoDictionary::new(),
            stroke_buffer: StrokeSequence::new(),
            buffer_start_time: 0,
            history: StrokeHistory::new(),
            state: EngineState::Ready,
            last_stroke: None,
            stats: EngineStats {
                strokes_processed: 0,
                intents_matched: 0,
                corrections: 0,
                unrecognized: 0,
                multi_stroke_matches: 0,
            },
            timestamp: 0,
        }
    }
    
    /// Initialize the engine with default dictionary
    pub fn init(&mut self) {
        self.dictionary.init_defaults();
        self.state = EngineState::Ready;
    }
    
    /// Update the timestamp (call this from timer interrupt)
    pub fn set_timestamp(&mut self, ts: u64) {
        self.timestamp = ts;
    }
    
    /// Check if the stroke buffer has timed out
    fn is_buffer_timed_out(&self) -> bool {
        if self.stroke_buffer.is_empty() {
            return false;
        }
        self.timestamp.saturating_sub(self.buffer_start_time) > MULTI_STROKE_TIMEOUT_US
    }
    
    /// Process a single stroke
    ///
    /// Returns an intent if the stroke (or stroke sequence) resolves to one.
    pub fn process(&mut self, stroke: Stroke) -> Option<Intent> {
        self.stats.strokes_processed += 1;
        self.last_stroke = Some(stroke);
        
        // Handle correction stroke specially
        if stroke.is_correction() {
            return self.handle_correction();
        }
        
        // Check for timeout on pending buffer - if timed out, emit unknown and clear
        if self.is_buffer_timed_out() {
            self.stats.unrecognized += 1;
            self.stroke_buffer.clear();
            self.state = EngineState::Ready;
            // Don't return yet - continue processing the new stroke
        }
        
        // Try single-stroke lookup first (only if buffer is empty)
        if self.stroke_buffer.is_empty() {
            if let Some(intent) = self.dictionary.stroke_to_intent(stroke) {
                self.stats.intents_matched += 1;
                self.history.push(stroke, Some(&intent), self.timestamp);
                self.state = EngineState::Ready;
                return Some(intent);
            }
        }
        
        // Add to buffer for multi-stroke lookup
        if self.stroke_buffer.is_empty() {
            self.buffer_start_time = self.timestamp;
        }
        self.stroke_buffer.push(stroke);
        
        // Check multi-stroke dictionary
        let (exact_match, prefix_match) = self.dictionary.check_multi_prefix(&self.stroke_buffer);
        
        if exact_match {
            // We have a complete match!
            if let Some(intent) = self.dictionary.lookup_multi(&self.stroke_buffer) {
                self.stats.intents_matched += 1;
                self.stats.multi_stroke_matches += 1;
                self.history.push(stroke, Some(&intent), self.timestamp);
                self.stroke_buffer.clear();
                self.state = EngineState::Ready;
                return Some(intent);
            }
        }
        
        if prefix_match {
            // Could still be a valid multi-stroke, wait for more
            self.state = EngineState::Pending;
            return None;
        }
        
        // No prefix match - this sequence won't lead anywhere
        // If buffer has multiple strokes, the first might have been a valid single-stroke
        // that we missed. For now, emit unknown and clear.
        if self.stroke_buffer.len() > 1 {
            self.stats.unrecognized += 1;
            self.history.push(stroke, None, self.timestamp);
            self.stroke_buffer.clear();
            self.state = EngineState::Ready;
            return Some(Intent {
                concept_id: ConceptID::UNKNOWN,
                confidence: 0.0,
                data: IntentData::None,
                name: "UNKNOWN",
            });
        }
        
        // Single stroke that doesn't match anything - keep it buffered briefly
        // in case next stroke completes a multi-stroke brief
        self.state = EngineState::Pending;
        None
    }
    
    /// Process raw stroke bits from hardware
    pub fn process_raw(&mut self, bits: u32) -> Option<Intent> {
        self.process(Stroke::from_raw(bits))
    }
    
    /// Handle correction/undo stroke
    fn handle_correction(&mut self) -> Option<Intent> {
        self.stats.corrections += 1;
        
        // Clear stroke buffer if pending
        if !self.stroke_buffer.is_empty() {
            self.stroke_buffer.pop();
            self.state = if self.stroke_buffer.is_empty() {
                EngineState::Ready
            } else {
                EngineState::Pending
            };
            // Just cleared buffer, don't emit undo intent
            if self.stroke_buffer.is_empty() {
                return None;
            }
        }
        
        // Undo the most recent action in history
        self.history.undo();
        
        // Emit undo intent
        Some(Intent {
            concept_id: concepts::UNDO,
            confidence: 1.0,
            data: IntentData::None,
            name: "UNDO",
        })
    }
    
    /// Force flush the buffer (e.g., on timeout from external source)
    /// Returns intent if buffer matched something, or Unknown if not
    pub fn flush_buffer(&mut self) -> Option<Intent> {
        if self.stroke_buffer.is_empty() {
            return None;
        }
        
        // Try one last multi-stroke lookup
        if let Some(intent) = self.dictionary.lookup_multi(&self.stroke_buffer) {
            self.stats.intents_matched += 1;
            self.stats.multi_stroke_matches += 1;
            if let Some(stroke) = self.stroke_buffer.last() {
                self.history.push(stroke, Some(&intent), self.timestamp);
            }
            self.stroke_buffer.clear();
            self.state = EngineState::Ready;
            return Some(intent);
        }
        
        // No match - emit unknown
        self.stats.unrecognized += 1;
        self.stroke_buffer.clear();
        self.state = EngineState::Ready;
        Some(Intent {
            concept_id: ConceptID::UNKNOWN,
            confidence: 0.0,
            data: IntentData::None,
            name: "UNKNOWN",
        })
    }
    
    /// Get current engine state
    pub fn state(&self) -> EngineState {
        self.state
    }
    
    /// Get the last processed stroke
    pub fn last_stroke(&self) -> Option<Stroke> {
        self.last_stroke
    }
    
    /// Get the current stroke buffer
    pub fn stroke_buffer(&self) -> &StrokeSequence {
        &self.stroke_buffer
    }
    
    /// Get statistics
    pub fn stats(&self) -> &EngineStats {
        &self.stats
    }
    
    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = EngineStats::default();
    }
    
    /// Get dictionary reference
    pub fn dictionary(&self) -> &StenoDictionary {
        &self.dictionary
    }
    
    /// Get mutable dictionary reference (for adding custom entries)
    pub fn dictionary_mut(&mut self) -> &mut StenoDictionary {
        &mut self.dictionary
    }
    
    /// Get history reference
    pub fn history(&self) -> &StrokeHistory {
        &self.history
    }
    
    /// Get mutable history reference
    pub fn history_mut(&mut self) -> &mut StrokeHistory {
        &mut self.history
    }
    
    /// Redo the last undone action
    pub fn redo(&mut self) -> Option<Intent> {
        if let Some(entry) = self.history.redo() {
            if let Some(id) = entry.intent_id {
                return Some(Intent {
                    concept_id: ConceptID(id),
                    name: "REDO",
                    confidence: 1.0,
                    data: IntentData::None,
                });
            }
        }
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// STROKE PRODUCER TRAIT
// ═══════════════════════════════════════════════════════════════════════════════

/// Trait for devices that produce steno strokes
pub trait StrokeProducer {
    /// Check if a stroke is available
    fn stroke_available(&self) -> bool;
    
    /// Read the next stroke (blocking)
    fn read_stroke(&mut self) -> Stroke;
    
    /// Read stroke if available (non-blocking)
    fn try_read_stroke(&mut self) -> Option<Stroke>;
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTENT CONSUMER TRAIT
// ═══════════════════════════════════════════════════════════════════════════════

/// Trait for systems that consume intents
pub trait IntentConsumer {
    /// Handle an intent
    fn handle_intent(&mut self, intent: Intent);
    
    /// Check if this consumer can handle a specific concept
    fn can_handle(&self, concept_id: ConceptID) -> bool;
}

// ═══════════════════════════════════════════════════════════════════════════════
// ASYNC STROKE PROCESSING
// ═══════════════════════════════════════════════════════════════════════════════

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

/// Future for reading a stroke asynchronously
pub struct StrokeReadFuture<'a, P: StrokeProducer> {
    producer: &'a mut P,
}

impl<'a, P: StrokeProducer> Future for StrokeReadFuture<'a, P> {
    type Output = Stroke;
    
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(stroke) = self.producer.try_read_stroke() {
            Poll::Ready(stroke)
        } else {
            // In a real implementation, we'd register the waker with the interrupt
            Poll::Pending
        }
    }
}

/// Extension trait for async stroke reading
pub trait StrokeProducerExt: StrokeProducer {
    /// Read a stroke asynchronously
    fn read_stroke_async(&mut self) -> StrokeReadFuture<'_, Self>
    where
        Self: Sized,
    {
        StrokeReadFuture { producer: self }
    }
}

impl<T: StrokeProducer> StrokeProducerExt for T {}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::stroke::parse_steno_to_bits;
    
    #[test]
    fn test_engine_init() {
        let mut engine = StenoEngine::new();
        engine.init();
        assert_eq!(engine.state(), EngineState::Ready);
        assert!(engine.dictionary().len() > 0);
        assert!(engine.dictionary().multi_len() > 0);
    }
    
    #[test]
    fn test_correction_stroke() {
        let mut engine = StenoEngine::new();
        engine.init();
        
        let intent = engine.process(Stroke::STAR);
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().concept_id, concepts::UNDO);
    }
    
    #[test]
    fn test_single_stroke_lookup() {
        let mut engine = StenoEngine::new();
        engine.init();
        
        // "STAT" should match STATUS
        let stroke = Stroke::from_raw(parse_steno_to_bits("STAT"));
        let intent = engine.process(stroke);
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().name, "STATUS");
    }
    
    #[test]
    fn test_multi_stroke_lookup() {
        let mut engine = StenoEngine::new();
        engine.init();
        
        // "RAOE/PWOOT" should match REBOOT
        let stroke1 = Stroke::from_raw(parse_steno_to_bits("RAOE"));
        let stroke2 = Stroke::from_raw(parse_steno_to_bits("PWOOT"));
        
        // First stroke should return None (pending)
        let result1 = engine.process(stroke1);
        assert!(result1.is_none());
        assert_eq!(engine.state(), EngineState::Pending);
        
        // Second stroke should complete the match
        let result2 = engine.process(stroke2);
        assert!(result2.is_some());
        assert_eq!(result2.unwrap().name, "REBOOT");
        assert_eq!(engine.state(), EngineState::Ready);
    }
    
    #[test]
    fn test_buffer_clear_on_asterisk() {
        let mut engine = StenoEngine::new();
        engine.init();
        
        // Start a multi-stroke
        let stroke1 = Stroke::from_raw(parse_steno_to_bits("RAOE"));
        engine.process(stroke1);
        assert_eq!(engine.state(), EngineState::Pending);
        
        // Asterisk should clear buffer
        engine.process(Stroke::STAR);
        assert!(engine.stroke_buffer().is_empty() || engine.state() == EngineState::Ready);
    }
    
    #[test]
    fn test_stats_multi_stroke() {
        let mut engine = StenoEngine::new();
        engine.init();
        
        let stroke1 = Stroke::from_raw(parse_steno_to_bits("RAOE"));
        let stroke2 = Stroke::from_raw(parse_steno_to_bits("PWOOT"));
        
        engine.process(stroke1);
        engine.process(stroke2);
        
        assert_eq!(engine.stats().multi_stroke_matches, 1);
    }
}
