//! Feedback Loops Module
//! 
//! Implements predictive processing with feedback:
//! - Efference copy (prediction of action outcomes)
//! - Expectation matching (compare predicted vs actual)
//! - Surprise detection (flag unexpected events)
//! - Priority adjustment (surprise boosts priority)
//! 
//! # Biological Inspiration
//! 
//! Models the predictive processing framework:
//! - Motor commands generate efference copies (predictions)
//! - Sensory input is compared against predictions
//! - Mismatches generate prediction errors (surprise)
//! - Surprise signals modulate attention and learning

use alloc::vec::Vec;
use crate::intent::ConceptID;
use crate::kernel::sync::SpinLock;

// ═══════════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum number of active predictions
pub const MAX_PREDICTIONS: usize = 16;

/// Maximum number of expectation entries
pub const MAX_EXPECTATIONS: usize = 32;

/// Default prediction decay rate (per tick)
pub const DEFAULT_PREDICTION_DECAY: f32 = 0.1;

/// Default surprise threshold
pub const DEFAULT_SURPRISE_THRESHOLD: f32 = 0.5;

/// Default expectation window in ms
pub const DEFAULT_EXPECTATION_WINDOW_MS: u64 = 500;

// ═══════════════════════════════════════════════════════════════════════════════
// PREDICTION (Efference Copy)
// ═══════════════════════════════════════════════════════════════════════════════

/// A prediction of future input
/// 
/// # Biological Analogy
/// 
/// Like efference copy in motor control - when you move your arm,
/// a copy of the motor command predicts the sensory feedback you'll receive.
#[derive(Clone, Copy, Debug)]
pub struct Prediction {
    /// What we predict will happen
    pub predicted: ConceptID,
    /// Confidence in the prediction (0.0-1.0)
    pub confidence: f32,
    /// When the prediction was made
    pub timestamp: u64,
    /// When we expect the predicted event (deadline)
    pub expected_at: u64,
    /// What caused this prediction (source action/concept)
    pub source: ConceptID,
    /// Whether this prediction has been matched
    pub matched: bool,
}

impl Prediction {
    /// Create a new prediction
    pub fn new(
        predicted: ConceptID,
        confidence: f32,
        source: ConceptID,
        timestamp: u64,
        window_ms: u64,
    ) -> Self {
        Self {
            predicted,
            confidence,
            timestamp,
            expected_at: timestamp + window_ms,
            source,
            matched: false,
        }
    }
    
    /// Check if prediction has expired
    pub fn is_expired(&self, current_time: u64) -> bool {
        current_time > self.expected_at
    }
    
    /// Check if prediction matches an input
    pub fn matches(&self, concept_id: ConceptID) -> bool {
        self.predicted == concept_id && !self.matched
    }
}

/// Prediction buffer - holds active predictions
#[derive(Clone)]
pub struct PredictionBuffer {
    predictions: heapless::Vec<Prediction, MAX_PREDICTIONS>,
    decay_rate: f32,
}

impl PredictionBuffer {
    /// Create new prediction buffer
    pub const fn new() -> Self {
        Self {
            predictions: heapless::Vec::new(),
            decay_rate: DEFAULT_PREDICTION_DECAY,
        }
    }
    
    /// Add a prediction
    pub fn predict(
        &mut self,
        predicted: ConceptID,
        confidence: f32,
        source: ConceptID,
        timestamp: u64,
        window_ms: u64,
    ) -> bool {
        let prediction = Prediction::new(predicted, confidence, source, timestamp, window_ms);
        self.predictions.push(prediction).is_ok()
    }
    
    /// Check if a concept was predicted
    /// 
    /// Returns the matching prediction's confidence if found.
    pub fn check_predicted(&mut self, concept_id: ConceptID) -> Option<f32> {
        for pred in self.predictions.iter_mut() {
            if pred.matches(concept_id) {
                pred.matched = true;
                return Some(pred.confidence);
            }
        }
        None
    }
    
    /// Get all unmatched predictions (for surprise detection)
    pub fn unmatched(&self, current_time: u64) -> Vec<&Prediction> {
        self.predictions
            .iter()
            .filter(|p| !p.matched && !p.is_expired(current_time))
            .collect()
    }
    
