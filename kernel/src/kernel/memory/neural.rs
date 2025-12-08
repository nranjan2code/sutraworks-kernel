//! Neural Memory Allocator (ConceptID Edition)
//!
//! Semantic memory for the Intent Kernel.
//! Maps unique ConceptIDs directly to memory blocks.
//! 
//! # Neural Features
//! 
//! - **Activation**: Each concept has an activation level that decays over time
//! - **Associations**: Concepts can have links to related concepts (like synapses)
//! - **Spreading Activation**: Activating a concept spreads to its associates

use core::ptr::NonNull;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use crate::intent::ConceptID;
use crate::kernel::sync::SpinLock;

/// Global Neural Allocator instance
pub static NEURAL_ALLOCATOR: SpinLock<NeuralAllocator> = SpinLock::new(NeuralAllocator::new());

/// Maximum number of associations per concept (like synaptic connections)
pub const MAX_ASSOCIATIONS: usize = 8;

/// A Semantic Block of memory (Neural-Enhanced)
/// 
/// # Neural Dynamics
/// 
/// Each semantic block now tracks:
/// - **activation**: Current activation level (0.0 - 1.0)
/// - **last_accessed**: Timestamp for decay calculation
/// - **associations**: Links to related concepts (forming semantic network)
/// - **link_strengths**: Weights on associations (0.0 = unused slot)
#[repr(C)]
pub struct SemanticBlock {
    pub concept_id: ConceptID,
    pub access_count: u64,
    pub size: usize,
    
    // ─────────────────────────────────────────────────────────────────────────
    // Neural fields (new)
    // ─────────────────────────────────────────────────────────────────────────
    
    /// Current activation level (like neural firing rate)
    /// Decays over time, boosted by access
    pub activation: f32,
    /// Last access timestamp (ms since boot)
    pub last_accessed: u64,
    /// Associated concepts (like synaptic connections)
    pub associations: [ConceptID; MAX_ASSOCIATIONS],
    /// Weights on each association (0.0 = unused, 1.0 = strong link)
    pub link_strengths: [f32; MAX_ASSOCIATIONS],
    // Data follows immediately after this struct
}

impl SemanticBlock {
    /// Create a new semantic block with default neural values
    pub fn new(concept_id: ConceptID, size: usize) -> Self {
        Self {
            concept_id,
            access_count: 0,
            size,
            activation: 1.0,  // Start fully activated
            last_accessed: 0,
            associations: [ConceptID(0); MAX_ASSOCIATIONS],
            link_strengths: [0.0; MAX_ASSOCIATIONS],
        }
    }
    
    /// Check if concept is currently active (above threshold)
    pub fn is_active(&self) -> bool {
        self.activation >= 0.1
    }
}

/// Content-Addressable Pointer
#[derive(Clone, Copy, Debug)]
pub struct IntentPtr {
    pub id: ConceptID,
    pub ptr: NonNull<u8>,
    pub size: usize,
}

/// Page Header for Semantic Memory
#[repr(C)]
struct SemanticPage {
    next: Option<NonNull<SemanticPage>>,
    used: usize,
    // Data follows. We use the rest of the 4KB page.
}

/// Neural Allocator (Dynamic Page-Based)
/// 
/// # Neural Features
/// 
/// - **Spreading Activation**: Activating a concept spreads to associates
/// - **Decay**: All activations decay over time (call `decay_tick`)
/// - **Hebbian Learning**: "Fire together, wire together" via `associate()`
pub struct NeuralAllocator {
    head_page: Option<NonNull<SemanticPage>>,
    current_page: Option<NonNull<SemanticPage>>,
    total_items: usize,
    index: BTreeMap<ConceptID, IntentPtr>,
    /// Block metadata (separate from data to allow iteration)
    blocks: BTreeMap<ConceptID, SemanticBlock>,
}

// SAFETY: NeuralAllocator is protected by SpinLock.
unsafe impl Send for NeuralAllocator {}

impl NeuralAllocator {
    pub const fn new() -> Self {
        NeuralAllocator {
            head_page: None,
            current_page: None,
            total_items: 0,
            index: BTreeMap::new(),
            blocks: BTreeMap::new(),
        }
    }
    
    /// Clear/reset the allocator to free memory
    /// 
    /// This clears the BTreeMap indexes to release heap memory.
    /// Note: The pages themselves are NOT freed (no deallocation mechanism yet).
    /// The pages will be reused for new allocations.
    pub fn clear(&mut self) {
        self.index.clear();
        self.blocks.clear();
        self.total_items = 0;
        // Reset page pointers to reuse from beginning
        self.current_page = self.head_page;
        if let Some(mut page) = self.current_page {
            unsafe {
                page.as_mut().used = 0;
            }
        }
    }

