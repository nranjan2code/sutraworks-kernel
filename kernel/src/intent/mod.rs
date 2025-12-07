//! Intent Module - Semantic Intent Execution
//!
//! Intents are semantic concepts that trigger actions.
//! Inputs from any source (steno, English, sensors) map to ConceptIDs.
//!
//! # Architecture
//! ```
//! Any Input → ConceptID → Intent → Executor (Broadcast 1:N)
//! ```
//! Direct semantic execution. No character-level processing.

use crate::kernel::capability::{
    Capability, CapabilityType, Permissions, 
    mint_root
};

pub mod handlers;
pub mod queue;
pub mod manifest;
pub mod linker;
pub mod security;
pub mod temporal;
pub mod hierarchy;
pub mod feedback;
pub mod scheduling;

pub use handlers::{
    HandlerRegistry, HandlerResult, HandlerFn, HandlerEntry, 
    BroadcastResult, BroadcastStats, MAX_INHIBITS,
    BroadcastScope, ConflictResolution, HandlerResponse, MAX_RESPONSES,
};
pub use queue::{IntentQueue, QueuedIntent, Priority};
pub use security::{IntentSecurity, SecurityViolation, PrivilegeLevel};
pub use temporal::{
    TemporalDynamics, TemporalStats, TEMPORAL_DYNAMICS,
    decay_tick, process_intent_activation, summate, is_primed,
    DEFAULT_DECAY_RATE, DEFAULT_SUMMATION_WINDOW_MS,
};
pub use hierarchy::{
    HierarchicalProcessor, HierarchicalStats, LayerBuffer,
    AttentionFocus, GoalState, HIERARCHICAL_PROCESSOR,
    input_intent, propagate_all, attend, set_goal, get_actions,
    NUM_LAYERS, DEFAULT_ATTENTION_CAPACITY,
};
pub use feedback::{
    FeedbackProcessor, FeedbackResult, FeedbackStats,
    PredictionBuffer, Prediction, ExpectationMatcher, Expectation,
    SurpriseDetector, SurpriseEvent, SurpriseType,
    FEEDBACK_PROCESSOR, predict, expect, process_input, 
    check_omissions, surprise_level, priority_boost,
};
pub use scheduling::{
    NeuralScheduler, NeuralSchedulerStats, IntentRequest,
    IntentCategory, CoreAffinity, UrgencyAccumulator,
    DegradationPolicy, LoadLevel, NEURAL_SCHEDULER,
    submit_intent, next_intent, next_intent_for_core,
    update_load, scheduler_tick,
};

// ═══════════════════════════════════════════════════════════════════════════════
// CORE TYPES
// ═══════════════════════════════════════════════════════════════════════════════

/// A 64-bit semantic concept identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConceptID(pub u64);

impl ConceptID {
    /// Create from raw value
    pub const fn new(id: u64) -> Self {
        ConceptID(id)
    }
    
    /// Unknown/unrecognized concept
    pub const UNKNOWN: ConceptID = ConceptID(0xFFFF_FFFF_FFFF_FFFF);

    /// Create from string (FNV-1a hash)
    pub const fn from_str(s: &str) -> Self {
        let mut hash: u64 = 0xcbf29ce484222325;
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            hash ^= bytes[i] as u64;
            hash = hash.wrapping_mul(0x100000001b3);
            i += 1;
        }
        ConceptID(hash)
    }
}

impl From<&str> for ConceptID {
    fn from(s: &str) -> Self {
        Self::from_str(s)
    }
}

/// Intent data payload
#[derive(Clone, Debug)]
pub enum IntentData {
    None,
    Number(i64),
    Raw(u64),
    String(alloc::string::String),
}

// ═══════════════════════════════════════════════════════════════════════════════
// HIERARCHICAL PROCESSING (Cortex-Inspired)
// ═══════════════════════════════════════════════════════════════════════════════

