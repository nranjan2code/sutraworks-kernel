//! Hierarchical Processing Module
//! 
//! Implements cortex-inspired hierarchical processing:
//! - Layer-to-layer propagation (bottom-up and top-down)
//! - Top-down modulation (goals affect perception)
//! - Attention mechanism (selective focus)
//! 
//! # Biological Inspiration
//! 
//! Like the visual cortex: V1 → V2 → V4 → IT → PFC
//! Each layer extracts higher-level features.
//! Top-down connections modulate lower layers based on goals.

use alloc::vec::Vec;
use crate::intent::{ConceptID, Intent, IntentLevel};
use crate::kernel::sync::SpinLock;

// ═══════════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Number of processing layers
pub const NUM_LAYERS: usize = 5;

/// Maximum intents per layer buffer
pub const MAX_LAYER_INTENTS: usize = 32;

/// Default attention capacity (how many can be focused)
pub const DEFAULT_ATTENTION_CAPACITY: usize = 4;

/// Default propagation gain (how much signal passes between layers)
pub const DEFAULT_PROPAGATION_GAIN: f32 = 0.8;

/// Default modulation strength (how much goals affect perception)
pub const DEFAULT_MODULATION_STRENGTH: f32 = 0.3;

// ═══════════════════════════════════════════════════════════════════════════════
// LAYER BUFFER
// ═══════════════════════════════════════════════════════════════════════════════

/// Buffer for intents at a specific processing layer
#[derive(Clone)]
pub struct LayerBuffer {
    /// Intents currently at this layer
    intents: heapless::Vec<(ConceptID, f32), MAX_LAYER_INTENTS>,
    /// Layer level
    level: IntentLevel,
    /// Gain for propagating to next layer (0.0-1.0)
    propagation_gain: f32,
    /// Modulation from higher layers (multiplier)
    modulation: f32,
    /// Number of intents processed through this layer
    processed_count: u64,
}

impl LayerBuffer {
    /// Create a new layer buffer
    pub const fn new(level: IntentLevel) -> Self {
        Self {
            intents: heapless::Vec::new(),
            level,
            propagation_gain: 0.8,
            modulation: 1.0,
            processed_count: 0,
        }
    }
    
    /// Add an intent to this layer
    pub fn push(&mut self, concept_id: ConceptID, activation: f32) -> bool {
        // Apply modulation to incoming activation
        let modulated_activation = activation * self.modulation;
        self.intents.push((concept_id, modulated_activation)).is_ok()
    }
    
    /// Get all intents at this layer
    pub fn intents(&self) -> &[(ConceptID, f32)] {
        &self.intents
    }
    
    /// Get intents above activation threshold
    pub fn active_intents(&self, threshold: f32) -> Vec<(ConceptID, f32)> {
        self.intents
            .iter()
            .filter(|(_, act)| *act >= threshold)
            .copied()
            .collect()
    }
    
    /// Clear the layer buffer
    pub fn clear(&mut self) {
        self.intents.clear();
    }
    
    /// Set modulation from higher layer
    pub fn set_modulation(&mut self, modulation: f32) {
        self.modulation = modulation.clamp(0.1, 2.0);
    }
    
    /// Get current modulation
    pub fn modulation(&self) -> f32 {
        self.modulation
    }
    
    /// Get layer level
    pub fn level(&self) -> IntentLevel {
        self.level
    }
    
