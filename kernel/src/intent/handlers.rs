//! User-Defined Intent Handlers
//!
//! Allows registration of custom handlers for ConceptIDs.
//! Maintains the pure stroke→intent flow while enabling extensibility.
//!
//! # Architecture
//! ```
//! Intent → Handler Registry → Custom Handler OR Default Handler
//! ```
//!
//! # Thread Safety
//! Handler registration uses a SpinLock. Handlers run with interrupts enabled.

use crate::intent::{ConceptID, Intent, IntentData};
use crate::kernel::capability::CapabilityType;

// ═══════════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum registered handlers
pub const MAX_HANDLERS: usize = 128;

// ═══════════════════════════════════════════════════════════════════════════════
// HANDLER TRAIT
// ═══════════════════════════════════════════════════════════════════════════════

/// Result of handling an intent
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandlerResult {
    /// Intent was handled successfully, continue to next handler (Broadcast)
    Handled,
    /// Intent was handled, STOP propagation (Consume)
    StopPropagation,
    /// Intent was not handled (pass to next handler)
    NotHandled,
    /// Intent handling failed with error code
    Error(u32),
}

/// Function pointer type for intent handlers
/// 
/// Takes the intent and returns a result
pub type HandlerFn = fn(&Intent) -> HandlerResult;

// ═══════════════════════════════════════════════════════════════════════════════
// HANDLER ENTRY
// ═══════════════════════════════════════════════════════════════════════════════

/// A registered handler
#[derive(Clone, Copy)]
pub struct HandlerEntry {
    /// Concept ID this handler responds to (or 0 for wildcard)
    pub concept_id: ConceptID,
    /// Required capability (if any)
    pub required_cap: Option<CapabilityType>,
    /// Handler function
    pub handler: HandlerFn,
    /// Priority (higher runs first)
    pub priority: u8,
    /// Debug name
    pub name: &'static str,
}

impl HandlerEntry {
    /// Placeholder for empty slots
    pub const EMPTY: Self = Self {
        concept_id: ConceptID(0),
        required_cap: None,
        handler: empty_handler,
        priority: 0,
        name: "",
    };
}

/// Empty handler placeholder
fn empty_handler(_: &Intent) -> HandlerResult {
    HandlerResult::NotHandled
}

// ═══════════════════════════════════════════════════════════════════════════════
// HANDLER REGISTRY
// ═══════════════════════════════════════════════════════════════════════════════

/// Registry of intent handlers
pub struct HandlerRegistry {
    /// Registered handlers
    handlers: [HandlerEntry; MAX_HANDLERS],
    /// Number of valid handlers
    count: usize,
    /// Whether handlers are sorted by priority
    sorted: bool,
}

impl HandlerRegistry {
    /// Create an empty registry
    pub const fn new() -> Self {
        Self {
            handlers: [HandlerEntry::EMPTY; MAX_HANDLERS],
            count: 0,
            sorted: true,
        }
    }
    
    /// Register a handler for a specific concept
    pub fn register(
        &mut self,
        concept_id: ConceptID,
        handler: HandlerFn,
        name: &'static str,
    ) -> bool {
        self.register_with_options(concept_id, handler, name, 100, None)
    }
    
    /// Register a handler with full options
    pub fn register_with_options(
        &mut self,
        concept_id: ConceptID,
        handler: HandlerFn,
        name: &'static str,
        priority: u8,
        required_cap: Option<CapabilityType>,
    ) -> bool {
        if self.count >= MAX_HANDLERS {
            return false;
        }
        
        self.handlers[self.count] = HandlerEntry {
            concept_id,
            required_cap,
            handler,
            priority,
            name,
        };
        self.count += 1;
        self.sorted = false;
        true
    }
    
    /// Register a wildcard handler (receives all intents)
    pub fn register_wildcard(
        &mut self,
        handler: HandlerFn,
        name: &'static str,
        priority: u8,
    ) -> bool {
        self.register_with_options(
            ConceptID(0), // 0 = wildcard
            handler,
            name,
            priority,
            None,
        )
    }
    
    /// Unregister a handler by name
    pub fn unregister(&mut self, name: &'static str) -> bool {
        for i in 0..self.count {
            if self.handlers[i].name == name {
                // Shift remaining handlers
                for j in i..self.count - 1 {
                    self.handlers[j] = self.handlers[j + 1];
                }
                self.handlers[self.count - 1] = HandlerEntry::EMPTY;
                self.count -= 1;
                return true;
            }
        }
        false
    }
    
    /// Sort handlers by priority (descending)
    fn sort_by_priority(&mut self) {
        if self.sorted || self.count < 2 {
            return;
        }
        
        // Simple insertion sort (small array, rarely sorted)
        for i in 1..self.count {
            let key = self.handlers[i];
            let mut j = i;
            while j > 0 && self.handlers[j - 1].priority < key.priority {
                self.handlers[j] = self.handlers[j - 1];
                j -= 1;
            }
            self.handlers[j] = key;
        }
        
        self.sorted = true;
    }
    