/// Hierarchical processing level (cortex-inspired)
/// 
/// Intents flow through layers like signals through cortex:
/// - **Raw**: Sensory input (pixels, audio samples, raw keystrokes)
/// - **Feature**: Detected features (edges, phonemes, key patterns)
/// - **Object**: Recognized objects (face, word, stroke sequence)
/// - **Semantic**: Meaning (person=friend, word=command, intent recognized)
/// - **Action**: Motor output (speak, move, display, execute)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
pub enum IntentLevel {
    Raw = 0,       // Sensory input (pixels, samples)
    Feature = 1,   // Detected features (edges, phonemes)
    Object = 2,    // Recognized objects (face, word)
    #[default]
    Semantic = 3,  // Meaning (person=friend, word=command) - Most intents start here
    Action = 4,    // Motor output (speak, move, display)
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTENT (Neural-Enhanced)
// ═══════════════════════════════════════════════════════════════════════════════

/// A semantic intent - the result of processing input
/// 
/// # Neural Dynamics
/// 
/// Intents now carry biological-inspired metadata:
/// - **activation**: Current signal strength (decays over time)
/// - **timestamp**: When this intent was created (for temporal processing)
/// - **level**: Hierarchical layer in cortex-like processing
/// - **source**: What triggered this (for feedback/prediction)
#[derive(Clone, Debug)]
pub struct Intent {
    /// The semantic concept this intent represents
    pub concept_id: ConceptID,
    /// Pattern matching confidence (0.0 - 1.0)
    pub confidence: f32,
    /// Optional payload data
    pub data: IntentData,
    /// Human-readable name
    pub name: &'static str,
    
    // ─────────────────────────────────────────────────────────────────────────
    // Neural fields (new)
    // ─────────────────────────────────────────────────────────────────────────
    
    /// Current activation level (like neural firing rate)
    /// Starts at 1.0 and decays over time. High activation = high priority.
    pub activation: f32,
    /// Creation timestamp (milliseconds since boot)
    /// Used for temporal dynamics (decay, refractory periods)
    pub timestamp: u64,
    /// Hierarchical processing level (Raw → Feature → Object → Semantic → Action)
    pub level: IntentLevel,
    /// What triggered this intent (for efference copy / prediction)
    /// None for externally-generated intents (sensory input)
    pub source: Option<ConceptID>,
}

impl Intent {
    /// Create a new intent with default neural fields
    pub const fn new(concept_id: ConceptID) -> Self {
        Self {
            concept_id,
            confidence: 1.0,
            data: IntentData::None,
            name: "UNKNOWN",
            // Neural defaults
            activation: 1.0,
            timestamp: 0,
            level: IntentLevel::Semantic,
            source: None,
        }
    }
    
    /// Create with confidence
    pub const fn with_confidence(concept_id: ConceptID, confidence: f32) -> Self {
        Self {
            concept_id,
            confidence,
            data: IntentData::None,
            name: "UNKNOWN",
            activation: confidence,  // Match activation to confidence
            timestamp: 0,
            level: IntentLevel::Semantic,
            source: None,
        }
    }
    
    /// Create with full neural context
    pub const fn with_neural(
        concept_id: ConceptID,
        confidence: f32,
        activation: f32,
        timestamp: u64,
        level: IntentLevel,
        source: Option<ConceptID>,
    ) -> Self {
        Self {
            concept_id,
            confidence,
            data: IntentData::None,
            name: "UNKNOWN",
            activation,
            timestamp,
            level,
            source,
        }
    }
    
    /// Check if this is an unknown/unrecognized intent
    pub fn is_unknown(&self) -> bool {
        self.concept_id == ConceptID::UNKNOWN || self.confidence < 0.5
    }
    
    /// Check if intent is still "alive" (activation above threshold)
    pub fn is_active(&self) -> bool {
        self.activation >= 0.1
    }
    