    /// Allocate memory with a concept ID tag
    pub unsafe fn alloc(&mut self, size: usize, concept_id: ConceptID) -> Option<IntentPtr> {
        let block_size = core::mem::size_of::<SemanticBlock>();
        let total_needed = block_size + size;
        let align_padding = (16 - (total_needed % 16)) % 16;
        let alloc_size = total_needed + align_padding;
        
        // Ensure we have a current page with enough space
        if self.current_page.is_none() || !self.has_space(alloc_size) {
            self.grow_heap()?;
        }
        
        let page_ptr = self.current_page.expect("grow_heap ensures current_page");
        let page = page_ptr.as_ptr();
        
        // Calculate placement
        let offset = (*page).used;
        let start_ptr = (page as *mut u8).add(core::mem::size_of::<SemanticPage>() + offset);
        
        // Initialize Block (write the header in memory)
        let block_ptr = start_ptr as *mut SemanticBlock;
        core::ptr::write(block_ptr, SemanticBlock::new(concept_id, size));
        
        // Update Page State
        (*page).used += alloc_size;
        self.total_items += 1;
        
        let ptr = IntentPtr {
            id: concept_id,
            ptr: NonNull::new_unchecked(start_ptr.add(block_size)),
            size,
        };

        // Update Index (O(log N))
        self.index.insert(concept_id, ptr);
        
        // Also store block metadata
        self.blocks.insert(concept_id, SemanticBlock::new(concept_id, size));
        
        Some(ptr)
    }
    
    /// Check if current page has space
    unsafe fn has_space(&self, size: usize) -> bool {
        if let Some(page_ptr) = self.current_page {
            let page = page_ptr.as_ref();
            let available = 4096 - core::mem::size_of::<SemanticPage>() - page.used;
            available >= size
        } else {
            false
        }
    }
    
    /// Allocate a new page and link it
    unsafe fn grow_heap(&mut self) -> Option<()> {
        // Allocate 1 page (4KB)
        #[cfg(not(feature = "test_mocks"))]
        let ptr = crate::kernel::memory::alloc_pages(1)?;

        #[cfg(feature = "test_mocks")]
        let ptr = {
            use alloc::alloc::{alloc, Layout};
            let layout = Layout::from_size_align(4096, 4096).ok()?;
            let raw = alloc(layout);
            if raw.is_null() { return None; }
            NonNull::new(raw)?
        };
        let page = ptr.as_ptr() as *mut SemanticPage;
        
        // Initialize Header
        (*page).next = None;
        (*page).used = 0;
        
        // Link
        if let Some(curr) = self.current_page {
            (*curr.as_ptr()).next = Some(ptr.cast());
        } else {
            self.head_page = Some(ptr.cast());
        }
        
        self.current_page = Some(ptr.cast());
        Some(())
    }

    /// Retrieve memory by concept ID (Exact Match)
    /// Uses BTreeMap Index for O(log N) lookup.
    pub unsafe fn retrieve(&self, concept_id: ConceptID) -> Option<IntentPtr> {
        self.index.get(&concept_id).copied()
    }
    
    /// Get count of allocated blocks
    pub fn count(&self) -> usize {
        self.total_items
    }

    /// Get all allocated nodes (for visualization)
    pub fn get_all_nodes(&self) -> Vec<IntentPtr> {
        self.index.values().copied().collect()
    }
    
    // ═══════════════════════════════════════════════════════════════════════════
    // NEURAL DYNAMICS
    // ═══════════════════════════════════════════════════════════════════════════
    
    /// Activate a concept and spread activation to associates
    /// 
    /// # Arguments
    /// * `concept_id` - The concept to activate
    /// * `strength` - Activation strength (0.0 - 1.0)
    /// * `timestamp` - Current time in ms
    /// 
    /// # Returns
    /// Vector of associate ConceptIDs that were also activated (for chaining)
    pub fn activate(&mut self, concept_id: ConceptID, strength: f32, timestamp: u64) -> Vec<ConceptID> {
        let mut activated = Vec::new();
        
        // Activate the primary concept
        if let Some(block) = self.blocks.get_mut(&concept_id) {
            block.activation = (block.activation + strength).min(1.0);
            block.last_accessed = timestamp;
            block.access_count += 1;
            
            // Spread activation to associates (spreading activation)
            for i in 0..MAX_ASSOCIATIONS {
                let link_strength = block.link_strengths[i];
                if link_strength > 0.0 {
                    let associate_id = block.associations[i];
                    let spread_strength = strength * link_strength * 0.5; // Decay by 50%
                    
                    if spread_strength >= 0.05 { // Threshold for spreading
                        activated.push(associate_id);
                    }
                }
            }
        }
        
        // Activate associates (secondary spreading)
        for associate_id in activated.iter() {
            if let Some(assoc_block) = self.blocks.get_mut(associate_id) {
                let spread_strength = strength * 0.3; // Weaker secondary activation
                assoc_block.activation = (assoc_block.activation + spread_strength).min(1.0);
                assoc_block.last_accessed = timestamp;
            }
        }
        
        activated
    }
    