    /// Dispatch an intent to registered handlers
    /// 
    /// Returns true if any handler processed the intent
    pub fn dispatch(&mut self, intent: &Intent, has_cap: impl Fn(CapabilityType) -> bool) -> bool {
        self.sort_by_priority();
        
        let target_id = intent.concept_id;
        let mut any_handled = false;
        
        for i in 0..self.count {
            let entry = &self.handlers[i];
            
            // Check concept match (0 = wildcard)
            if entry.concept_id.0 != 0 && entry.concept_id != target_id {
                continue;
            }
            
            // Check capability
            if let Some(cap) = entry.required_cap {
                if !has_cap(cap) {
                    continue;
                }
            }
            
            // Call handler
            match (entry.handler)(intent) {
                HandlerResult::Handled => {
                    any_handled = true;
                    // Continue to next handler (Broadcast)
                },
                HandlerResult::StopPropagation => {
                    return true;
                },
                HandlerResult::NotHandled => continue,
                HandlerResult::Error(code) => {
                    crate::kprintln!("[HANDLER] {} error: {}", entry.name, code);
                    // Log error but continue broadcast
                }
            }
        }
        
        any_handled
    }
    
    /// Get the number of registered handlers
    pub fn len(&self) -> usize {
        self.count
    }
    
    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    
    /// List all registered handlers (for debugging)
    pub fn list(&self) -> &[HandlerEntry] {
        &self.handlers[..self.count]
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    static mut TEST_COUNTER: u32 = 0;
    
    fn test_handler_a(_: &Intent) -> HandlerResult {
        unsafe { TEST_COUNTER += 1; }
        HandlerResult::Handled
    }
    
    fn test_handler_b(_: &Intent) -> HandlerResult {
        unsafe { TEST_COUNTER += 10; }
        HandlerResult::StopPropagation
    }
    
    fn pass_through_handler(_: &Intent) -> HandlerResult {
        HandlerResult::NotHandled
    }
    
    #[test]
    fn test_register_handler() {
        let mut registry = HandlerRegistry::new();
        
        assert!(registry.register(
            ConceptID(0x0001),
            test_handler_a,
            "test_a"
        ));
        
        assert_eq!(registry.len(), 1);
    }
    
    #[test]
    fn test_dispatch_specific() {
        let mut registry = HandlerRegistry::new();
        unsafe { TEST_COUNTER = 0; }
        
        registry.register(ConceptID(0x0001), test_handler_a, "test_a");
        
        let intent = Intent::new(ConceptID(0x0001));
        let handled = registry.dispatch(&intent, |_| true);
        
        assert!(handled);
        unsafe { assert_eq!(TEST_COUNTER, 1); }
    }
    
    #[test]
    fn test_dispatch_priority() {
        let mut registry = HandlerRegistry::new();
        unsafe { TEST_COUNTER = 0; }
        
        // Lower priority
        registry.register_with_options(
            ConceptID(0x0001),
            test_handler_a,
            "low_priority",
            50,
            None,
        );
        
        // Higher priority - should run first and STOP propagation
        registry.register_with_options(
            ConceptID(0x0001),
            test_handler_b,
            "high_priority",
            200,
            None,
        );
        
        let intent = Intent::new(ConceptID(0x0001));
        registry.dispatch(&intent, |_| true);
        
        // Only high priority should run (adds 10) because it returns StopPropagation
        unsafe { assert_eq!(TEST_COUNTER, 10); }
    }
    
    #[test]
    fn test_dispatch_broadcast() {
        let mut registry = HandlerRegistry::new();
        unsafe { TEST_COUNTER = 0; }
        
        // Handler A (adds 1)
        registry.register(ConceptID(0x0001), test_handler_a, "handler_a");
        
        // Handler A again (adds 1) - simulating multiple listeners
        registry.register(ConceptID(0x0001), test_handler_a, "handler_a_2");
        
        let intent = Intent::new(ConceptID(0x0001));
        registry.dispatch(&intent, |_| true);
        
        // Both should run
        unsafe { assert_eq!(TEST_COUNTER, 2); }
    }
    
    #[test]
    fn test_unregister() {
        let mut registry = HandlerRegistry::new();
        
        registry.register(ConceptID(0x0001), test_handler_a, "test_a");
        registry.register(ConceptID(0x0002), test_handler_b, "test_b");
        
        assert_eq!(registry.len(), 2);
        
        assert!(registry.unregister("test_a"));
        assert_eq!(registry.len(), 1);
        
        assert!(!registry.unregister("nonexistent"));
    }
}
