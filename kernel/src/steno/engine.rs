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
//! ```

use super::stroke::Stroke;
use super::dictionary::{StenoDictionary, StrokeSequence, concepts};
use crate::intent::{ConceptID, Intent, IntentData};

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
    /// Current state
    state: EngineState,
    /// Last processed stroke
    last_stroke: Option<Stroke>,
    /// Statistics
    stats: EngineStats,
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
}

impl StenoEngine {
    /// Create a new engine
    pub const fn new() -> Self {
        Self {
            dictionary: StenoDictionary::new(),
            stroke_buffer: StrokeSequence::new(),
            state: EngineState::Ready,
            last_stroke: None,
            stats: EngineStats {
                strokes_processed: 0,
                intents_matched: 0,
                corrections: 0,
                unrecognized: 0,
            },
        }
    }
    
    /// Initialize the engine with default dictionary
    pub fn init(&mut self) {
        self.dictionary.init_defaults();
        self.state = EngineState::Ready;
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
        
        // Try single-stroke lookup first
        if let Some(intent) = self.dictionary.stroke_to_intent(stroke) {
            self.stats.intents_matched += 1;
            self.stroke_buffer.clear();
            self.state = EngineState::Ready;
            return Some(intent);
        }
        
        // Add to buffer for multi-stroke lookup
        self.stroke_buffer.push(stroke);
        
        // Try multi-stroke lookup
        if let Some(intent) = self.try_multi_stroke_lookup() {
            self.stats.intents_matched += 1;
            self.stroke_buffer.clear();
            self.state = EngineState::Ready;
            return Some(intent);
        }
        
        // No match yet - could be incomplete multi-stroke
        if self.stroke_buffer.len() >= 2 {
            // After 2 unmatched strokes, emit unknown and clear
            self.stats.unrecognized += 1;
            self.stroke_buffer.clear();
            self.state = EngineState::Ready;
            return Some(Intent {
                concept_id: ConceptID(0xFFFF_FFFF), // Unknown concept
                confidence: 0.0,
                data: IntentData::None,
            });
        }
        
        // Still waiting for more strokes
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
        }
        
        // Emit undo intent
        Some(Intent {
            concept_id: concepts::UNDO,
            confidence: 1.0,
            data: IntentData::None,
        })
    }
    
    /// Try to match the current stroke buffer as a multi-stroke brief
    fn try_multi_stroke_lookup(&self) -> Option<Intent> {
        // TODO: Implement multi-stroke dictionary lookup
        // For now, we only support single-stroke lookups
        None
    }
    
    /// Get current engine state
    pub fn state(&self) -> EngineState {
        self.state
    }
    
    /// Get the last processed stroke
    pub fn last_stroke(&self) -> Option<Stroke> {
        self.last_stroke
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
    
    #[test]
    fn test_engine_init() {
        let mut engine = StenoEngine::new();
        engine.init();
        assert_eq!(engine.state(), EngineState::Ready);
        assert!(engine.dictionary().len() > 0);
    }
    
    #[test]
    fn test_correction_stroke() {
        let mut engine = StenoEngine::new();
        engine.init();
        
        let intent = engine.process(Stroke::STAR);
        assert!(intent.is_some());
        assert_eq!(intent.unwrap().concept_id, concepts::UNDO);
    }
}