    /// Apply decay to activation (call periodically)
    pub fn decay(&mut self, factor: f32) {
        self.activation *= factor;
    }
    
    /// Boost activation (e.g., from attention or repeated input)
    pub fn boost(&mut self, amount: f32) {
        self.activation = (self.activation + amount).min(1.0);
    }
    
    /// Escalate to next hierarchical level
    pub fn escalate(&mut self) {
        self.level = match self.level {
            IntentLevel::Raw => IntentLevel::Feature,
            IntentLevel::Feature => IntentLevel::Object,
            IntentLevel::Object => IntentLevel::Semantic,
            IntentLevel::Semantic => IntentLevel::Action,
            IntentLevel::Action => IntentLevel::Action,  // Already at top
        };
    }
}

impl Default for Intent {
    fn default() -> Self {
        Self::new(ConceptID::UNKNOWN)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTENT EXECUTOR
// ═══════════════════════════════════════════════════════════════════════════════

/// Executes intents with capability checking
pub struct IntentExecutor {
    // Capabilities
    display_cap: Option<Capability>,
    memory_cap: Option<Capability>,
    system_cap: Option<Capability>,
    compute_cap: Option<Capability>,
    // User-defined handlers
    handlers: HandlerRegistry,
    // Intent queue for deferred execution
    queue: IntentQueue,
    // Security layer
    security: security::IntentSecurity,
}

impl IntentExecutor {
    pub const fn new() -> Self {
        Self {
            display_cap: None,
            memory_cap: None,
            system_cap: None,
            compute_cap: None,
            handlers: HandlerRegistry::new(),
            queue: IntentQueue::new(),
            security: security::IntentSecurity::new(),
        }
    }
    
    /// Initialize capabilities and default handlers
    pub fn init(&mut self) {
        unsafe {
            self.display_cap = mint_root(CapabilityType::Display, 0, 0, Permissions::ALL);
            self.memory_cap = mint_root(CapabilityType::Memory, 0, 0x1_0000_0000, Permissions::ALL);
            self.system_cap = mint_root(CapabilityType::System, 0, 0, Permissions::ALL);
            self.compute_cap = mint_root(CapabilityType::Compute, 0, 0, Permissions::ALL);
        }

        // Register default system handlers
        use crate::steno::dictionary::concepts;
        use crate::intent::handlers::system;

        // System
        self.handlers.register(concepts::HELP, system::handle_help, "help");
        self.handlers.register(concepts::STATUS, system::handle_status, "status");
        self.handlers.register_with_options(concepts::REBOOT, system::handle_reboot, "reboot", 100, Some(CapabilityType::System));
        self.handlers.register(concepts::CLEAR, system::handle_clear, "clear");
        self.handlers.register(concepts::UNDO, system::handle_undo, "undo");
        
        // Display
        self.handlers.register(concepts::SHOW, system::handle_show, "show");
        self.handlers.register(concepts::HIDE, system::handle_hide, "hide");

        // Memory
        self.handlers.register_with_options(concepts::STORE, system::handle_store, "store", 100, Some(CapabilityType::Memory));
        self.handlers.register(concepts::RECALL, system::handle_recall, "recall");
        self.handlers.register_with_options(concepts::DELETE, system::handle_delete, "delete", 100, Some(CapabilityType::Memory));

        // Navigation
        self.handlers.register(concepts::NEXT, system::handle_next, "next");
        self.handlers.register(concepts::PREVIOUS, system::handle_previous, "previous");
        self.handlers.register(concepts::BACK, system::handle_back, "back");

        // Confirmation
        self.handlers.register(concepts::YES, system::handle_yes, "yes");
        self.handlers.register(concepts::NO, system::handle_no, "no");
        self.handlers.register(concepts::CONFIRM, system::handle_confirm, "confirm");
        self.handlers.register(concepts::CANCEL, system::handle_cancel, "cancel");

        // Files
        self.handlers.register(concepts::LIST_FILES, system::handle_list_files, "ls");
        self.handlers.register(concepts::READ_FILE, system::handle_read_file, "cat");
    }
    
    /// Check if we have a capability
    pub fn has_capability(&self, cap_type: CapabilityType) -> bool {
        match cap_type {
            CapabilityType::Display => self.display_cap.as_ref().map(|c| c.is_valid()).unwrap_or(false),
            CapabilityType::Memory => self.memory_cap.as_ref().map(|c| c.is_valid()).unwrap_or(false),
            CapabilityType::System => self.system_cap.as_ref().map(|c| c.is_valid()).unwrap_or(false),
            CapabilityType::Compute => self.compute_cap.as_ref().map(|c| c.is_valid()).unwrap_or(false),
            _ => false,
        }
    }
    
    /// Execute an intent
    pub fn execute(&mut self, intent: &Intent) {
        // ═══════════════════════════════════════════════════════════════════════════
        // SECURITY CHECKS (HDC-BASED)
        // ═══════════════════════════════════════════════════════════════════════════
        
        // Get current process/source ID for rate limiting
        let source_id = {
            let scheduler = crate::kernel::scheduler::SCHEDULER.lock();
            scheduler.current_pid().unwrap_or(0)
        };
        
        // Get current timestamp
        let timestamp = crate::drivers::timer::uptime_ms();
        
        // Determine privilege level (kernel privilege assumed for now)
        // In real implementation, this would check the current execution level
        let privilege = security::PrivilegeLevel::Kernel;
        
        // CHECK SECURITY
        if let Err(violation) = self.security.check_intent(
            intent.concept_id,
            source_id,
            privilege,
            timestamp,
        ) {
            crate::kprintln!("[SECURITY] Intent rejected: {:?}", violation);
            return;
        }
        
        // ═══════════════════════════════════════════════════════════════════════════
        // CAPABILITY-BASED DISPATCH
        // ═══════════════════════════════════════════════════════════════════════════
        
        // Pre-compute capability checks to avoid borrow issues
        let caps = [
            (CapabilityType::Display, self.has_capability(CapabilityType::Display)),
            (CapabilityType::Memory, self.has_capability(CapabilityType::Memory)),
            (CapabilityType::System, self.has_capability(CapabilityType::System)),
            (CapabilityType::Compute, self.has_capability(CapabilityType::Compute)),
        ];
        
        let has_cap = |cap: CapabilityType| {
            caps.iter().find(|(c, _)| *c == cap).map(|(_, v)| *v).unwrap_or(false)
        };
        
        // First, try user-defined handlers (which now include system handlers)
        if self.handlers.dispatch(intent, has_cap) {
            return; // Handled
        }
        
        // Fall back to Skill Registry
        use crate::steno::dictionary::concepts;
        let id = intent.concept_id;

        // Check if a Skill is registered for this concept
        if let Some(skill) = crate::apps::registry::REGISTRY.lock().find_by_tag(id) {
            let ctx = crate::apps::registry::Context::default();
            match skill.execute(intent.name, &ctx) {
                Ok(result) => {
                    crate::kprintln!("[SKILL] {}: {}", skill.name(), result);
                }
                Err(e) => {
                    crate::kprintln!("[SKILL] {} failed: {:?}", skill.name(), e);
                }
            }
        } else {
            crate::kprintln!("[INTENT] Unknown concept: {:?}", id);
        }
    }
    
    /// Register a user-defined handler for a concept
    pub fn register_handler(
        &mut self,
        concept_id: ConceptID,
        handler: HandlerFn,
        name: &'static str,
    ) -> bool {
        self.handlers.register(concept_id, handler, name)
    }
    
    /// Register a handler with full options
    pub fn register_handler_with_options(
        &mut self,
        concept_id: ConceptID,
        handler: HandlerFn,
        name: &'static str,
        priority: u8,
        required_cap: Option<CapabilityType>,
    ) -> bool {
        self.handlers.register_with_options(concept_id, handler, name, priority, required_cap)
    }

    /// Register a wildcard handler (receives all intents)
    pub fn register_wildcard(
        &mut self,
        handler: HandlerFn,
        name: &'static str,
        priority: u8,
    ) -> bool {
        self.handlers.register_wildcard(handler, name, priority)
    }
    
    /// Unregister a handler
    pub fn unregister_handler(&mut self, name: &'static str) -> bool {
        self.handlers.unregister(name)
    }
    
    /// Get handler count
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }
    
    // ═══════════════════════════════════════════════════════════════════════════
    // INTENT QUEUE
    // ═══════════════════════════════════════════════════════════════════════════
    
    /// Queue an intent for later execution
    pub fn queue_intent(&mut self, intent: Intent, timestamp: u64) -> bool {
        self.queue.push(intent, timestamp)
    }
    
    /// Queue an intent with priority
    pub fn queue_intent_with_priority(
        &mut self,
        intent: Intent,
        priority: Priority,
        timestamp: u64,
    ) -> bool {
        self.queue.push_with_priority(intent, priority, timestamp, 0)
    }
    
    /// Process next queued intent
    pub fn process_queue(&mut self) -> bool {
        if let Some(queued) = self.queue.pop() {
            self.execute(&queued.intent);
            true
        } else {
            false
        }
    }
    
    /// Get queue length
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }
    
