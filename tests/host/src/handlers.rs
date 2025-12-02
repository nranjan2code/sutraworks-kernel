//! Host-Based Tests for Intent Handler Registry
//!
//! Tests the user-defined handler registration and dispatch.

use std::sync::atomic::{AtomicU32, Ordering};

// ═══════════════════════════════════════════════════════════════════════════════
// TYPES
// ═══════════════════════════════════════════════════════════════════════════════

pub const MAX_HANDLERS: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandlerResult {
    Handled,
    NotHandled,
    Error(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConceptID(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityType {
    Display,
    Memory,
    System,
    Compute,
}

pub type HandlerFn = fn(ConceptID) -> HandlerResult;

// ═══════════════════════════════════════════════════════════════════════════════
// HANDLER ENTRY
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy)]
pub struct HandlerEntry {
    pub concept_id: ConceptID,
    pub required_cap: Option<CapabilityType>,
    pub handler: HandlerFn,
    pub priority: u8,
    pub name: &'static str,
}

fn empty_handler(_: ConceptID) -> HandlerResult {
    HandlerResult::NotHandled
}

impl HandlerEntry {
    pub const EMPTY: Self = Self {
        concept_id: ConceptID(0),
        required_cap: None,
        handler: empty_handler,
        priority: 0,
        name: "",
    };
}

// ═══════════════════════════════════════════════════════════════════════════════
// HANDLER REGISTRY
// ═══════════════════════════════════════════════════════════════════════════════

pub struct HandlerRegistry {
    handlers: Vec<HandlerEntry>,
    sorted: bool,
}

impl HandlerRegistry {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            sorted: true,
        }
    }
    
    pub fn register(
        &mut self,
        concept_id: ConceptID,
        handler: HandlerFn,
        name: &'static str,
    ) -> bool {
        self.register_with_options(concept_id, handler, name, 100, None)
    }
    
    pub fn register_with_options(
        &mut self,
        concept_id: ConceptID,
        handler: HandlerFn,
        name: &'static str,
        priority: u8,
        required_cap: Option<CapabilityType>,
    ) -> bool {
        if self.handlers.len() >= MAX_HANDLERS {
            return false;
        }
        
        self.handlers.push(HandlerEntry {
            concept_id,
            required_cap,
            handler,
            priority,
            name,
        });
        self.sorted = false;
        true
    }
    
    pub fn register_wildcard(
        &mut self,
        handler: HandlerFn,
        name: &'static str,
        priority: u8,
    ) -> bool {
        self.register_with_options(ConceptID(0), handler, name, priority, None)
    }
    
    pub fn unregister(&mut self, name: &'static str) -> bool {
        if let Some(idx) = self.handlers.iter().position(|h| h.name == name) {
            self.handlers.remove(idx);
            true
        } else {
            false
        }
    }
    
    fn sort_by_priority(&mut self) {
        if self.sorted || self.handlers.len() < 2 {
            return;
        }
        
        self.handlers.sort_by(|a, b| b.priority.cmp(&a.priority));
        self.sorted = true;
    }
    
    pub fn dispatch(&mut self, concept_id: ConceptID, has_cap: impl Fn(CapabilityType) -> bool) -> bool {
        self.sort_by_priority();
        
        for entry in &self.handlers {
            // Check concept match (0 = wildcard)
            if entry.concept_id.0 != 0 && entry.concept_id != concept_id {
                continue;
            }
            
            // Check capability
            if let Some(cap) = entry.required_cap {
                if !has_cap(cap) {
                    continue;
                }
            }
            
            match (entry.handler)(concept_id) {
                HandlerResult::Handled => return true,
                HandlerResult::NotHandled => continue,
                HandlerResult::Error(_) => return false,
            }
        }
        
        false
    }
    
    pub fn len(&self) -> usize {
        self.handlers.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);
    
    fn handler_increment(_: ConceptID) -> HandlerResult {
        TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        HandlerResult::Handled
    }
    
    fn handler_add_10(_: ConceptID) -> HandlerResult {
        TEST_COUNTER.fetch_add(10, Ordering::SeqCst);
        HandlerResult::Handled
    }
    
    fn handler_pass_through(_: ConceptID) -> HandlerResult {
        HandlerResult::NotHandled
    }
    
    fn handler_error(_: ConceptID) -> HandlerResult {
        HandlerResult::Error(42)
    }
    
    fn reset_counter() {
        TEST_COUNTER.store(0, Ordering::SeqCst);
    }
    
    fn get_counter() -> u32 {
        TEST_COUNTER.load(Ordering::SeqCst)
    }
    
    #[test]
    fn test_registry_empty() {
        let registry = HandlerRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }
    
    #[test]
    fn test_registry_register() {
        let mut registry = HandlerRegistry::new();
        
        assert!(registry.register(ConceptID(0x0001), handler_increment, "test"));
        assert_eq!(registry.len(), 1);
    }
    
    #[test]
    fn test_registry_unregister() {
        let mut registry = HandlerRegistry::new();
        
        registry.register(ConceptID(0x0001), handler_increment, "test_a");
        registry.register(ConceptID(0x0002), handler_add_10, "test_b");
        
        assert_eq!(registry.len(), 2);
        
        assert!(registry.unregister("test_a"));
        assert_eq!(registry.len(), 1);
        
        assert!(!registry.unregister("nonexistent"));
    }
    
    #[test]
    fn test_dispatch_matching() {
        let mut registry = HandlerRegistry::new();
        reset_counter();
        
        registry.register(ConceptID(0x0001), handler_increment, "test");
        
        let handled = registry.dispatch(ConceptID(0x0001), |_| true);
        
        assert!(handled);
        assert_eq!(get_counter(), 1);
    }
    
    #[test]
    fn test_dispatch_no_match() {
        let mut registry = HandlerRegistry::new();
        reset_counter();
        
        registry.register(ConceptID(0x0001), handler_increment, "test");
        
        // Dispatch for different concept
        let handled = registry.dispatch(ConceptID(0x0002), |_| true);
        
        assert!(!handled);
        assert_eq!(get_counter(), 0);
    }
    
    #[test]
    fn test_dispatch_wildcard() {
        let mut registry = HandlerRegistry::new();
        reset_counter();
        
        registry.register_wildcard(handler_increment, "wildcard", 100);
        
        // Should match any concept
        assert!(registry.dispatch(ConceptID(0x0001), |_| true));
        assert!(registry.dispatch(ConceptID(0x0002), |_| true));
        assert!(registry.dispatch(ConceptID(0xFFFF), |_| true));
        
        assert_eq!(get_counter(), 3);
    }
    
    #[test]
    fn test_dispatch_priority() {
        let mut registry = HandlerRegistry::new();
        reset_counter();
        
        // Low priority adds 1
        registry.register_with_options(
            ConceptID(0x0001),
            handler_increment,
            "low",
            50,
            None,
        );
        
        // High priority adds 10 - should run first
        registry.register_with_options(
            ConceptID(0x0001),
            handler_add_10,
            "high",
            200,
            None,
        );
        
        registry.dispatch(ConceptID(0x0001), |_| true);
        
        // Only high priority runs (it handles, stopping dispatch)
        assert_eq!(get_counter(), 10);
    }
    
    #[test]
    fn test_dispatch_pass_through() {
        let mut registry = HandlerRegistry::new();
        reset_counter();
        
        // First handler passes through
        registry.register_with_options(
            ConceptID(0x0001),
            handler_pass_through,
            "pass",
            200,
            None,
        );
        
        // Second handler handles
        registry.register_with_options(
            ConceptID(0x0001),
            handler_increment,
            "handle",
            100,
            None,
        );
        
        registry.dispatch(ConceptID(0x0001), |_| true);
        
        // Pass through didn't increment, but second handler did
        assert_eq!(get_counter(), 1);
    }
    
    #[test]
    fn test_dispatch_capability_required() {
        let mut registry = HandlerRegistry::new();
        reset_counter();
        
        registry.register_with_options(
            ConceptID(0x0001),
            handler_increment,
            "needs_system",
            100,
            Some(CapabilityType::System),
        );
        
        // Without capability - should not dispatch
        let handled = registry.dispatch(ConceptID(0x0001), |_| false);
        assert!(!handled);
        assert_eq!(get_counter(), 0);
        
        // With capability - should dispatch
        let handled = registry.dispatch(ConceptID(0x0001), |_| true);
        assert!(handled);
        assert_eq!(get_counter(), 1);
    }
    
    #[test]
    fn test_dispatch_capability_selective() {
        let mut registry = HandlerRegistry::new();
        reset_counter();
        
        registry.register_with_options(
            ConceptID(0x0001),
            handler_increment,
            "needs_system",
            100,
            Some(CapabilityType::System),
        );
        
        // Only have Display, not System
        let has_cap = |cap: CapabilityType| cap == CapabilityType::Display;
        
        let handled = registry.dispatch(ConceptID(0x0001), has_cap);
        assert!(!handled);
        assert_eq!(get_counter(), 0);
    }
    
    #[test]
    fn test_dispatch_error() {
        let mut registry = HandlerRegistry::new();
        
        registry.register(ConceptID(0x0001), handler_error, "error_handler");
        
        let handled = registry.dispatch(ConceptID(0x0001), |_| true);
        
        // Error stops dispatch and returns false
        assert!(!handled);
    }
    
    #[test]
    fn test_registry_multiple_handlers_same_concept() {
        let mut registry = HandlerRegistry::new();
        reset_counter();
        
        // Three handlers for same concept, different priorities
        registry.register_with_options(ConceptID(0x0001), handler_pass_through, "a", 100, None);
        registry.register_with_options(ConceptID(0x0001), handler_pass_through, "b", 200, None);
        registry.register_with_options(ConceptID(0x0001), handler_increment, "c", 50, None);
        
        // All pass through except 'c' which handles
        registry.dispatch(ConceptID(0x0001), |_| true);
        
        assert_eq!(get_counter(), 1);
    }
}