    /// Get count of intents
    pub fn len(&self) -> usize {
        self.intents.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.intents.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ATTENTION FOCUS
// ═══════════════════════════════════════════════════════════════════════════════

/// Attention focus - what the system is currently attending to
/// 
/// # Biological Analogy
/// 
/// Like the attentional spotlight in visual processing.
/// Attended items get boosted processing, unattended items are suppressed.
#[derive(Clone, Debug)]
pub struct AttentionFocus {
    /// Currently attended concepts (limited capacity)
    attended: heapless::Vec<ConceptID, 8>,
    /// Attention weights for each attended concept
    weights: heapless::Vec<f32, 8>,
    /// Maximum attention capacity
    capacity: usize,
    /// Global attention gain (1.0 = normal, >1.0 = hyper-focused)
    global_gain: f32,
    /// Suppression factor for unattended items
    suppression: f32,
}

impl AttentionFocus {
    /// Create with default settings
    pub const fn new() -> Self {
        Self {
            attended: heapless::Vec::new(),
            weights: heapless::Vec::new(),
            capacity: DEFAULT_ATTENTION_CAPACITY,
            global_gain: 1.0,
            suppression: 0.3, // Unattended items get 30% of normal activation
        }
    }
    
    /// Create with custom capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity: capacity.min(8),
            ..Self::new()
        }
    }
    
    /// Focus attention on a concept
    /// 
    /// If at capacity, the lowest-weighted item is removed.
    pub fn focus(&mut self, concept_id: ConceptID, weight: f32) {
        // Check if already attended
        for i in 0..self.attended.len() {
            if self.attended[i] == concept_id {
                // Update weight
                self.weights[i] = (self.weights[i] + weight).min(1.0);
                return;
            }
        }
        
        // Add new item
        if self.attended.len() < self.capacity {
            self.attended.push(concept_id).ok();
            self.weights.push(weight).ok();
        } else {
            // Replace lowest weight
            if let Some((min_idx, _)) = self.weights
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(core::cmp::Ordering::Equal))
            {
                if weight > self.weights[min_idx] {
                    self.attended[min_idx] = concept_id;
                    self.weights[min_idx] = weight;
                }
            }
        }
    }
    
    /// Remove attention from a concept
    pub fn unfocus(&mut self, concept_id: ConceptID) {
        if let Some(idx) = self.attended.iter().position(|&c| c == concept_id) {
            self.attended.swap_remove(idx);
            self.weights.swap_remove(idx);
        }
    }
    
    /// Clear all attention
    pub fn clear(&mut self) {
        self.attended.clear();
        self.weights.clear();
    }
    
    /// Check if a concept is attended
    pub fn is_attended(&self, concept_id: ConceptID) -> bool {
        self.attended.contains(&concept_id)
    }
    
    /// Get attention weight for a concept
    pub fn get_weight(&self, concept_id: ConceptID) -> f32 {
        for i in 0..self.attended.len() {
            if self.attended[i] == concept_id {
                return self.weights[i] * self.global_gain;
            }
        }
        self.suppression // Unattended items get suppression factor
    }
    
    /// Apply attention modulation to an activation
    pub fn modulate(&self, concept_id: ConceptID, activation: f32) -> f32 {
        activation * self.get_weight(concept_id)
    }
    
    /// Get all attended concepts
    pub fn attended_concepts(&self) -> &[ConceptID] {
        &self.attended
    }
    
    /// Get number of attended items
    pub fn len(&self) -> usize {
        self.attended.len()
    }
    
    /// Check if attention is empty
    pub fn is_empty(&self) -> bool {
        self.attended.is_empty()
    }
    
    /// Set global gain (focus intensity)
    pub fn set_global_gain(&mut self, gain: f32) {
        self.global_gain = gain.clamp(0.5, 3.0);
    }
    
    /// Set suppression factor for unattended items
    pub fn set_suppression(&mut self, suppression: f32) {
        self.suppression = suppression.clamp(0.0, 1.0);
    }
}

impl Default for AttentionFocus {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GOAL STATE (Top-Down Modulation)
// ═══════════════════════════════════════════════════════════════════════════════

/// Current goal state - affects top-down modulation
/// 
/// # Biological Analogy
/// 
/// Like prefrontal cortex maintaining goal representations
/// that modulate sensory processing in lower areas.
#[derive(Clone, Debug)]
pub struct GoalState {
    /// Active goals (concept IDs)
    goals: heapless::Vec<ConceptID, 4>,
    /// Priority weights for each goal
    priorities: heapless::Vec<f32, 4>,
    /// How much goals modulate perception (0.0-1.0)
    modulation_strength: f32,
}

impl GoalState {
    /// Create with default settings
    pub const fn new() -> Self {
        Self {
            goals: heapless::Vec::new(),
            priorities: heapless::Vec::new(),
            modulation_strength: DEFAULT_MODULATION_STRENGTH,
        }
    }
    
