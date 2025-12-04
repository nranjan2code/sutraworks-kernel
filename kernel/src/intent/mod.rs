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

pub use handlers::{HandlerRegistry, HandlerResult, HandlerFn, HandlerEntry};
pub use queue::{IntentQueue, QueuedIntent, Priority};

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
}

/// A semantic intent - the result of processing a stroke
#[derive(Clone, Debug)]
pub struct Intent {
    pub concept_id: ConceptID,
    pub confidence: f32,
    pub data: IntentData,
    pub name: &'static str,
}

impl Intent {
    /// Create a new intent
    pub const fn new(concept_id: ConceptID) -> Self {
        Self {
            concept_id,
            confidence: 1.0,
            data: IntentData::None,
            name: "UNKNOWN",
        }
    }
    
    /// Create with confidence
    pub const fn with_confidence(concept_id: ConceptID, confidence: f32) -> Self {
        Self {
            concept_id,
            confidence,
            data: IntentData::None,
            name: "UNKNOWN",
        }
    }
    
    /// Check if this is an unknown/unrecognized intent
    pub fn is_unknown(&self) -> bool {
        self.concept_id == ConceptID::UNKNOWN || self.confidence < 0.5
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
        }
    }
    
    /// Initialize with root capabilities
    pub fn init(&mut self) {
        unsafe {
            self.display_cap = mint_root(CapabilityType::Display, 0, 0, Permissions::ALL);
            self.memory_cap = mint_root(CapabilityType::Memory, 0, 0x1_0000_0000, Permissions::ALL);
            self.system_cap = mint_root(CapabilityType::System, 0, 0, Permissions::ALL);
            self.compute_cap = mint_root(CapabilityType::Compute, 0, 0, Permissions::ALL);
        }
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
        // First, try user-defined handlers
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
        
        if self.handlers.dispatch(intent, has_cap) {
            return; // Handled by user handler
        }
        
        // Fall back to built-in handlers
        use crate::steno::dictionary::concepts;
        
        // Route by concept ID
        let id = intent.concept_id;
        
        // System commands
        if id == concepts::HELP {
            self.handle_help();
        } else if id == concepts::STATUS {
            self.handle_status();
        } else if id == concepts::REBOOT {
            self.handle_reboot();
        } else if id == concepts::CLEAR {
            self.handle_clear();
        } else if id == concepts::UNDO {
            self.handle_undo();
        }
        // Display actions
        else if id == concepts::SHOW {
            self.handle_show();
        } else if id == concepts::HIDE {
            self.handle_hide();
        }
        // Memory actions
        else if id == concepts::STORE {
            self.handle_store();
        } else if id == concepts::RECALL {
            self.handle_recall();
        } else if id == concepts::DELETE {
            self.handle_delete();
        }
        // Navigation
        else if id == concepts::NEXT {
            self.handle_next();
        } else if id == concepts::PREVIOUS {
            self.handle_previous();
        } else if id == concepts::BACK {
            self.handle_back();
        }
        // Confirmation
        else if id == concepts::YES {
            self.handle_yes();
        } else if id == concepts::NO {
            self.handle_no();
        } else if id == concepts::CONFIRM {
            self.handle_confirm();
        } else if id == concepts::CANCEL {
            self.handle_cancel();
        }
        // Unknown
        else {
            crate::kprintln!("[INTENT] Unknown concept: {:?}", id);
        }
    }
    
    // ═══════════════════════════════════════════════════════════════════════════
    // HANDLERS
    // ═══════════════════════════════════════════════════════════════════════════
    
    fn handle_help(&self) {
        crate::kprintln!("╔═══════════════════════════════════════╗");
        crate::kprintln!("║     INTENT KERNEL - STENO HELP        ║");
        crate::kprintln!("╠═══════════════════════════════════════╣");
        crate::kprintln!("║ Strokes are semantic. Not characters. ║");
        crate::kprintln!("║                                       ║");
        crate::kprintln!("║ System:  STAT, PH-FPL (help), *       ║");
        crate::kprintln!("║ Display: SHRO (show), HEU (hide)      ║");
        crate::kprintln!("║ Memory:  STOR, RAOE/KAUL (recall)     ║");
        crate::kprintln!("║ Nav:     TPHEFBGT, PREUF, PWABG       ║");
        crate::kprintln!("║ Confirm: KWRE (yes), TPHO (no)        ║");
        crate::kprintln!("╚═══════════════════════════════════════╝");
    }
    
    fn handle_status(&self) {
        let stats = crate::steno::stats();
        crate::kprintln!("[STATUS]");
        crate::kprintln!("  Strokes processed: {}", stats.strokes_processed);
        crate::kprintln!("  Intents matched:   {}", stats.intents_matched);
        crate::kprintln!("  Corrections:       {}", stats.corrections);
        crate::kprintln!("  Unrecognized:      {}", stats.unrecognized);
    }
    
    fn handle_reboot(&self) {
        if !self.has_capability(CapabilityType::System) {
            crate::kprintln!("[DENIED] System capability required");
            return;
        }
        crate::kprintln!("[SYSTEM] Rebooting...");
        // In real implementation: trigger watchdog or reset
    }
    
    fn handle_clear(&self) {
        crate::kprintln!("\x1B[2J\x1B[H"); // ANSI clear screen
    }
    
    fn handle_undo(&self) {
        crate::kprintln!("[UNDO] Last action undone");
    }
    
    fn handle_show(&self) {
        crate::kprintln!("[DISPLAY] Show");
    }
    
    fn handle_hide(&self) {
        crate::kprintln!("[DISPLAY] Hide");
    }
    
    fn handle_store(&self) {
        if !self.has_capability(CapabilityType::Memory) {
            crate::kprintln!("[DENIED] Memory capability required");
            return;
        }
        crate::kprintln!("[MEMORY] Store");
    }
    
    fn handle_recall(&self) {
        crate::kprintln!("[MEMORY] Recall");
    }
    
    fn handle_delete(&self) {
        if !self.has_capability(CapabilityType::Memory) {
            crate::kprintln!("[DENIED] Memory capability required");
            return;
        }
        crate::kprintln!("[MEMORY] Delete");
    }
    
    fn handle_next(&self) {
        crate::kprintln!("[NAV] Next");
    }
    
    fn handle_previous(&self) {
        crate::kprintln!("[NAV] Previous");
    }
    
    fn handle_back(&self) {
        crate::kprintln!("[NAV] Back");
    }
    
    fn handle_yes(&self) {
        crate::kprintln!("[CONFIRM] Yes");
    }
    
    fn handle_no(&self) {
        crate::kprintln!("[CONFIRM] No");
    }
    
    fn handle_confirm(&self) {
        crate::kprintln!("[CONFIRM] Confirmed");
    }
    
    fn handle_cancel(&self) {
        crate::kprintln!("[CONFIRM] Cancelled");
    }
    
    // ═══════════════════════════════════════════════════════════════════════════
    // USER-DEFINED HANDLERS
    // ═══════════════════════════════════════════════════════════════════════════
    
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

use crate::arch::SpinLock;

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