    /// Remove expired intents from queue
    pub fn cleanup_queue(&mut self, now: u64) -> usize {
        self.queue.remove_expired(now)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL EXECUTOR
// ═══════════════════════════════════════════════════════════════════════════════

use crate::kernel::sync::SpinLock;

static EXECUTOR: SpinLock<IntentExecutor> = SpinLock::new(IntentExecutor::new());

/// Initialize the intent system
pub fn init() {
    let mut executor = EXECUTOR.lock();
    executor.init();
    crate::kprintln!("[INTENT] Executor initialized");
}

/// Execute an intent
pub fn execute(intent: &Intent) {
    let mut executor = EXECUTOR.lock();
    executor.execute(intent);
}

/// Check if we have a capability
pub fn has_capability(cap_type: CapabilityType) -> bool {
    let executor = EXECUTOR.lock();
    executor.has_capability(cap_type)
}

/// Register a user-defined intent handler
pub fn register_handler(
    concept_id: ConceptID,
    handler: HandlerFn,
    name: &'static str,
) -> bool {
    let mut executor = EXECUTOR.lock();
    executor.register_handler(concept_id, handler, name)
}

/// Register a wildcard handler
pub fn register_wildcard(
    handler: HandlerFn,
    name: &'static str,
    priority: u8,
) -> bool {
    let mut executor = EXECUTOR.lock();
    executor.register_wildcard(handler, name, priority)
}

/// Unregister a handler by name
pub fn unregister_handler(name: &'static str) -> bool {
    let mut executor = EXECUTOR.lock();
    executor.unregister_handler(name)
}

/// Queue an intent for deferred execution
pub fn queue(intent: Intent, timestamp: u64) -> bool {
    let mut executor = EXECUTOR.lock();
    executor.queue_intent(intent, timestamp)
}

/// Queue with priority
pub fn queue_with_priority(intent: Intent, priority: Priority, timestamp: u64) -> bool {
    let mut executor = EXECUTOR.lock();
    executor.queue_intent_with_priority(intent, priority, timestamp)
}

/// Process next queued intent
pub fn process_queue() -> bool {
    let mut executor = EXECUTOR.lock();
    executor.process_queue()
}

/// Get the number of queued intents
pub fn queue_len() -> usize {
    let executor = EXECUTOR.lock();
    executor.queue_len()
}

impl Default for IntentExecutor {
    fn default() -> Self {
        Self::new()
    }
}
