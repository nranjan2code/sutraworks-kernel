//! Neural Scheduler Integration
//! 
//! Integrates neural intent processing with the scheduler:
//! - Intent-priority preemption (high-activation intents preempt)
//! - Urgency-based scheduling (basal ganglia model)
//! - Core affinity for intent types
//! - Graceful degradation under load
//! 
//! # Biological Inspiration
//! 
//! Models the basal ganglia's role in action selection:
//! - Striatum: Collects competing action requests
//! - GP/SNr: Tonic inhibition (default = don't act)
//! - Thalamus: Releases selected action
//! - Dopamine: Modulates urgency/reward

use alloc::vec::Vec;
use crate::intent::ConceptID;
use crate::kernel::sync::SpinLock;

// ═══════════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum pending intent requests
pub const MAX_PENDING_INTENTS: usize = 32;

/// Default urgency threshold for preemption
pub const DEFAULT_URGENCY_THRESHOLD: f32 = 0.7;

/// Maximum cores supported
pub const MAX_CORES: usize = 4;

/// Load threshold for degradation (0.0-1.0)
pub const LOAD_THRESHOLD_HIGH: f32 = 0.8;
pub const LOAD_THRESHOLD_CRITICAL: f32 = 0.95;

// ═══════════════════════════════════════════════════════════════════════════════
// INTENT PRIORITY
// ═══════════════════════════════════════════════════════════════════════════════

/// An intent scheduling request
#[derive(Clone, Copy, Debug)]
pub struct IntentRequest {
    /// The intent concept
    pub concept_id: ConceptID,
    /// Priority (0-255, higher = more important)
    pub priority: u8,
    /// Urgency (0.0-1.0, from neural activation)
    pub urgency: f32,
    /// Surprise boost (from feedback system)
    pub surprise_boost: f32,
    /// Preferred core (None = any)
    pub preferred_core: Option<u8>,
    /// Timestamp
    pub timestamp: u64,
    /// Source process ID
    pub source_pid: u64,
}

impl IntentRequest {
    /// Create new request
    pub fn new(concept_id: ConceptID, priority: u8, source_pid: u64, timestamp: u64) -> Self {
        Self {
            concept_id,
            priority,
            urgency: 0.5,
            surprise_boost: 1.0,
            preferred_core: None,
            timestamp,
            source_pid,
        }
    }
    
    /// Calculate effective priority
    /// 
    /// Combines static priority with dynamic urgency and surprise.
    pub fn effective_priority(&self) -> f32 {
        let base = self.priority as f32 / 255.0;
        let urgency_factor = self.urgency;
        let surprise_factor = self.surprise_boost;
        
        // Combine: base * urgency * surprise
        (base * urgency_factor * surprise_factor).clamp(0.0, 1.0)
    }
    