    /// Set a goal (replaces if already exists)
    pub fn set_goal(&mut self, goal: ConceptID, priority: f32) {
        // Check if already exists
        for i in 0..self.goals.len() {
            if self.goals[i] == goal {
                self.priorities[i] = priority;
                return;
            }
        }
        
        // Add new goal
        if self.goals.len() < 4 {
            self.goals.push(goal).ok();
            self.priorities.push(priority).ok();
        } else {
            // Replace lowest priority
            if let Some((min_idx, _)) = self.priorities
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(core::cmp::Ordering::Equal))
            {
                if priority > self.priorities[min_idx] {
                    self.goals[min_idx] = goal;
                    self.priorities[min_idx] = priority;
                }
            }
        }
    }
    
    /// Remove a goal
    pub fn remove_goal(&mut self, goal: ConceptID) {
        if let Some(idx) = self.goals.iter().position(|&g| g == goal) {
            self.goals.swap_remove(idx);
            self.priorities.swap_remove(idx);
        }
    }
    
    /// Clear all goals
    pub fn clear(&mut self) {
        self.goals.clear();
        self.priorities.clear();
    }
    
    /// Check if a concept is goal-relevant
    /// 
    /// Returns modulation factor (1.0 = neutral, >1.0 = boost, <1.0 = suppress)
    pub fn relevance(&self, concept_id: ConceptID) -> f32 {
        for i in 0..self.goals.len() {
            if self.goals[i] == concept_id {
                // Goal-relevant concepts get boosted
                return 1.0 + (self.priorities[i] * self.modulation_strength);
            }
        }
        // Non-goal concepts are slightly suppressed
        1.0 - (self.modulation_strength * 0.5)
    }
    
    /// Get all goals
    pub fn goals(&self) -> &[ConceptID] {
        &self.goals
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.goals.is_empty()
    }
    
    /// Set modulation strength
    pub fn set_modulation_strength(&mut self, strength: f32) {
        self.modulation_strength = strength.clamp(0.0, 1.0);
    }
}

impl Default for GoalState {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// HIERARCHICAL PROCESSOR
// ═══════════════════════════════════════════════════════════════════════════════

/// Hierarchical intent processor
/// 
/// Manages layer-to-layer propagation with attention and goal modulation.
/// 
/// # Architecture
/// 
/// ```text
/// Raw → Feature → Object → Semantic → Action
///   ↑       ↑        ↑         ↑
///   └───────┴────────┴─────────┘
///        Top-Down Modulation
/// ```
pub struct HierarchicalProcessor {
    /// Layer buffers (one per level)
    layers: [LayerBuffer; NUM_LAYERS],
    /// Attention mechanism
    attention: AttentionFocus,
    /// Goal state for top-down modulation
    goals: GoalState,
    /// Propagation gain between layers
    propagation_gain: f32,
    /// Whether to apply attention during propagation
    use_attention: bool,
    /// Whether to apply goal modulation
    use_goal_modulation: bool,
    /// Total intents propagated
    total_propagated: u64,
}

impl HierarchicalProcessor {
    /// Create with default settings
    pub const fn new() -> Self {
        Self {
            layers: [
                LayerBuffer::new(IntentLevel::Raw),
                LayerBuffer::new(IntentLevel::Feature),
                LayerBuffer::new(IntentLevel::Object),
                LayerBuffer::new(IntentLevel::Semantic),
                LayerBuffer::new(IntentLevel::Action),
            ],
            attention: AttentionFocus::new(),
            goals: GoalState::new(),
            propagation_gain: DEFAULT_PROPAGATION_GAIN,
            use_attention: true,
            use_goal_modulation: true,
            total_propagated: 0,
        }
    }
    