    /// Create or strengthen an association between two concepts (Hebbian learning)
    /// 
    /// "Neurons that fire together, wire together"
    /// 
    /// # Arguments
    /// * `from` - Source concept
    /// * `to` - Target concept
    /// * `strength` - Initial/additional strength (0.0 - 1.0)
    pub fn associate(&mut self, from: ConceptID, to: ConceptID, strength: f32) -> bool {
        if let Some(block) = self.blocks.get_mut(&from) {
            // Find existing or empty slot
            let mut empty_slot = None;
            
            for i in 0..MAX_ASSOCIATIONS {
                if block.associations[i] == to {
                    // Strengthen existing link (saturating add)
                    block.link_strengths[i] = (block.link_strengths[i] + strength).min(1.0);
                    return true;
                }
                if block.link_strengths[i] == 0.0 && empty_slot.is_none() {
                    empty_slot = Some(i);
                }
            }
            
            // Use empty slot for new association
            if let Some(slot) = empty_slot {
                block.associations[slot] = to;
                block.link_strengths[slot] = strength;
                return true;
            }
        }
        false
    }
    
    /// Decay all activations (call periodically, e.g., every 100ms)
    /// 
    /// # Arguments
    /// * `timestamp` - Current time in ms
    /// * `decay_rate` - Per-ms decay factor (e.g., 0.001 = 0.1% per ms)
    pub fn decay_tick(&mut self, timestamp: u64, decay_rate: f32) {
        for block in self.blocks.values_mut() {
            let elapsed = timestamp.saturating_sub(block.last_accessed);
            let decay = 1.0 - (decay_rate * elapsed as f32).min(1.0);
            block.activation *= decay;
            
            // Clamp to zero if very small
            if block.activation < 0.001 {
                block.activation = 0.0;
            }
        }
    }
    
