//! Temporal Dynamics Module
//! 
//! Implements biological-inspired temporal processing:
//! - Decay management for neural activations
//! - Temporal summation (repeated weak signals)
//! - Predictive priming (anticipate next intent)
//! - Refractory period tracking

use crate::intent::ConceptID;
use crate::kernel::memory::neural::NEURAL_ALLOCATOR;

// ═══════════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Default decay rate (0.1% per ms = 10% per second)
pub const DEFAULT_DECAY_RATE: f32 = 0.0001;

/// Default temporal summation window (100ms)
pub const DEFAULT_SUMMATION_WINDOW_MS: u64 = 100;

/// Default sequence learning window (500ms)
pub const DEFAULT_SEQUENCE_WINDOW_MS: u64 = 500;

/// Default decay tick interval (100ms)
pub const DEFAULT_DECAY_INTERVAL_MS: u64 = 100;

// ═══════════════════════════════════════════════════════════════════════════════
// TEMPORAL DYNAMICS MANAGER
// ═══════════════════════════════════════════════════════════════════════════════

/// Manages temporal dynamics for the neural intent system
/// 
/// # Features
/// 
/// - **Automatic Decay**: Call `tick()` periodically to decay all activations
/// - **Temporal Summation**: Weak signals within a window accumulate
/// - **Sequence Learning**: Records A→B sequences for prediction
/// - **Predictive Priming**: Pre-activates likely next concepts
pub struct TemporalDynamics {
    /// Last decay tick timestamp
    last_decay_tick: u64,
    /// Decay rate (per ms)
    decay_rate: f32,
    /// Decay tick interval (ms)
    decay_interval: u64,
    /// Temporal summation window (ms)
    summation_window: u64,
    /// Sequence learning window (ms)
    sequence_window: u64,
    /// Last activated concept (for sequence learning)
    last_concept: Option<ConceptID>,
    /// Timestamp of last activation
    last_activation_time: u64,
    /// Total decay ticks performed
    tick_count: u64,
}

impl TemporalDynamics {
    /// Create with default settings
    pub const fn new() -> Self {
        Self {
            last_decay_tick: 0,
            decay_rate: DEFAULT_DECAY_RATE,
            decay_interval: DEFAULT_DECAY_INTERVAL_MS,
            summation_window: DEFAULT_SUMMATION_WINDOW_MS,
            sequence_window: DEFAULT_SEQUENCE_WINDOW_MS,
            last_concept: None,
            last_activation_time: 0,
            tick_count: 0,
        }
    }
    
    /// Create with custom settings
    pub fn with_config(
        decay_rate: f32,
        decay_interval: u64,
        summation_window: u64,
        sequence_window: u64,
    ) -> Self {
        Self {
            decay_rate,
            decay_interval,
            summation_window,
            sequence_window,
            ..Self::new()
        }
    }
    
    /// Periodic tick - call this from scheduler or timer interrupt
    /// 
    /// Applies decay to all neural activations if enough time has passed.
    /// 
    /// # Arguments
    /// * `timestamp` - Current time in ms
    /// 
    /// # Returns
    /// True if decay was applied, false if not yet time
    pub fn tick(&mut self, timestamp: u64) -> bool {
        let elapsed = timestamp.saturating_sub(self.last_decay_tick);
        
        if elapsed >= self.decay_interval {
            let mut allocator = NEURAL_ALLOCATOR.lock();
            allocator.decay_tick(timestamp, self.decay_rate);
            drop(allocator);
            
            self.last_decay_tick = timestamp;
            self.tick_count += 1;
            true
        } else {
            false
        }
    }
    
    /// Process an intent activation with temporal dynamics
    /// 
    /// This method:
    /// 1. Activates the concept with spreading activation
    /// 2. Records sequence for prediction learning
    /// 3. Applies predictive priming to associates
    /// 
    /// # Arguments
    /// * `concept_id` - Concept being activated
    /// * `strength` - Activation strength
    /// * `timestamp` - Current time
    /// 
    /// # Returns
    /// Number of concepts primed for prediction
    pub fn process_activation(
        &mut self,
        concept_id: ConceptID,
        strength: f32,
        timestamp: u64,
    ) -> usize {
        let mut allocator = NEURAL_ALLOCATOR.lock();
        
        // 1. Activate with spreading
        allocator.activate(concept_id, strength, timestamp);
        
        // 2. Record sequence (if we have a previous concept)
        if let Some(prev) = self.last_concept {
            allocator.record_sequence(prev, concept_id, timestamp, self.sequence_window);
        }
        
        // 3. Apply predictive priming
        let primed = allocator.apply_predictive_priming(concept_id, timestamp);
        
        // Update state
        self.last_concept = Some(concept_id);
        self.last_activation_time = timestamp;
        
        primed
    }
    