    /// Input an intent at its designated level
    pub fn input(&mut self, intent: &Intent) {
        let layer_idx = intent.level as usize;
        if layer_idx < NUM_LAYERS {
            self.layers[layer_idx].push(intent.concept_id, intent.activation);
        }
    }
    
    /// Propagate intents from one layer to the next (bottom-up)
    /// 
    /// Returns the number of intents that passed threshold.
    pub fn propagate_up(&mut self, from_level: IntentLevel) -> usize {
        let from_idx = from_level as usize;
        let to_idx = from_idx + 1;
        
        if to_idx >= NUM_LAYERS {
            return 0;
        }
        
        let threshold = 0.3; // Minimum activation to propagate
        let mut propagated = 0;
        
        // Collect intents to propagate (to avoid borrow issues)
        let to_propagate: Vec<_> = self.layers[from_idx]
            .active_intents(threshold)
            .into_iter()
            .map(|(id, act)| {
                // Apply attention modulation
                let attended_act = if self.use_attention {
                    self.attention.modulate(id, act)
                } else {
                    act
                };
                
                // Apply goal modulation
                let goal_act = if self.use_goal_modulation {
                    attended_act * self.goals.relevance(id)
                } else {
                    attended_act
                };
                
                // Apply propagation gain
                (id, goal_act * self.propagation_gain)
            })
            .filter(|(_, act)| *act >= threshold)
            .collect();
        
        // Push to next layer
        for (id, act) in to_propagate {
            if self.layers[to_idx].push(id, act) {
                propagated += 1;
                self.total_propagated += 1;
            }
        }
        
        propagated
    }
    
    /// Propagate through all layers (full bottom-up pass)
    /// 
    /// Returns total number of intents that reached Action layer.
    pub fn propagate_all(&mut self) -> usize {
        let mut reached_action = 0;
        
        // Propagate Raw → Feature → Object → Semantic → Action
        for level in [IntentLevel::Raw, IntentLevel::Feature, IntentLevel::Object, IntentLevel::Semantic] {
            let count = self.propagate_up(level);
            if level == IntentLevel::Semantic {
                reached_action = count;
            }
        }
        
        reached_action
    }
    
    /// Apply top-down modulation from goals
    /// 
    /// Higher layers modulate lower layers based on current goals.
    pub fn apply_top_down_modulation(&mut self) {
        if self.goals.is_empty() {
            // Reset all modulation to neutral
            for layer in &mut self.layers {
                layer.set_modulation(1.0);
            }
            return;
        }
        
        // Calculate modulation for each layer
        // Lower layers get more modulation from goals
        let modulation_factors = [
            1.5, // Raw: heavily modulated by goals
            1.3, // Feature
            1.2, // Object
            1.1, // Semantic
            1.0, // Action: minimal goal modulation
        ];
        
        for (idx, layer) in self.layers.iter_mut().enumerate() {
            // Check if any goals are present in this layer
            let has_goal_relevant = layer.intents()
                .iter()
                .any(|(id, _)| self.goals.goals().contains(id));
            
            if has_goal_relevant {
                layer.set_modulation(modulation_factors[idx]);
            } else {
                layer.set_modulation(1.0 / modulation_factors[idx]); // Suppress non-goal
            }
        }
    }
    
    /// Focus attention on a concept
    pub fn attend(&mut self, concept_id: ConceptID) {
        self.attention.focus(concept_id, 1.0);
    }
    
    /// Focus attention with specific weight
    pub fn attend_with_weight(&mut self, concept_id: ConceptID, weight: f32) {
        self.attention.focus(concept_id, weight);
    }
    
    /// Remove attention from a concept
    pub fn unattend(&mut self, concept_id: ConceptID) {
        self.attention.unfocus(concept_id);
    }
    