    /// Get the most active concepts (above threshold)
    /// 
    /// # Arguments
    /// * `threshold` - Minimum activation level
    /// * `limit` - Maximum number of results
    pub fn get_active_concepts(&self, threshold: f32, limit: usize) -> Vec<(ConceptID, f32)> {
        let mut active: Vec<_> = self.blocks
            .iter()
            .filter(|(_, block)| block.activation >= threshold)
            .map(|(id, block)| (*id, block.activation))
            .collect();
        
        // Sort by activation (descending)
        active.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));
        
        active.truncate(limit);
        active
    }
    
    /// Get activation level of a concept
    pub fn get_activation(&self, concept_id: ConceptID) -> Option<f32> {
        self.blocks.get(&concept_id).map(|b| b.activation)
    }
    
    /// Get associates of a concept
    pub fn get_associates(&self, concept_id: ConceptID) -> Vec<(ConceptID, f32)> {
        self.blocks.get(&concept_id)
            .map(|block| {
                block.associations.iter()
                    .zip(block.link_strengths.iter())
                    .filter(|(_, &strength)| strength > 0.0)
                    .map(|(id, strength)| (*id, *strength))
                    .collect()
            })
            .unwrap_or_default()
    }
    
    // ═══════════════════════════════════════════════════════════════════════════
    // TEMPORAL DYNAMICS (Phase 3)
    // ═══════════════════════════════════════════════════════════════════════════
    
    /// Temporal summation: repeated weak activations accumulate
    /// 
    /// Unlike `activate()`, this uses an integration window. If the same concept
    /// is activated multiple times within `window_ms`, the activations sum.
    /// This models temporal summation in biological neurons.
    /// 
    /// # Arguments
    /// * `concept_id` - Concept to activate
    /// * `strength` - Individual activation strength (can be weak, e.g., 0.1)
    /// * `timestamp` - Current time in ms
    /// * `window_ms` - Integration window (typically 50-200ms)
    /// 
    /// # Returns
    /// True if activation crossed threshold (0.5), indicating "firing"
    pub fn temporal_summate(
        &mut self, 
        concept_id: ConceptID, 
        strength: f32, 
        timestamp: u64,
        window_ms: u64,
    ) -> bool {
        if let Some(block) = self.blocks.get_mut(&concept_id) {
            let elapsed = timestamp.saturating_sub(block.last_accessed);
            
            if elapsed <= window_ms {
                // Within integration window - sum activations
                block.activation = (block.activation + strength).min(1.0);
            } else {
                // Outside window - reset and start fresh
                block.activation = strength;
            }
            
            block.last_accessed = timestamp;
            block.access_count += 1;
            
            // Return true if crossed firing threshold
            block.activation >= 0.5
        } else {
            false
        }
    }
    
    /// Record a sequence for predictive priming
    /// 
    /// When concept A is followed by concept B, we strengthen A→B association.
    /// This enables predictive priming: activating A will pre-activate B.
    /// 
    /// # Arguments
    /// * `previous` - Previous concept
    /// * `current` - Current concept (follows previous)
    /// * `timestamp` - Current time
    /// * `max_gap_ms` - Max time between concepts to count as sequence
    pub fn record_sequence(
        &mut self,
        previous: ConceptID,
        current: ConceptID,
        timestamp: u64,
        max_gap_ms: u64,
    ) {
        // Check if previous concept was recently active
        if let Some(prev_block) = self.blocks.get(&previous) {
            let elapsed = timestamp.saturating_sub(prev_block.last_accessed);
            
            if elapsed <= max_gap_ms {
                // Within temporal window - strengthen prediction link
                let strength = 0.1; // Incremental learning rate
                self.associate(previous, current, strength);
            }
        }
    }
    
    /// Get predicted next concepts based on current activation
    /// 
    /// Uses spreading activation to find likely next concepts.
    /// Returns concepts that are primed (have high activation from association).
    /// 
    /// # Arguments
    /// * `threshold` - Minimum priming level to consider
    /// * `limit` - Max predictions to return
    pub fn get_predictions(&self, threshold: f32, limit: usize) -> Vec<(ConceptID, f32)> {
        // Find concepts that are active purely from spreading (not direct access)
        let mut predictions: Vec<_> = self.blocks
            .iter()
            .filter(|(_, block)| {
                // Active but not recently directly accessed
                block.activation >= threshold && block.access_count == 0
            })
            .map(|(id, block)| (*id, block.activation))
            .collect();
        
        predictions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(core::cmp::Ordering::Equal));
        predictions.truncate(limit);
        predictions
    }
    
    /// Prime a concept for faster future activation
    /// 
    /// Pre-activates a concept without fully activating it.
    /// Primed concepts require less additional activation to fire.
    /// 
    /// # Arguments
    /// * `concept_id` - Concept to prime
    /// * `prime_level` - Priming strength (typically 0.2-0.4)
    /// * `timestamp` - Current time
    pub fn prime(&mut self, concept_id: ConceptID, prime_level: f32, timestamp: u64) {
        if let Some(block) = self.blocks.get_mut(&concept_id) {
            // Prime doesn't reset access time (it's background activation)
            block.activation = (block.activation + prime_level).min(0.49); // Below firing threshold
        } else {
            // Create a new primed block if it doesn't exist
            let mut new_block = SemanticBlock::new(concept_id, 0);
            new_block.activation = prime_level.min(0.49);
            new_block.last_accessed = timestamp;
            self.blocks.insert(concept_id, new_block);
        }
    }
    
    /// Apply predictive priming based on current activation
    /// 
    /// Automatically primes concepts that are associated with currently active concepts.
    /// Call this after activating a concept to enable prediction.
    /// 
    /// # Arguments
    /// * `source` - Source concept whose associates will be primed
    /// * `timestamp` - Current time
    /// 
    /// # Returns
    /// Number of concepts that were primed
    pub fn apply_predictive_priming(&mut self, source: ConceptID, timestamp: u64) -> usize {
        // First, collect the associates (can't borrow mutably twice)
        let associates: Vec<(ConceptID, f32)> = self.get_associates(source);
        
        let mut primed = 0;
        for (assoc_id, link_strength) in associates {
            // Prime level is proportional to link strength
            let prime_level = link_strength * 0.3;
            if prime_level >= 0.05 {
                self.prime(assoc_id, prime_level, timestamp);
                primed += 1;
            }
        }
        
        primed
    }
    
    /// Check if a concept is primed (ready for fast activation)
    pub fn is_primed(&self, concept_id: ConceptID) -> bool {
        self.blocks.get(&concept_id)
            .map(|b| b.activation >= 0.1 && b.activation < 0.5)
            .unwrap_or(false)
    }
    
    /// Get priming level of a concept
    pub fn get_priming_level(&self, concept_id: ConceptID) -> f32 {
        self.blocks.get(&concept_id)
            .map(|b| if b.activation < 0.5 { b.activation } else { 0.0 })
            .unwrap_or(0.0)
    }
}


impl Default for NeuralAllocator { fn default() -> Self { Self::new() } }