    /// Temporal summation - accumulate weak signals
    /// 
    /// Call this for weak/noisy signals that should sum within a window.
    /// 
    /// # Returns
    /// True if the accumulated activation crossed firing threshold
    pub fn summate(
        &self,
        concept_id: ConceptID,
        strength: f32,
        timestamp: u64,
    ) -> bool {
        let mut allocator = NEURAL_ALLOCATOR.lock();
        allocator.temporal_summate(concept_id, strength, timestamp, self.summation_window)
    }
    
    /// Get predicted next concepts
    /// 
    /// Returns concepts that have been primed by recent activations.
    pub fn get_predictions(&self, limit: usize) -> alloc::vec::Vec<(ConceptID, f32)> {
        let allocator = NEURAL_ALLOCATOR.lock();
        allocator.get_predictions(0.1, limit)
    }
    
    /// Check if a concept is primed (ready for fast activation)
    pub fn is_primed(&self, concept_id: ConceptID) -> bool {
        let allocator = NEURAL_ALLOCATOR.lock();
        allocator.is_primed(concept_id)
    }
    
    /// Get priming level of a concept
    pub fn get_priming_level(&self, concept_id: ConceptID) -> f32 {
        let allocator = NEURAL_ALLOCATOR.lock();
        allocator.get_priming_level(concept_id)
    }
    
    /// Get statistics
    pub fn stats(&self) -> TemporalStats {
        TemporalStats {
            tick_count: self.tick_count,
            last_decay_tick: self.last_decay_tick,
            decay_rate: self.decay_rate,
            last_concept: self.last_concept,
        }
    }
    
    /// Reset temporal state (clear history but keep settings)
    pub fn reset(&mut self) {
        self.last_concept = None;
        self.last_activation_time = 0;
        self.tick_count = 0;
    }
}

impl Default for TemporalDynamics {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// STATISTICS
// ═══════════════════════════════════════════════════════════════════════════════

/// Temporal dynamics statistics
#[derive(Clone, Copy, Debug)]
pub struct TemporalStats {
    /// Total decay ticks performed
    pub tick_count: u64,
    /// Last decay tick timestamp
    pub last_decay_tick: u64,
    /// Current decay rate
    pub decay_rate: f32,
    /// Last activated concept
    pub last_concept: Option<ConceptID>,
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL INSTANCE
// ═══════════════════════════════════════════════════════════════════════════════

use crate::kernel::sync::SpinLock;

/// Global temporal dynamics manager
pub static TEMPORAL_DYNAMICS: SpinLock<TemporalDynamics> = SpinLock::new(TemporalDynamics::new());

/// Convenience function: process decay tick
/// 
/// Call this from the timer interrupt or scheduler tick.
pub fn decay_tick(timestamp: u64) -> bool {
    TEMPORAL_DYNAMICS.lock().tick(timestamp)
}

/// Convenience function: process intent activation with temporal dynamics
pub fn process_intent_activation(concept_id: ConceptID, strength: f32, timestamp: u64) -> usize {
    TEMPORAL_DYNAMICS.lock().process_activation(concept_id, strength, timestamp)
}

/// Convenience function: temporal summation
pub fn summate(concept_id: ConceptID, strength: f32, timestamp: u64) -> bool {
    TEMPORAL_DYNAMICS.lock().summate(concept_id, strength, timestamp)
}

/// Convenience function: check if primed
pub fn is_primed(concept_id: ConceptID) -> bool {
    TEMPORAL_DYNAMICS.lock().is_primed(concept_id)
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_temporal_dynamics_new() {
        let td = TemporalDynamics::new();
        assert_eq!(td.decay_rate, DEFAULT_DECAY_RATE);
        assert_eq!(td.decay_interval, DEFAULT_DECAY_INTERVAL_MS);
        assert!(td.last_concept.is_none());
    }
    
    #[test]
    fn test_temporal_dynamics_tick_interval() {
        let mut td = TemporalDynamics::new();
        
        // First tick at time 0 should not apply decay (no time passed)
        assert!(!td.tick(0));
        
        // Tick at time 50 should not apply decay (interval is 100)
        assert!(!td.tick(50));
        
        // Tick at time 100 should apply decay
        assert!(td.tick(100));
        assert_eq!(td.tick_count, 1);
    }
    
    #[test]
    fn test_temporal_stats() {
        let td = TemporalDynamics::new();
        let stats = td.stats();
        
        assert_eq!(stats.tick_count, 0);
        assert_eq!(stats.decay_rate, DEFAULT_DECAY_RATE);
        assert!(stats.last_concept.is_none());
    }
}