    /// Set a goal
    pub fn set_goal(&mut self, goal: ConceptID, priority: f32) {
        self.goals.set_goal(goal, priority);
        self.apply_top_down_modulation();
    }
    
    /// Remove a goal
    pub fn remove_goal(&mut self, goal: ConceptID) {
        self.goals.remove_goal(goal);
        self.apply_top_down_modulation();
    }
    
    /// Get intents at a specific layer
    pub fn get_layer(&self, level: IntentLevel) -> &LayerBuffer {
        &self.layers[level as usize]
    }
    
    /// Get mutable access to a layer
    pub fn get_layer_mut(&mut self, level: IntentLevel) -> &mut LayerBuffer {
        &mut self.layers[level as usize]
    }
    
    /// Get action-level intents (final output)
    pub fn get_actions(&self) -> Vec<(ConceptID, f32)> {
        self.layers[IntentLevel::Action as usize].active_intents(0.3)
    }
    
    /// Clear all layers
    pub fn clear_all(&mut self) {
        for layer in &mut self.layers {
            layer.clear();
        }
    }
    
    /// Clear a specific layer
    pub fn clear_layer(&mut self, level: IntentLevel) {
        self.layers[level as usize].clear();
    }
    
    /// Get attention state
    pub fn attention(&self) -> &AttentionFocus {
        &self.attention
    }
    
    /// Get mutable attention state
    pub fn attention_mut(&mut self) -> &mut AttentionFocus {
        &mut self.attention
    }
    
    /// Get goal state
    pub fn goals(&self) -> &GoalState {
        &self.goals
    }
    
    /// Get mutable goal state
    pub fn goals_mut(&mut self) -> &mut GoalState {
        &mut self.goals
    }
    
    /// Get statistics
    pub fn stats(&self) -> HierarchicalStats {
        HierarchicalStats {
            layer_counts: [
                self.layers[0].len(),
                self.layers[1].len(),
                self.layers[2].len(),
                self.layers[3].len(),
                self.layers[4].len(),
            ],
            attended_count: self.attention.len(),
            goal_count: self.goals.goals().len(),
            total_propagated: self.total_propagated,
        }
    }
    
    /// Enable/disable attention
    pub fn set_use_attention(&mut self, enabled: bool) {
        self.use_attention = enabled;
    }
    
    /// Enable/disable goal modulation
    pub fn set_use_goal_modulation(&mut self, enabled: bool) {
        self.use_goal_modulation = enabled;
    }
    