    /// Check if this request should preempt another
    pub fn should_preempt(&self, other: &IntentRequest, threshold: f32) -> bool {
        let my_priority = self.effective_priority();
        let other_priority = other.effective_priority();
        
        // Preempt if significantly higher priority
        my_priority > other_priority + threshold
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CORE AFFINITY
// ═══════════════════════════════════════════════════════════════════════════════

/// Intent type category for core affinity
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum IntentCategory {
    /// Time-critical input processing (steno, audio)
    Input = 0,
    /// Computation-heavy (perception, inference)
    Compute = 1,
    /// Output generation (display, speech)
    Output = 2,
    /// Background/housekeeping
    Background = 3,
}

impl IntentCategory {
    /// Get preferred core for this category
    pub fn preferred_core(&self) -> Option<u8> {
        match self {
            IntentCategory::Input => Some(0),      // Core 0: Input processing
            IntentCategory::Compute => Some(1),    // Core 1: Heavy compute
            IntentCategory::Output => Some(2),     // Core 2: Output
            IntentCategory::Background => None,    // Any core
        }
    }
    
    /// Categorize a concept based on its ID range
    /// 
    /// This uses the high byte of ConceptID to determine category.
    pub fn from_concept(concept_id: ConceptID) -> Self {
        let high_byte = (concept_id.0 >> 56) as u8;
        match high_byte {
            0x00..=0x1F => IntentCategory::Input,
            0x20..=0x5F => IntentCategory::Compute,
            0x60..=0x9F => IntentCategory::Output,
            _ => IntentCategory::Background,
        }
    }
}

/// Core affinity configuration
#[derive(Clone, Copy, Debug)]
pub struct CoreAffinity {
    /// Preferred categories per core
    preferred: [IntentCategory; MAX_CORES],
    /// Whether affinity is enforced (false = suggestion only)
    enforced: bool,
}

impl CoreAffinity {
    /// Create with default affinity
    pub const fn new() -> Self {
        Self {
            preferred: [
                IntentCategory::Input,
                IntentCategory::Compute,
                IntentCategory::Output,
                IntentCategory::Background,
            ],
            enforced: false,
        }
    }
    
    /// Get best core for an intent
    pub fn best_core(&self, concept_id: ConceptID) -> Option<u8> {
        let category = IntentCategory::from_concept(concept_id);
        
        // Find core that prefers this category
        for (core, &pref) in self.preferred.iter().enumerate() {
            if pref == category {
                return Some(core as u8);
            }
        }
        
        // Fallback: use category's default
        category.preferred_core()
    }
    
    /// Set affinity for a core
    pub fn set_affinity(&mut self, core: u8, category: IntentCategory) {
        if (core as usize) < MAX_CORES {
            self.preferred[core as usize] = category;
        }
    }
    
    /// Enable/disable enforcement
    pub fn set_enforced(&mut self, enforced: bool) {
        self.enforced = enforced;
    }
}

impl Default for CoreAffinity {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// URGENCY-BASED SCHEDULING (Basal Ganglia Model)
// ═══════════════════════════════════════════════════════════════════════════════

/// Urgency accumulator (like striatum)
/// 
/// Collects urgency signals and selects actions when threshold is reached.
pub struct UrgencyAccumulator {
    /// Accumulated urgency per concept
    urgencies: heapless::Vec<(ConceptID, f32, u64), 16>,
    /// Threshold for action selection
    threshold: f32,
    /// Dopamine level (modulates all urgencies)
    dopamine: f32,
    /// Inhibition level (default = don't act)
    tonic_inhibition: f32,
}

impl UrgencyAccumulator {
    /// Create new accumulator
    pub const fn new() -> Self {
        Self {
            urgencies: heapless::Vec::new(),
            threshold: DEFAULT_URGENCY_THRESHOLD,
            dopamine: 1.0,
            tonic_inhibition: 0.5,
        }
    }
    
    /// Add urgency for a concept
    pub fn accumulate(&mut self, concept_id: ConceptID, urgency: f32, timestamp: u64) {
        // Apply dopamine modulation
        let modulated = urgency * self.dopamine;
        
        // Find existing or add new
        for entry in self.urgencies.iter_mut() {
            if entry.0 == concept_id {
                entry.1 = (entry.1 + modulated).min(1.0);
                entry.2 = timestamp;
                return;
            }
        }
        
        // Add new
        if self.urgencies.len() < 16 {
            self.urgencies.push((concept_id, modulated, timestamp)).ok();
        }
    }
    
    /// Select action (if any exceeds threshold)
    /// 
    /// Returns the selected concept if urgency exceeds threshold.
    pub fn select_action(&mut self) -> Option<ConceptID> {
        let effective_threshold = self.threshold + self.tonic_inhibition;
        
        // Find highest urgency above threshold
        let mut best: Option<(usize, f32)> = None;
        
        for (i, &(_, urgency, _)) in self.urgencies.iter().enumerate() {
            if urgency >= effective_threshold {
                match best {
                    None => best = Some((i, urgency)),
                    Some((_, best_urgency)) if urgency > best_urgency => {
                        best = Some((i, urgency));
                    }
                    _ => {}
                }
            }
        }
        
        // Remove and return selected
        if let Some((idx, _)) = best {
            let (concept_id, _, _) = self.urgencies.swap_remove(idx);
            Some(concept_id)
        } else {
            None
        }
    }
    
    /// Decay all urgencies
    pub fn decay(&mut self, rate: f32) {
        for entry in self.urgencies.iter_mut() {
            entry.1 = (entry.1 - rate).max(0.0);
        }
        
        // Remove zero-urgency entries
        self.urgencies.retain(|&(_, u, _)| u > 0.01);
    }
    
    /// Set dopamine level (reward signal)
    pub fn set_dopamine(&mut self, level: f32) {
        self.dopamine = level.clamp(0.1, 2.0);
    }
    
    /// Set tonic inhibition (default = don't act strength)
    pub fn set_tonic_inhibition(&mut self, level: f32) {
        self.tonic_inhibition = level.clamp(0.0, 1.0);
    }
    
    /// Get current urgencies
    pub fn current_urgencies(&self) -> &[(ConceptID, f32, u64)] {
        &self.urgencies
    }
    
    /// Clear all urgencies
    pub fn clear(&mut self) {
        self.urgencies.clear();
    }
}

impl Default for UrgencyAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GRACEFUL DEGRADATION
// ═══════════════════════════════════════════════════════════════════════════════

/// System load level
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LoadLevel {
    /// Normal operation
    #[default]
    Normal,
    /// High load - start degradation
    High,
    /// Critical load - aggressive degradation
    Critical,
}

/// Degradation policy
#[derive(Clone, Copy, Debug)]
pub struct DegradationPolicy {
    /// Current load level
    pub load_level: LoadLevel,
    /// Whether to skip non-essential intents
    pub skip_background: bool,
    /// Whether to reduce perception fidelity
    pub reduce_perception: bool,
    /// Minimum priority to process
    pub min_priority: u8,
    /// Time slice reduction factor (1.0 = normal)
    pub time_slice_factor: f32,
}

impl DegradationPolicy {
    /// Create with default settings
    pub const fn new() -> Self {
        Self {
            load_level: LoadLevel::Normal,
            skip_background: false,
            reduce_perception: false,
            min_priority: 0,
            time_slice_factor: 1.0,
        }
    }
    
    /// Update based on load
    pub fn update_for_load(&mut self, load: f32) {
        if load >= LOAD_THRESHOLD_CRITICAL {
            self.load_level = LoadLevel::Critical;
            self.skip_background = true;
            self.reduce_perception = true;
            self.min_priority = 128;
            self.time_slice_factor = 0.5;
        } else if load >= LOAD_THRESHOLD_HIGH {
            self.load_level = LoadLevel::High;
            self.skip_background = true;
            self.reduce_perception = false;
            self.min_priority = 64;
            self.time_slice_factor = 0.75;
        } else {
            self.load_level = LoadLevel::Normal;
            self.skip_background = false;
            self.reduce_perception = false;
            self.min_priority = 0;
            self.time_slice_factor = 1.0;
        }
    }
    
    /// Check if an intent should be processed
    pub fn should_process(&self, request: &IntentRequest) -> bool {
        // Check minimum priority
        if request.priority < self.min_priority {
            return false;
        }
        
        // Check background skip
        if self.skip_background {
            let category = IntentCategory::from_concept(request.concept_id);
            if category == IntentCategory::Background {
                return false;
            }
        }
        
        true
    }
    
    /// Get adjusted time slice
    pub fn adjusted_time_slice(&self, base_slice: u64) -> u64 {
        ((base_slice as f32) * self.time_slice_factor) as u64
    }
}

impl Default for DegradationPolicy {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// NEURAL SCHEDULER
// ═══════════════════════════════════════════════════════════════════════════════

/// Neural-aware scheduler extension
/// 
/// Works alongside the main IntentScheduler to provide neural features.
pub struct NeuralScheduler {
    /// Pending intent requests (priority queue)
    pending: heapless::Vec<IntentRequest, MAX_PENDING_INTENTS>,
    /// Core affinity configuration
    affinity: CoreAffinity,
    /// Urgency accumulator (basal ganglia)
    urgency: UrgencyAccumulator,
    /// Degradation policy
    degradation: DegradationPolicy,
    /// Preemption threshold
    preemption_threshold: f32,
    /// Current load estimate (0.0-1.0)
    current_load: f32,
    /// Total requests processed
    total_processed: u64,
    /// Total preemptions
    total_preemptions: u64,
}

impl NeuralScheduler {
    /// Create new neural scheduler
    pub const fn new() -> Self {
        Self {
            pending: heapless::Vec::new(),
            affinity: CoreAffinity::new(),
            urgency: UrgencyAccumulator::new(),
            degradation: DegradationPolicy::new(),
            preemption_threshold: DEFAULT_URGENCY_THRESHOLD,
            current_load: 0.0,
            total_processed: 0,
            total_preemptions: 0,
        }
    }
    
    /// Submit an intent request
    pub fn submit(&mut self, mut request: IntentRequest) -> bool {
        // Apply degradation policy
        if !self.degradation.should_process(&request) {
            return false;
        }
        
        // Set preferred core from affinity
        if request.preferred_core.is_none() {
            request.preferred_core = self.affinity.best_core(request.concept_id);
        }
        
        // Add to urgency accumulator
        self.urgency.accumulate(request.concept_id, request.urgency, request.timestamp);
        
        // Add to pending queue (sorted by effective priority)
        if self.pending.len() < MAX_PENDING_INTENTS {
            self.pending.push(request).ok();
            self.sort_pending();
            true
        } else {
            // Queue full - check if we should replace lowest
            if let Some(lowest) = self.pending.last() {
                if request.effective_priority() > lowest.effective_priority() {
                    self.pending.pop();
                    self.pending.push(request).ok();
                    self.sort_pending();
                    return true;
                }
            }
            false
        }
    }
    
    /// Sort pending by effective priority (highest first)
    fn sort_pending(&mut self) {
        // Simple insertion sort (small array)
        for i in 1..self.pending.len() {
            let key = self.pending[i];
            let key_priority = key.effective_priority();
            let mut j = i;
            while j > 0 && self.pending[j - 1].effective_priority() > key_priority {
                self.pending[j] = self.pending[j - 1];
                j -= 1;
            }
            self.pending[j] = key;
        }
    }
    
    /// Get next intent to process
    pub fn next(&mut self) -> Option<IntentRequest> {
        if let Some(request) = self.pending.pop() {
            self.total_processed += 1;
            Some(request)
        } else {
            None
        }
    }
    
    /// Get next intent for a specific core
    pub fn next_for_core(&mut self, core_id: u8) -> Option<IntentRequest> {
        // Find highest priority request that prefers this core
        let mut best_idx: Option<usize> = None;
        let mut best_priority: f32 = 0.0;
        
        for (i, request) in self.pending.iter().enumerate() {
            if request.preferred_core == Some(core_id) || request.preferred_core.is_none() {
                let priority = request.effective_priority();
                if best_idx.is_none() || priority > best_priority {
                    best_idx = Some(i);
                    best_priority = priority;
                }
            }
        }
        
        if let Some(idx) = best_idx {
            let request = self.pending.swap_remove(idx);
            self.total_processed += 1;
            Some(request)
        } else {
            None
        }
    }
    
    /// Check if preemption should occur
    pub fn should_preempt(&self, current: &IntentRequest) -> Option<&IntentRequest> {
        if let Some(highest) = self.pending.last() { // `last()` is highest priority due to ascending sort
            if highest.should_preempt(current, self.preemption_threshold) {
                return Some(highest);
            }
        }
        None
    }
    
    /// Update load estimate
    pub fn update_load(&mut self, load: f32) {
        self.current_load = load.clamp(0.0, 1.0);
        self.degradation.update_for_load(load);
    }
    
    /// Tick - decay urgencies and update state
    pub fn tick(&mut self, timestamp: u64) {
        let _ = timestamp; // Reserved for future use
        self.urgency.decay(0.01);
    }
    
    /// Get pending count
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
    
    /// Get current load
    pub fn load(&self) -> f32 {
        self.current_load
    }
    
    /// Get degradation policy
    pub fn degradation(&self) -> &DegradationPolicy {
        &self.degradation
    }
    
    /// Get mutable degradation policy
    pub fn degradation_mut(&mut self) -> &mut DegradationPolicy {
        &mut self.degradation
    }
    
    /// Get affinity config
    pub fn affinity(&self) -> &CoreAffinity {
        &self.affinity
    }
    
    /// Get mutable affinity config
    pub fn affinity_mut(&mut self) -> &mut CoreAffinity {
        &mut self.affinity
    }
    
    /// Get urgency accumulator
    pub fn urgency(&self) -> &UrgencyAccumulator {
        &self.urgency
    }
    
    /// Get mutable urgency accumulator
    pub fn urgency_mut(&mut self) -> &mut UrgencyAccumulator {
        &mut self.urgency
    }
    
    /// Set preemption threshold
    pub fn set_preemption_threshold(&mut self, threshold: f32) {
        self.preemption_threshold = threshold.clamp(0.1, 1.0);
    }
    
    /// Get statistics
    pub fn stats(&self) -> NeuralSchedulerStats {
        NeuralSchedulerStats {
            pending_count: self.pending.len(),
            total_processed: self.total_processed,
            total_preemptions: self.total_preemptions,
            current_load: self.current_load,
            load_level: self.degradation.load_level,
            urgency_count: self.urgency.current_urgencies().len(),
        }
    }
    
    /// Clear all state
    pub fn clear(&mut self) {
        self.pending.clear();
        self.urgency.clear();
    }
}

impl Default for NeuralScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// STATISTICS
// ═══════════════════════════════════════════════════════════════════════════════

/// Statistics for neural scheduler
#[derive(Clone, Copy, Debug, Default)]
pub struct NeuralSchedulerStats {
    /// Number of pending requests
    pub pending_count: usize,
    /// Total requests processed
    pub total_processed: u64,
    /// Total preemptions performed
    pub total_preemptions: u64,
    /// Current load estimate
    pub current_load: f32,
    /// Current load level
    pub load_level: LoadLevel,
    /// Number of urgency entries
    pub urgency_count: usize,
}

// LoadLevel implements Default via #[derive(Default)] with #[default] on Normal variant

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL INSTANCE
// ═══════════════════════════════════════════════════════════════════════════════

/// Global neural scheduler
pub static NEURAL_SCHEDULER: SpinLock<NeuralScheduler> = SpinLock::new(NeuralScheduler::new());

/// Convenience: submit an intent request
pub fn submit_intent(request: IntentRequest) -> bool {
    NEURAL_SCHEDULER.lock().submit(request)
}

/// Convenience: get next intent
pub fn next_intent() -> Option<IntentRequest> {
    NEURAL_SCHEDULER.lock().next()
}

/// Convenience: get next intent for core
pub fn next_intent_for_core(core_id: u8) -> Option<IntentRequest> {
    NEURAL_SCHEDULER.lock().next_for_core(core_id)
}

/// Convenience: update load
pub fn update_load(load: f32) {
    NEURAL_SCHEDULER.lock().update_load(load);
}

/// Convenience: tick
pub fn scheduler_tick(timestamp: u64) {
    NEURAL_SCHEDULER.lock().tick(timestamp);
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_intent_request_priority() {
        let request = IntentRequest {
            concept_id: ConceptID(1),
            priority: 128,
            urgency: 0.8,
            surprise_boost: 1.2,
            preferred_core: None,
            timestamp: 0,
            source_pid: 0,
        };
        
        let eff = request.effective_priority();
        assert!(eff > 0.0 && eff <= 1.0);
    }
    
    #[test]
    fn test_preemption() {
        let low = IntentRequest::new(ConceptID(1), 64, 0, 0);
        let high = IntentRequest {
            priority: 200,
            urgency: 0.9,
            ..IntentRequest::new(ConceptID(2), 200, 0, 0)
        };
        
        assert!(high.should_preempt(&low, 0.2));
        assert!(!low.should_preempt(&high, 0.2));
    }
    
    #[test]
    fn test_core_affinity() {
        let affinity = CoreAffinity::new();
        
        // Input category should prefer core 0
        let input_concept = ConceptID(0x0010_0000_0000_0000);
        assert_eq!(affinity.best_core(input_concept), Some(0));
    }
    
    #[test]
    fn test_urgency_accumulator() {
        let mut urgency = UrgencyAccumulator::new();
        
        urgency.accumulate(ConceptID(1), 0.3, 1000);
        urgency.accumulate(ConceptID(1), 0.3, 1001);
        urgency.accumulate(ConceptID(1), 0.3, 1002);
        
        // Should have accumulated
        let urgencies = urgency.current_urgencies();
        assert_eq!(urgencies.len(), 1);
        assert!(urgencies[0].1 > 0.5);
    }
    
    #[test]
    fn test_action_selection() {
        let mut urgency = UrgencyAccumulator::new();
        urgency.set_tonic_inhibition(0.0); // Remove inhibition for test
        urgency.threshold = 0.5;
        
        urgency.accumulate(ConceptID(1), 0.8, 1000);
        
        let selected = urgency.select_action();
        assert_eq!(selected, Some(ConceptID(1)));
    }
    
    #[test]
    fn test_degradation_policy() {
        let mut policy = DegradationPolicy::new();
        
        // Normal load
        policy.update_for_load(0.5);
        assert_eq!(policy.load_level, LoadLevel::Normal);
        assert!(!policy.skip_background);
        
        // High load
        policy.update_for_load(0.85);
        assert_eq!(policy.load_level, LoadLevel::High);
        assert!(policy.skip_background);
        
        // Critical load
        policy.update_for_load(0.98);
        assert_eq!(policy.load_level, LoadLevel::Critical);
        assert!(policy.reduce_perception);
    }
    
    #[test]
    fn test_neural_scheduler_submit() {
        let mut scheduler = NeuralScheduler::new();
        
        let request = IntentRequest::new(ConceptID(1), 100, 0, 1000);
        assert!(scheduler.submit(request));
        assert_eq!(scheduler.pending_count(), 1);
    }
    
    #[test]
    fn test_neural_scheduler_priority_order() {
        let mut scheduler = NeuralScheduler::new();
        
        // Submit low priority
        let low = IntentRequest::new(ConceptID(1), 50, 0, 1000);
        scheduler.submit(low);
        
        // Submit high priority
        let high = IntentRequest::new(ConceptID(2), 200, 0, 1001);
        scheduler.submit(high);
        
        // Should get high priority first
        let next = scheduler.next().unwrap();
        assert_eq!(next.concept_id, ConceptID(2));
    }
    
    #[test]
    fn test_stats() {
        let scheduler = NeuralScheduler::new();
        let stats = scheduler.stats();
        
        assert_eq!(stats.pending_count, 0);
        assert_eq!(stats.total_processed, 0);
        assert_eq!(stats.load_level, LoadLevel::Normal);
    }
}