    /// Get expired unmatched predictions (omissions - expected but didn't happen)
    pub fn omissions(&self, current_time: u64) -> Vec<&Prediction> {
        self.predictions
            .iter()
            .filter(|p| !p.matched && p.is_expired(current_time))
            .collect()
    }
    
    /// Clean up expired predictions
    pub fn cleanup(&mut self, current_time: u64) {
        // Keep only unexpired or matched predictions
        let mut i = 0;
        while i < self.predictions.len() {
            let pred = &self.predictions[i];
            if pred.is_expired(current_time) && !pred.matched {
                self.predictions.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }
    
    /// Number of active predictions
    pub fn len(&self) -> usize {
        self.predictions.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.predictions.is_empty()
    }
    
    /// Clear all predictions
    pub fn clear(&mut self) {
        self.predictions.clear();
    }
}

impl Default for PredictionBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// EXPECTATION MATCHING
// ═══════════════════════════════════════════════════════════════════════════════

/// An expectation entry for matching
#[derive(Clone, Copy, Debug)]
pub struct Expectation {
    /// Expected concept
    pub expected: ConceptID,
    /// Strength of expectation (0.0-1.0)
    pub strength: f32,
    /// When set
    pub timestamp: u64,
    /// Expiry time
    pub expires_at: u64,
}

impl Expectation {
    /// Create new expectation
    pub fn new(expected: ConceptID, strength: f32, timestamp: u64, window_ms: u64) -> Self {
        Self {
            expected,
            strength,
            timestamp,
            expires_at: timestamp + window_ms,
        }
    }
    
    /// Check if expired
    pub fn is_expired(&self, current_time: u64) -> bool {
        current_time > self.expires_at
    }
}

/// Expectation matcher - compares inputs against expectations
pub struct ExpectationMatcher {
    expectations: heapless::Vec<Expectation, MAX_EXPECTATIONS>,
    window_ms: u64,
}

impl ExpectationMatcher {
    /// Create new matcher
    pub const fn new() -> Self {
        Self {
            expectations: heapless::Vec::new(),
            window_ms: DEFAULT_EXPECTATION_WINDOW_MS,
        }
    }
    
    /// Set an expectation
    pub fn expect(&mut self, concept_id: ConceptID, strength: f32, timestamp: u64) -> bool {
        let exp = Expectation::new(concept_id, strength, timestamp, self.window_ms);
        self.expectations.push(exp).is_ok()
    }
    
    /// Match an input against expectations
    /// 
    /// Returns (matched, strength) - matched is true if expected, strength is expectation strength
    pub fn match_input(&mut self, concept_id: ConceptID, timestamp: u64) -> (bool, f32) {
        // Clean up expired first
        self.cleanup(timestamp);
        
        // Find matching expectation
        for i in 0..self.expectations.len() {
            if self.expectations[i].expected == concept_id {
                let strength = self.expectations[i].strength;
                self.expectations.swap_remove(i);
                return (true, strength);
            }
        }
        
        (false, 0.0)
    }
    
    /// Get current expectations
    pub fn current_expectations(&self) -> &[Expectation] {
        // Return as slice (heapless::Vec derefs to slice)
        &self.expectations
    }
    
    /// Clean up expired expectations
    pub fn cleanup(&mut self, current_time: u64) {
        let mut i = 0;
        while i < self.expectations.len() {
            if self.expectations[i].is_expired(current_time) {
                self.expectations.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }
    
    /// Number of active expectations
    pub fn len(&self) -> usize {
        self.expectations.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.expectations.is_empty()
    }
    
    /// Clear all expectations
    pub fn clear(&mut self) {
        self.expectations.clear();
    }
    
    /// Set expectation window
    pub fn set_window(&mut self, window_ms: u64) {
        self.window_ms = window_ms;
    }
}

impl Default for ExpectationMatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SURPRISE DETECTION
// ═══════════════════════════════════════════════════════════════════════════════

/// Type of surprise event
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurpriseType {
    /// Unexpected input (not predicted)
    Unexpected,
    /// Omission (predicted but didn't happen)
    Omission,
    /// Mismatch (different from expected)
    Mismatch,
}

/// A surprise event
#[derive(Clone, Copy, Debug)]
pub struct SurpriseEvent {
    /// Type of surprise
    pub surprise_type: SurpriseType,
    /// Concept that caused surprise
    pub concept_id: ConceptID,
    /// Surprise magnitude (0.0-1.0)
    pub magnitude: f32,
    /// When it occurred
    pub timestamp: u64,
    /// What was expected instead (if applicable)
    pub expected: Option<ConceptID>,
}

impl SurpriseEvent {
    /// Create unexpected event
    pub fn unexpected(concept_id: ConceptID, magnitude: f32, timestamp: u64) -> Self {
        Self {
            surprise_type: SurpriseType::Unexpected,
            concept_id,
            magnitude,
            timestamp,
            expected: None,
        }
    }
    
    /// Create omission event
    pub fn omission(expected: ConceptID, magnitude: f32, timestamp: u64) -> Self {
        Self {
            surprise_type: SurpriseType::Omission,
            concept_id: expected, // The missing concept
            magnitude,
            timestamp,
            expected: Some(expected),
        }
    }
    
    /// Create mismatch event
    pub fn mismatch(
        actual: ConceptID,
        expected: ConceptID,
        magnitude: f32,
        timestamp: u64,
    ) -> Self {
        Self {
            surprise_type: SurpriseType::Mismatch,
            concept_id: actual,
            magnitude,
            timestamp,
            expected: Some(expected),
        }
    }
}

/// Surprise detector
pub struct SurpriseDetector {
    /// Recent surprise events
    events: heapless::Vec<SurpriseEvent, 16>,
    /// Surprise threshold
    threshold: f32,
    /// Cumulative surprise level
    cumulative_surprise: f32,
    /// Decay rate for cumulative surprise
    decay_rate: f32,
    /// Last update timestamp
    last_update: u64,
}

impl SurpriseDetector {
    /// Create new detector
    pub const fn new() -> Self {
        Self {
            events: heapless::Vec::new(),
            threshold: DEFAULT_SURPRISE_THRESHOLD,
            cumulative_surprise: 0.0,
            decay_rate: 0.01,
            last_update: 0,
        }
    }
    
    /// Record a surprise event
    pub fn record(&mut self, event: SurpriseEvent) {
        // Update cumulative surprise
        self.cumulative_surprise = (self.cumulative_surprise + event.magnitude).min(1.0);
        
        // Store event
        if self.events.len() >= 16 {
            self.events.remove(0); // Remove oldest
        }
        self.events.push(event).ok();
    }
    
    /// Record unexpected input
    pub fn unexpected(&mut self, concept_id: ConceptID, timestamp: u64) {
        let magnitude = 1.0; // Fully unexpected
        self.record(SurpriseEvent::unexpected(concept_id, magnitude, timestamp));
    }
    
    /// Record omission
    pub fn omission(&mut self, expected: ConceptID, confidence: f32, timestamp: u64) {
        let magnitude = confidence; // Surprise proportional to confidence
        self.record(SurpriseEvent::omission(expected, magnitude, timestamp));
    }
    
    /// Record mismatch
    pub fn mismatch(
        &mut self,
        actual: ConceptID,
        expected: ConceptID,
        timestamp: u64,
    ) {
        let magnitude = 0.8; // Mismatches are somewhat surprising
        self.record(SurpriseEvent::mismatch(actual, expected, magnitude, timestamp));
    }
    
    /// Update and decay cumulative surprise
    pub fn tick(&mut self, timestamp: u64) {
        let elapsed = timestamp.saturating_sub(self.last_update);
        let decay = (self.decay_rate * elapsed as f32).min(1.0);
        self.cumulative_surprise = (self.cumulative_surprise - decay).max(0.0);
        self.last_update = timestamp;
    }
    
    /// Get current surprise level
    pub fn surprise_level(&self) -> f32 {
        self.cumulative_surprise
    }
    
    /// Check if above surprise threshold
    pub fn is_surprised(&self) -> bool {
        self.cumulative_surprise >= self.threshold
    }
    
    /// Get recent events
    pub fn recent_events(&self) -> &[SurpriseEvent] {
        &self.events
    }
    
    /// Get priority boost based on surprise
    /// 
    /// Higher surprise = higher priority boost for processing.
    pub fn priority_boost(&self) -> f32 {
        1.0 + (self.cumulative_surprise * 0.5) // Up to 50% boost
    }
    
    /// Clear all events
    pub fn clear(&mut self) {
        self.events.clear();
        self.cumulative_surprise = 0.0;
    }
    
    /// Set threshold
    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold.clamp(0.1, 1.0);
    }
}

impl Default for SurpriseDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// FEEDBACK PROCESSOR
// ═══════════════════════════════════════════════════════════════════════════════

/// Integrated feedback processor
/// 
/// Combines prediction, expectation matching, and surprise detection.
/// 
/// # Usage
/// 
/// ```ignore
/// // When taking an action, predict its outcome
/// processor.predict(ACTION_OPEN, RESULT_OPENED, 0.9, timestamp);
/// 
/// // When input arrives, process it
/// let result = processor.process_input(RESULT_OPENED, timestamp);
/// // result.was_predicted = true, result.surprise = 0.0
/// 
/// // If unexpected input arrives
/// let result = processor.process_input(UNEXPECTED_ERROR, timestamp);
/// // result.was_predicted = false, result.surprise > 0
/// ```
pub struct FeedbackProcessor {
    /// Active predictions
    predictions: PredictionBuffer,
    /// Expectation matcher
    expectations: ExpectationMatcher,
    /// Surprise detector
    surprise: SurpriseDetector,
    /// Total inputs processed
    total_processed: u64,
    /// Total predictions made
    total_predictions: u64,
    /// Successful predictions
    successful_predictions: u64,
}

impl FeedbackProcessor {
    /// Create new processor
    pub const fn new() -> Self {
        Self {
            predictions: PredictionBuffer::new(),
            expectations: ExpectationMatcher::new(),
            surprise: SurpriseDetector::new(),
            total_processed: 0,
            total_predictions: 0,
            successful_predictions: 0,
        }
    }
    
    /// Make a prediction (efference copy)
    /// 
    /// # Arguments
    /// * `source` - Action/concept that causes the prediction
    /// * `predicted` - What we expect to happen
    /// * `confidence` - How confident are we
    /// * `timestamp` - Current time
    /// * `window_ms` - How long to wait for the prediction
    pub fn predict(
        &mut self,
        source: ConceptID,
        predicted: ConceptID,
        confidence: f32,
        timestamp: u64,
        window_ms: u64,
    ) {
        self.predictions.predict(predicted, confidence, source, timestamp, window_ms);
        self.total_predictions += 1;
    }
    
    /// Set an expectation
    pub fn expect(&mut self, concept_id: ConceptID, strength: f32, timestamp: u64) {
        self.expectations.expect(concept_id, strength, timestamp);
    }
    
    /// Process an input and check against predictions/expectations
    pub fn process_input(&mut self, concept_id: ConceptID, timestamp: u64) -> FeedbackResult {
        self.total_processed += 1;
        
        // Check if this was predicted
        let prediction_confidence = self.predictions.check_predicted(concept_id);
        let was_predicted = prediction_confidence.is_some();
        
        if was_predicted {
            self.successful_predictions += 1;
        }
        
        // Check against expectations
        let (was_expected, expectation_strength) = 
            self.expectations.match_input(concept_id, timestamp);
        
        // Calculate surprise
        let surprise = if was_predicted {
            0.0 // Predicted = no surprise
        } else if was_expected {
            0.1 // Expected but not predicted = low surprise
        } else {
            1.0 // Neither predicted nor expected = full surprise
        };
        
        // Record surprise if significant
        if surprise > 0.3 {
            self.surprise.unexpected(concept_id, timestamp);
        }
        
        // Update surprise decay
        self.surprise.tick(timestamp);
        
        FeedbackResult {
            concept_id,
            was_predicted,
            prediction_confidence: prediction_confidence.unwrap_or(0.0),
            was_expected,
            expectation_strength,
            surprise,
            priority_boost: self.surprise.priority_boost(),
        }
    }
    
    /// Check for omissions (predictions that didn't happen)
    pub fn check_omissions(&mut self, timestamp: u64) -> Vec<ConceptID> {
        let omissions: Vec<_> = self.predictions.omissions(timestamp)
            .iter()
            .map(|p| {
                self.surprise.omission(p.predicted, p.confidence, timestamp);
                p.predicted
            })
            .collect();
        
        // Clean up
        self.predictions.cleanup(timestamp);
        self.expectations.cleanup(timestamp);
        
        omissions
    }
    
    /// Get current surprise level
    pub fn surprise_level(&self) -> f32 {
        self.surprise.surprise_level()
    }
    
    /// Check if system is surprised
    pub fn is_surprised(&self) -> bool {
        self.surprise.is_surprised()
    }
    
    /// Get priority boost from surprise
    pub fn priority_boost(&self) -> f32 {
        self.surprise.priority_boost()
    }
    
    /// Get prediction accuracy
    pub fn prediction_accuracy(&self) -> f32 {
        if self.total_predictions == 0 {
            1.0
        } else {
            self.successful_predictions as f32 / self.total_predictions as f32
        }
    }
    
    /// Get statistics
    pub fn stats(&self) -> FeedbackStats {
        FeedbackStats {
            total_processed: self.total_processed,
            total_predictions: self.total_predictions,
            successful_predictions: self.successful_predictions,
            active_predictions: self.predictions.len(),
            active_expectations: self.expectations.len(),
            surprise_level: self.surprise.surprise_level(),
            prediction_accuracy: self.prediction_accuracy(),
        }
    }
    
    /// Clear all state
    pub fn clear(&mut self) {
        self.predictions.clear();
        self.expectations.clear();
        self.surprise.clear();
    }
    
    /// Get prediction buffer
    pub fn predictions(&self) -> &PredictionBuffer {
        &self.predictions
    }
    
    /// Get expectation matcher
    pub fn expectations(&self) -> &ExpectationMatcher {
        &self.expectations
    }
    
    /// Get surprise detector
    pub fn surprise(&self) -> &SurpriseDetector {
        &self.surprise
    }
    
    /// Get mutable surprise detector
    pub fn surprise_mut(&mut self) -> &mut SurpriseDetector {
        &mut self.surprise
    }
}

impl Default for FeedbackProcessor {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// RESULT TYPES
// ═══════════════════════════════════════════════════════════════════════════════

/// Result of processing an input through feedback system
#[derive(Clone, Copy, Debug)]
pub struct FeedbackResult {
    /// The input concept
    pub concept_id: ConceptID,
    /// Whether it was predicted
    pub was_predicted: bool,
    /// Confidence of the prediction (if predicted)
    pub prediction_confidence: f32,
    /// Whether it was expected
    pub was_expected: bool,
    /// Strength of expectation (if expected)
    pub expectation_strength: f32,
    /// Surprise level (0.0 = predicted, 1.0 = fully unexpected)
    pub surprise: f32,
    /// Priority boost from cumulative surprise
    pub priority_boost: f32,
}

/// Statistics for feedback system
#[derive(Clone, Copy, Debug, Default)]
pub struct FeedbackStats {
    /// Total inputs processed
    pub total_processed: u64,
    /// Total predictions made
    pub total_predictions: u64,
    /// Successful predictions
    pub successful_predictions: u64,
    /// Currently active predictions
    pub active_predictions: usize,
    /// Currently active expectations
    pub active_expectations: usize,
    /// Current surprise level
    pub surprise_level: f32,
    /// Overall prediction accuracy
    pub prediction_accuracy: f32,
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL INSTANCE
// ═══════════════════════════════════════════════════════════════════════════════

/// Global feedback processor
pub static FEEDBACK_PROCESSOR: SpinLock<FeedbackProcessor> = 
    SpinLock::new(FeedbackProcessor::new());

/// Convenience: make a prediction
pub fn predict(source: ConceptID, predicted: ConceptID, confidence: f32, timestamp: u64) {
    FEEDBACK_PROCESSOR.lock().predict(source, predicted, confidence, timestamp, 500);
}

/// Convenience: set an expectation
pub fn expect(concept_id: ConceptID, strength: f32, timestamp: u64) {
    FEEDBACK_PROCESSOR.lock().expect(concept_id, strength, timestamp);
}

/// Convenience: process input
pub fn process_input(concept_id: ConceptID, timestamp: u64) -> FeedbackResult {
    FEEDBACK_PROCESSOR.lock().process_input(concept_id, timestamp)
}

/// Convenience: check for omissions
pub fn check_omissions(timestamp: u64) -> Vec<ConceptID> {
    FEEDBACK_PROCESSOR.lock().check_omissions(timestamp)
}

/// Convenience: get surprise level
pub fn surprise_level() -> f32 {
    FEEDBACK_PROCESSOR.lock().surprise_level()
}

/// Convenience: get priority boost
pub fn priority_boost() -> f32 {
    FEEDBACK_PROCESSOR.lock().priority_boost()
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_prediction() {
        let mut buffer = PredictionBuffer::new();
        
        buffer.predict(ConceptID(1), 0.9, ConceptID(0), 1000, 500);
        assert_eq!(buffer.len(), 1);
        
        // Check predicted
        assert!(buffer.check_predicted(ConceptID(1)).is_some());
        
        // Already matched
        assert!(buffer.check_predicted(ConceptID(1)).is_none());
    }
    
    #[test]
    fn test_prediction_expiry() {
        let mut buffer = PredictionBuffer::new();
        
        buffer.predict(ConceptID(1), 0.9, ConceptID(0), 1000, 500);
        
        // Not expired at 1400
        let omissions = buffer.omissions(1400);
        assert!(omissions.is_empty());
        
        // Expired at 1600
        let omissions = buffer.omissions(1600);
        assert_eq!(omissions.len(), 1);
    }
    
    #[test]
    fn test_expectation_matching() {
        let mut matcher = ExpectationMatcher::new();
        
        matcher.expect(ConceptID(42), 0.8, 1000);
        
        // Match
        let (matched, strength) = matcher.match_input(ConceptID(42), 1100);
        assert!(matched);
        assert_eq!(strength, 0.8);
        
        // No longer there
        let (matched, _) = matcher.match_input(ConceptID(42), 1100);
        assert!(!matched);
    }
    
    #[test]
    fn test_surprise_detection() {
        let mut detector = SurpriseDetector::new();
        
        assert!(!detector.is_surprised());
        
        // Record unexpected event
        detector.unexpected(ConceptID(99), 1000);
        
        assert!(detector.surprise_level() > 0.0);
        assert!(detector.priority_boost() > 1.0);
    }
    
    #[test]
    fn test_feedback_processor_predicted() {
        let mut processor = FeedbackProcessor::new();
        
        // Make prediction
        processor.predict(ConceptID(0), ConceptID(1), 0.9, 1000, 500);
        
        // Process predicted input
        let result = processor.process_input(ConceptID(1), 1100);
        
        assert!(result.was_predicted);
        assert_eq!(result.surprise, 0.0);
    }
    
    #[test]
    fn test_feedback_processor_unexpected() {
        let mut processor = FeedbackProcessor::new();
        
        // Process unexpected input
        let result = processor.process_input(ConceptID(99), 1000);
        
        assert!(!result.was_predicted);
        assert!(!result.was_expected);
        assert_eq!(result.surprise, 1.0);
    }
    
    #[test]
    fn test_prediction_accuracy() {
        let mut processor = FeedbackProcessor::new();
        
        // Make predictions
        processor.predict(ConceptID(0), ConceptID(1), 0.9, 1000, 500);
        processor.predict(ConceptID(0), ConceptID(2), 0.9, 1000, 500);
        
        // One correct
        processor.process_input(ConceptID(1), 1100);
        // One missed (process different)
        processor.process_input(ConceptID(99), 1100);
        
        // 1 out of 2
        assert_eq!(processor.prediction_accuracy(), 0.5);
    }
    
    #[test]
    fn test_omission_detection() {
        let mut processor = FeedbackProcessor::new();
        
        // Make prediction
        processor.predict(ConceptID(0), ConceptID(1), 0.9, 1000, 500);
        
        // Don't process the predicted input, just check omissions after deadline
        let omissions = processor.check_omissions(1600);
        
        assert_eq!(omissions.len(), 1);
        assert_eq!(omissions[0], ConceptID(1));
    }
    
    #[test]
    fn test_stats() {
        let processor = FeedbackProcessor::new();
        let stats = processor.stats();
        
        assert_eq!(stats.total_processed, 0);
        assert_eq!(stats.prediction_accuracy, 1.0); // No predictions = 100%
    }
}
