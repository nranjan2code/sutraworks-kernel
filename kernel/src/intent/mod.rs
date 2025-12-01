//! Intent Module - Stroke-Native Semantic Intent
//!
//! This is a stenographic kernel. Intents come from strokes, not words.
//! The old word-tokenization system has been removed.
//!
//! # Architecture
//! ```
//! Stroke (23-bit) → Dictionary → Intent → Executor
//! ```
//! Direct. No parsing. No tokenization.

use crate::kernel::capability::{
    Capability, CapabilityType, Permissions, 
    mint_root, validate
};

// ═══════════════════════════════════════════════════════════════════════════════
// CORE TYPES
// ═══════════════════════════════════════════════════════════════════════════════

/// A 64-bit semantic concept identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ConceptID(pub u64);

impl ConceptID {
    /// Create from raw value
    pub const fn new(id: u64) -> Self {
        ConceptID(id)
    }
    
    /// Unknown/unrecognized concept
    pub const UNKNOWN: ConceptID = ConceptID(0xFFFF_FFFF_FFFF_FFFF);
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
}

impl Intent {
    /// Create a new intent
    pub const fn new(concept_id: ConceptID) -> Self {
        Self {
            concept_id,
            confidence: 1.0,
            data: IntentData::None,
        }
    }
    
    /// Create with confidence
    pub const fn with_confidence(concept_id: ConceptID, confidence: f32) -> Self {
        Self {
            concept_id,
            confidence,
            data: IntentData::None,
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
}

impl IntentExecutor {
    pub const fn new() -> Self {
        Self {
            display_cap: None,
            memory_cap: None,
            system_cap: None,
            compute_cap: None,
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