    /// Set propagation gain
    pub fn set_propagation_gain(&mut self, gain: f32) {
        self.propagation_gain = gain.clamp(0.1, 1.0);
    }
}

impl Default for HierarchicalProcessor {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// STATISTICS
// ═══════════════════════════════════════════════════════════════════════════════

/// Statistics for hierarchical processing
#[derive(Clone, Copy, Debug, Default)]
pub struct HierarchicalStats {
    /// Number of intents at each layer [Raw, Feature, Object, Semantic, Action]
    pub layer_counts: [usize; NUM_LAYERS],
    /// Number of attended concepts
    pub attended_count: usize,
    /// Number of active goals
    pub goal_count: usize,
    /// Total intents propagated across all layers
    pub total_propagated: u64,
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL INSTANCE
// ═══════════════════════════════════════════════════════════════════════════════

/// Global hierarchical processor
pub static HIERARCHICAL_PROCESSOR: SpinLock<HierarchicalProcessor> = 
    SpinLock::new(HierarchicalProcessor::new());

/// Convenience: input an intent to hierarchical processor
pub fn input_intent(intent: &Intent) {
    HIERARCHICAL_PROCESSOR.lock().input(intent);
}

/// Convenience: propagate all layers
pub fn propagate_all() -> usize {
    HIERARCHICAL_PROCESSOR.lock().propagate_all()
}

/// Convenience: focus attention
pub fn attend(concept_id: ConceptID) {
    HIERARCHICAL_PROCESSOR.lock().attend(concept_id);
}

/// Convenience: set goal
pub fn set_goal(goal: ConceptID, priority: f32) {
    HIERARCHICAL_PROCESSOR.lock().set_goal(goal, priority);
}

/// Convenience: get actions
pub fn get_actions() -> Vec<(ConceptID, f32)> {
    HIERARCHICAL_PROCESSOR.lock().get_actions()
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_layer_buffer() {
        let mut layer = LayerBuffer::new(IntentLevel::Feature);
        
        assert!(layer.is_empty());
        assert!(layer.push(ConceptID(1), 0.5));
        assert_eq!(layer.len(), 1);
        
        let active = layer.active_intents(0.3);
        assert_eq!(active.len(), 1);
    }
    
    #[test]
    fn test_attention_focus() {
        let mut attention = AttentionFocus::new();
        
        assert!(attention.is_empty());
        
        attention.focus(ConceptID(1), 1.0);
        assert!(attention.is_attended(ConceptID(1)));
        assert!(!attention.is_attended(ConceptID(2)));
        
        // Attended items get full weight
        assert_eq!(attention.get_weight(ConceptID(1)), 1.0);
        // Unattended items get suppression
        assert!(attention.get_weight(ConceptID(2)) < 1.0);
    }
    
    #[test]
    fn test_attention_capacity() {
        let mut attention = AttentionFocus::with_capacity(2);
        
        attention.focus(ConceptID(1), 0.5);
        attention.focus(ConceptID(2), 0.7);
        attention.focus(ConceptID(3), 0.9); // Should replace lowest
        
        assert_eq!(attention.len(), 2);
        assert!(attention.is_attended(ConceptID(3)));
        assert!(attention.is_attended(ConceptID(2)));
        assert!(!attention.is_attended(ConceptID(1))); // Lowest was replaced
    }
    
    #[test]
    fn test_goal_state() {
        let mut goals = GoalState::new();
        
        assert!(goals.is_empty());
        
        goals.set_goal(ConceptID(100), 1.0);
        
        // Goal-relevant concepts get boosted
        let relevance = goals.relevance(ConceptID(100));
        assert!(relevance > 1.0);
        
        // Non-goal concepts are suppressed
        let irrelevance = goals.relevance(ConceptID(200));
        assert!(irrelevance < 1.0);
    }
    
    #[test]
    fn test_hierarchical_processor() {
        let mut processor = HierarchicalProcessor::new();
        
        // Create a test intent
        let intent = Intent {
            concept_id: ConceptID(42),
            activation: 0.8,
            level: IntentLevel::Raw,
            ..Intent::new(ConceptID(42))
        };
        
        processor.input(&intent);
        processor.set_use_attention(false); // Disable attention to pass propagation threshold
        
        assert_eq!(processor.get_layer(IntentLevel::Raw).len(), 1);
        
        // Propagate up
        let count = processor.propagate_up(IntentLevel::Raw);
        assert!(count > 0);
        
        assert_eq!(processor.get_layer(IntentLevel::Feature).len(), 1);
    }
    
    #[test]
    fn test_full_propagation() {
        let mut processor = HierarchicalProcessor::new();
        processor.set_use_attention(false);
        processor.set_use_goal_modulation(false);
        
        // Input at Raw level with high activation
        let intent = Intent {
            concept_id: ConceptID(42),
            activation: 1.0,
            level: IntentLevel::Raw,
            ..Intent::new(ConceptID(42))
        };
        
        processor.input(&intent);
        
        // Propagate all the way
        processor.propagate_all();
        
        // Should reach Action level (with some decay)
        let actions = processor.get_actions();
        assert!(!actions.is_empty());
    }
    
    #[test]
    fn test_stats() {
        let processor = HierarchicalProcessor::new();
        let stats = processor.stats();
        
        assert_eq!(stats.layer_counts, [0, 0, 0, 0, 0]);
        assert_eq!(stats.attended_count, 0);
        assert_eq!(stats.goal_count, 0);
    }
}
