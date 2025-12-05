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

use crate::intent::{ConceptID, Intent};
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
/// 
/// # Neural Semantics
/// 
/// - `Handled`: Intent processed, continue broadcast (like excitatory neuron)
/// - `StopPropagation`: Intent consumed, stop broadcast (strong inhibition)
/// - `Inhibit`: Suppress specific handlers (lateral inhibition - GABA-like)
/// - `Modulate`: Adjust activation for subsequent handlers (neuromodulation)
#[derive(Clone, Debug, PartialEq)]
pub enum HandlerResult {
    /// Intent was handled successfully, continue to next handler (Broadcast)
    Handled,
    /// Intent was handled, STOP propagation (Consume)
    StopPropagation,
    /// Intent was not handled (pass to next handler)
    NotHandled,
    /// Intent handling failed with error code
    Error(u32),
    /// Suppress specific handlers from firing (lateral inhibition)
    /// The handler was processed, but these targets should not fire
    Inhibit(heapless::Vec<ConceptID, 4>),
    /// Modulate (boost or reduce) activation for subsequent handlers
    /// Values > 1.0 amplify, < 1.0 suppress (like dopamine/serotonin)
    Modulate(f32),
}

/// Function pointer type for intent handlers
/// 
/// Takes the intent and returns a result
pub type HandlerFn = fn(&Intent) -> HandlerResult;

// ═══════════════════════════════════════════════════════════════════════════════
// HANDLER ENTRY (Neural-Enhanced)
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum number of concepts a handler can inhibit
pub const MAX_INHIBITS: usize = 4;

/// A registered handler (neural-enhanced)
/// 
/// # Neural Features
/// 
/// - **Inhibits**: List of ConceptIDs this handler suppresses when it fires
///   (like GABA-ergic lateral inhibition)
/// - **Refractory Period**: Minimum time between firings (like neural refractory period)
/// - **Last Fired**: Timestamp of last activation (for refractory tracking)
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
    
    // ─────────────────────────────────────────────────────────────────────────
    // Neural fields (new)
    // ─────────────────────────────────────────────────────────────────────────
    
    /// Concepts this handler inhibits when it fires (lateral inhibition)
    /// Use [ConceptID(0); MAX_INHIBITS] for empty (0 = unused slot)
    pub inhibits: [ConceptID; MAX_INHIBITS],
    /// Number of valid entries in inhibits array
    pub inhibits_count: u8,
    /// Minimum milliseconds between firings (refractory period)
    /// 0 = no refractory period (can fire every time)
    pub refractory_ms: u16,
    /// Timestamp of last firing (for refractory check)
    pub last_fired: u64,
}

impl HandlerEntry {
    /// Placeholder for empty slots
    pub const EMPTY: Self = Self {
        concept_id: ConceptID(0),
        required_cap: None,
        handler: empty_handler,
        priority: 0,
        name: "",
        inhibits: [ConceptID(0); MAX_INHIBITS],
        inhibits_count: 0,
        refractory_ms: 0,
        last_fired: 0,
    };
    
    /// Check if this handler is in refractory period
    pub fn is_refractory(&self, now: u64) -> bool {
        if self.refractory_ms == 0 {
            return false;
        }
        now.saturating_sub(self.last_fired) < self.refractory_ms as u64
    }
    
    /// Get the list of concepts this handler inhibits
    pub fn get_inhibits(&self) -> &[ConceptID] {
        &self.inhibits[..self.inhibits_count as usize]
    }
}

/// Empty handler placeholder
fn empty_handler(_: &Intent) -> HandlerResult {
    HandlerResult::NotHandled
}

// ═══════════════════════════════════════════════════════════════════════════════
// BROADCAST SCOPE
// ═══════════════════════════════════════════════════════════════════════════════

/// Broadcast scope determines which handlers receive the intent
/// 
/// # Biological Analogy
/// 
/// - `Local`: Like a local circuit (interneurons within a cortical column)
/// - `Subsystem`: Like pathways (visual cortex, motor cortex)
/// - `Global`: Like ascending/descending tracts (sensory → motor)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BroadcastScope {
    /// Only handlers with matching ConceptID
    Local,
    /// Handlers in the same subsystem (first byte of ConceptID matches)
    Subsystem,
    /// All handlers (wildcard + matching + subsystem)
    #[default]
    Global,
}

impl BroadcastScope {
    /// Check if a handler's concept matches within this scope
    pub fn matches(&self, handler_id: ConceptID, target_id: ConceptID) -> bool {
        match self {
            BroadcastScope::Local => {
                // Local scope: EXACT match only, no wildcards
                handler_id == target_id
            },
            BroadcastScope::Subsystem => {
                // Wildcards always match in Subsystem scope
                if handler_id.0 == 0 {
                    return true;
                }
                // Match if first 2 bytes (subsystem prefix) are equal
                (handler_id.0 >> 48) == (target_id.0 >> 48)
            },
            BroadcastScope::Global => {
                // Wildcards match, or exact match
                handler_id == target_id || handler_id.0 == 0
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONFLICT RESOLUTION
// ═══════════════════════════════════════════════════════════════════════════════

/// How to resolve conflicts when multiple handlers claim exclusivity
/// 
/// # Biological Analogy
/// 
/// - `WinnerTakeAll`: Like lateral inhibition in retina/cortex
/// - `HighestPriority`: Like urgency signals (pain > touch)
/// - `HighestActivation`: Like competitive learning
/// - `Consensus`: Like population coding (majority vote)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ConflictResolution {
    /// First handler that claims exclusivity wins (fastest response)
    FirstClaims,
    /// Handler with highest priority wins
    #[default]
    HighestPriority,
    /// Handler with highest activation/modulation wins
    HighestActivation,
    /// All handlers vote, majority wins (requires aggregation)
    Consensus,
}

// ═══════════════════════════════════════════════════════════════════════════════
// HANDLER RESPONSE (for aggregation)
// ═══════════════════════════════════════════════════════════════════════════════

/// Individual handler's response (for aggregation)
#[derive(Clone, Debug)]
pub struct HandlerResponse {
    /// Which handler produced this response
    pub handler_name: &'static str,
    /// The handler's priority
    pub priority: u8,
    /// The result returned
    pub result: HandlerResult,
    /// Execution order (for tie-breaking)
    pub sequence: u16,
}

/// Maximum number of responses to collect
pub const MAX_RESPONSES: usize = 32;

// ═══════════════════════════════════════════════════════════════════════════════
// BROADCAST RESULT (Neural-Enhanced with Full Aggregation)
// ═══════════════════════════════════════════════════════════════════════════════

/// Result of broadcasting an intent to all handlers
/// 
/// Captures the full neural dynamics of the broadcast:
/// - All handler responses (for aggregation and analysis)
/// - Which concepts were inhibited
/// - What modulation was applied
/// - Winner resolution (if conflicts occurred)
#[derive(Clone, Debug)]
pub struct BroadcastResult {
    /// Count of handlers that processed the intent
    pub handled_count: usize,
    /// Concepts that were inhibited during broadcast
    pub inhibited: heapless::Vec<ConceptID, 16>,
    /// Accumulated modulation factor (product of all Modulate results)
    pub modulation: f32,
    /// Whether broadcast was stopped early (StopPropagation)
    pub stopped: bool,
    /// All handler responses (for aggregation)
    pub responses: heapless::Vec<HandlerResponse, MAX_RESPONSES>,
    /// The winning handler (if conflict resolution was needed)
    pub winner: Option<&'static str>,
    /// Scope used for this broadcast
    pub scope: BroadcastScope,
}

impl Default for BroadcastResult {
    fn default() -> Self {
        Self::new()
    }
}

impl BroadcastResult {
    pub fn new() -> Self {
        Self {
            handled_count: 0,
            inhibited: heapless::Vec::new(),
            modulation: 1.0,
            stopped: false,
            responses: heapless::Vec::new(),
            winner: None,
            scope: BroadcastScope::Global,
        }
    }
    
    /// Create with specific scope
    pub fn with_scope(scope: BroadcastScope) -> Self {
        Self {
            scope,
            ..Self::new()
        }
    }
    
    /// Check if any handler processed the intent
    pub fn was_handled(&self) -> bool {
        self.handled_count > 0
    }
    
    /// Get count of successful handlers
    pub fn success_count(&self) -> usize {
        self.responses.iter()
            .filter(|r| matches!(r.result, HandlerResult::Handled | HandlerResult::StopPropagation))
            .count()
    }
    
    /// Get count of errors
    pub fn error_count(&self) -> usize {
        self.responses.iter()
            .filter(|r| matches!(r.result, HandlerResult::Error(_)))
            .count()
    }
    
    /// Resolve conflicts using winner-take-all strategy
    /// Returns the winning handler name
    pub fn resolve_winner(&mut self, strategy: ConflictResolution) -> Option<&'static str> {
        if self.responses.is_empty() {
            return None;
        }
        
        let winner = match strategy {
            ConflictResolution::FirstClaims => {
                // First StopPropagation or Handled wins
                self.responses.iter()
                    .find(|r| matches!(r.result, HandlerResult::StopPropagation | HandlerResult::Handled))
                    .map(|r| r.handler_name)
            },
            ConflictResolution::HighestPriority => {
                // Highest priority handler that handled wins
                self.responses.iter()
                    .filter(|r| matches!(r.result, HandlerResult::Handled | HandlerResult::StopPropagation))
                    .max_by_key(|r| r.priority)
                    .map(|r| r.handler_name)
            },
            ConflictResolution::HighestActivation => {
                // Handler that contributed highest modulation wins
                self.responses.iter()
                    .filter_map(|r| {
                        if let HandlerResult::Modulate(factor) = r.result {
                            Some((r, factor))
                        } else if matches!(r.result, HandlerResult::Handled | HandlerResult::StopPropagation) {
                            Some((r, 1.0))
                        } else {
                            None
                        }
                    })
                    .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(core::cmp::Ordering::Equal))
                    .map(|(r, _)| r.handler_name)
            },
            ConflictResolution::Consensus => {
                // Count votes (Handled = +1, StopPropagation = +2)
                // Handler with most "votes" wins (in a single-handler context, just pick highest priority)
                self.responses.iter()
                    .filter(|r| matches!(r.result, HandlerResult::Handled | HandlerResult::StopPropagation))
                    .max_by_key(|r| {
                        if matches!(r.result, HandlerResult::StopPropagation) { 2 } else { 1 }
                    })
                    .map(|r| r.handler_name)
            }
        };
        
        self.winner = winner;
        winner
    }
    
    /// Get aggregate statistics
    pub fn stats(&self) -> BroadcastStats {
        BroadcastStats {
            total_handlers: self.responses.len(),
            handled: self.success_count(),
            errors: self.error_count(),
            inhibited: self.inhibited.len(),
            modulation: self.modulation,
            stopped: self.stopped,
        }
    }
}

/// Statistics summary of a broadcast
#[derive(Clone, Copy, Debug, Default)]
pub struct BroadcastStats {
    pub total_handlers: usize,
    pub handled: usize,
    pub errors: usize,
    pub inhibited: usize,
    pub modulation: f32,
    pub stopped: bool,
}

// ═══════════════════════════════════════════════════════════════════════════════
// HANDLER REGISTRY (Neural-Enhanced)
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
    
    /// Register a handler for a specific concept (simple API)
    pub fn register(
        &mut self,
        concept_id: ConceptID,
        handler: HandlerFn,
        name: &'static str,
    ) -> bool {
        self.register_neural(concept_id, handler, name, 100, None, &[], 0)
    }
    
    /// Register a handler with priority and capability (compatibility API)
    pub fn register_with_options(
        &mut self,
        concept_id: ConceptID,
        handler: HandlerFn,
        name: &'static str,
        priority: u8,
        required_cap: Option<CapabilityType>,
    ) -> bool {
        self.register_neural(concept_id, handler, name, priority, required_cap, &[], 0)
    }
    
    /// Register a handler with full neural options
    /// 
    /// # Arguments
    /// - `inhibits`: Concepts this handler suppresses when it fires
    /// - `refractory_ms`: Minimum time between firings (0 = no limit)
    pub fn register_neural(
        &mut self,
        concept_id: ConceptID,
        handler: HandlerFn,
        name: &'static str,
        priority: u8,
        required_cap: Option<CapabilityType>,
        inhibits: &[ConceptID],
        refractory_ms: u16,
    ) -> bool {
        if self.count >= MAX_HANDLERS {
            return false;
        }
        
        // Build inhibits array
        let mut inhibits_arr = [ConceptID(0); MAX_INHIBITS];
        let inhibits_count = inhibits.len().min(MAX_INHIBITS);
        for (i, id) in inhibits.iter().take(MAX_INHIBITS).enumerate() {
            inhibits_arr[i] = *id;
        }
        
        self.handlers[self.count] = HandlerEntry {
            concept_id,
            required_cap,
            handler,
            priority,
            name,
            inhibits: inhibits_arr,
            inhibits_count: inhibits_count as u8,
            refractory_ms,
            last_fired: 0,
        };
        self.count += 1;
        self.sorted = false;
        true
    }
    
    /// Register a handler with inhibition (convenience API)
    pub fn register_with_inhibition(
        &mut self,
        concept_id: ConceptID,
        handler: HandlerFn,
        name: &'static str,
        inhibits: &[ConceptID],
    ) -> bool {
        self.register_neural(concept_id, handler, name, 100, None, inhibits, 0)
    }
    
    /// Register a handler with refractory period (convenience API)
    pub fn register_with_refractory(
        &mut self,
        concept_id: ConceptID,
        handler: HandlerFn,
        name: &'static str,
        refractory_ms: u16,
    ) -> bool {
        self.register_neural(concept_id, handler, name, 100, None, &[], refractory_ms)
    }
    
    /// Register a wildcard handler (receives all intents)
    pub fn register_wildcard(
        &mut self,
        handler: HandlerFn,
        name: &'static str,
        priority: u8,
    ) -> bool {
        self.register_neural(ConceptID(0), handler, name, priority, None, &[], 0)
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
    
    /// Dispatch an intent to registered handlers (backward-compatible API)
    /// 
    /// Returns true if any handler processed the intent.
    /// Note: Use `broadcast()` for full neural dynamics.
    pub fn dispatch(&mut self, intent: &Intent, has_cap: impl Fn(CapabilityType) -> bool) -> bool {
        self.broadcast(intent, has_cap, 0).was_handled()
    }
    
    /// Broadcast an intent to all handlers with full neural dynamics
    /// 
    /// # Neural Features
    /// 
    /// - **Refractory Periods**: Handlers won't fire if recently fired
    /// - **Lateral Inhibition**: Handlers can suppress other handlers
    /// - **Modulation**: Handlers can boost/reduce subsequent activations
    /// - **Response Aggregation**: Collects all handler responses
    /// 
    /// # Arguments
    /// 
    /// - `intent`: The intent to broadcast
    /// - `has_cap`: Capability checker function
    /// - `timestamp`: Current time in ms (for refractory period checking)
    pub fn broadcast(
        &mut self,
        intent: &Intent, 
        has_cap: impl Fn(CapabilityType) -> bool,
        timestamp: u64,
    ) -> BroadcastResult {
        self.broadcast_scoped(intent, has_cap, timestamp, BroadcastScope::Global)
    }
    
    /// Broadcast with explicit scope control
    /// 
    /// # Scope Levels
    /// 
    /// - `Local`: Only exact ConceptID matches
    /// - `Subsystem`: Same subsystem prefix (first 2 bytes of ConceptID)
    /// - `Global`: All matching handlers including wildcards
    pub fn broadcast_scoped(
        &mut self,
        intent: &Intent, 
        has_cap: impl Fn(CapabilityType) -> bool,
        timestamp: u64,
        scope: BroadcastScope,
    ) -> BroadcastResult {
        self.sort_by_priority();
        
        let target_id = intent.concept_id;
        let mut result = BroadcastResult::with_scope(scope);
        let mut sequence: u16 = 0;
        
        for i in 0..self.count {
            let entry = &mut self.handlers[i];
            
            // Check scope-based matching
            if !scope.matches(entry.concept_id, target_id) {
                continue;
            }
            
            // Check if handler is inhibited by a previous handler
            if result.inhibited.contains(&entry.concept_id) {
                continue;
            }
            
            // Check refractory period (neural timing)
            if entry.is_refractory(timestamp) {
                continue;
            }
            
            // Check capability
            if let Some(cap) = entry.required_cap {
                if !has_cap(cap) {
                    continue;
                }
            }
            
            // Record firing time (for refractory period)
            entry.last_fired = timestamp;
            
            // Call handler and capture response
            let handler_result = (entry.handler)(intent);
            
            // Record response for aggregation
            let response = HandlerResponse {
                handler_name: entry.name,
                priority: entry.priority,
                result: handler_result.clone(),
                sequence,
            };
            result.responses.push(response).ok();
            sequence += 1;
            
            // Process result
            match handler_result {
                HandlerResult::Handled => {
                    result.handled_count += 1;
                    // Add this handler's static inhibitions
                    for inhibit_id in entry.get_inhibits() {
                        result.inhibited.push(*inhibit_id).ok();
                    }
                    // Continue to next handler (Broadcast)
                },
                HandlerResult::StopPropagation => {
                    result.handled_count += 1;
                    result.stopped = true;
                    // Still record this as the winner
                    result.winner = Some(entry.name);
                    return result;
                },
                HandlerResult::Inhibit(targets) => {
                    result.handled_count += 1;
                    // Add dynamic inhibitions
                    for target in targets {
                        result.inhibited.push(target).ok();
                    }
                    // Also add static inhibitions
                    for inhibit_id in entry.get_inhibits() {
                        result.inhibited.push(*inhibit_id).ok();
                    }
                },
                HandlerResult::Modulate(factor) => {
                    result.handled_count += 1;
                    result.modulation *= factor;
                },
                HandlerResult::NotHandled => {
                    // Still recorded in responses but doesn't count as handled
                },
                HandlerResult::Error(code) => {
                    crate::kprintln!("[HANDLER] {} error: {}", entry.name, code);
                    // Recorded in responses, continues broadcast
                }
            }
        }
        
        result
    }
    
    /// Broadcast with conflict resolution
    /// 
    /// Broadcasts to all handlers, then resolves conflicts using the specified strategy.
    /// Returns the broadcast result with the winner field populated.
    /// 
    /// # Resolution Strategies
    /// 
    /// - `FirstClaims`: First handler to claim (StopPropagation/Handled) wins
    /// - `HighestPriority`: Highest priority handler wins
    /// - `HighestActivation`: Handler with highest modulation wins
    /// - `Consensus`: Handler with strongest response type wins
    pub fn broadcast_with_resolution(
        &mut self,
        intent: &Intent, 
        has_cap: impl Fn(CapabilityType) -> bool,
        timestamp: u64,
        strategy: ConflictResolution,
    ) -> BroadcastResult {
        let mut result = self.broadcast(intent, has_cap, timestamp);
        result.resolve_winner(strategy);
        result
    }
    
    /// Broadcast to specific subsystem only
    /// 
    /// Convenience method for subsystem-scoped broadcast.
    /// Subsystem is determined by the first 2 bytes of ConceptID.
    pub fn broadcast_subsystem(
        &mut self,
        intent: &Intent, 
        has_cap: impl Fn(CapabilityType) -> bool,
        timestamp: u64,
    ) -> BroadcastResult {
        self.broadcast_scoped(intent, has_cap, timestamp, BroadcastScope::Subsystem)
    }
    
    /// Broadcast to exact match only (no wildcards)
    /// 
    /// Convenience method for local-scoped broadcast.
    /// Only handlers with exact ConceptID match will fire.
    pub fn broadcast_local(
        &mut self,
        intent: &Intent, 
        has_cap: impl Fn(CapabilityType) -> bool,
        timestamp: u64,
    ) -> BroadcastResult {
        self.broadcast_scoped(intent, has_cap, timestamp, BroadcastScope::Local)
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
    
    /// Get handlers that would match a given ConceptID under a scope
    pub fn matching_handlers(&self, concept_id: ConceptID, scope: BroadcastScope) -> usize {
        self.handlers[..self.count]
            .iter()
            .filter(|e| scope.matches(e.concept_id, concept_id))
            .count()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    use core::sync::atomic::{AtomicU32, Ordering};
    
    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);
    
    fn test_handler_a(_: &Intent) -> HandlerResult {
        TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        HandlerResult::Handled
    }
    
    fn test_handler_b(_: &Intent) -> HandlerResult {
        TEST_COUNTER.fetch_add(10, Ordering::Relaxed);
        HandlerResult::StopPropagation
    }
    
    fn _pass_through_handler(_: &Intent) -> HandlerResult {
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
        TEST_COUNTER.store(0, Ordering::Relaxed);
        
        registry.register(ConceptID(0x0001), test_handler_a, "test_a");
        
        let intent = Intent::new(ConceptID(0x0001));
        let handled = registry.dispatch(&intent, |_| true);
        
        assert!(handled);
        assert_eq!(TEST_COUNTER.load(Ordering::Relaxed), 1);
    }
    
    #[test]
    fn test_dispatch_priority() {
        let mut registry = HandlerRegistry::new();
        TEST_COUNTER.store(0, Ordering::Relaxed);
        
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
        assert_eq!(TEST_COUNTER.load(Ordering::Relaxed), 10);
    }
    
    #[test]
    fn test_dispatch_broadcast() {
        let mut registry = HandlerRegistry::new();
        TEST_COUNTER.store(0, Ordering::Relaxed);
        
        // Handler A (adds 1)
        registry.register(ConceptID(0x0001), test_handler_a, "handler_a");
        
        // Handler A again (adds 1) - simulating multiple listeners
        registry.register(ConceptID(0x0001), test_handler_a, "handler_a_2");
        
        let intent = Intent::new(ConceptID(0x0001));
        registry.dispatch(&intent, |_| true);
        
        // Both should run
        assert_eq!(TEST_COUNTER.load(Ordering::Relaxed), 2);
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
    
    // ═══════════════════════════════════════════════════════════════════════════
    // PHASE 2 TESTS: Scope, Response Aggregation, Winner-Take-All
    // ═══════════════════════════════════════════════════════════════════════════
    
    #[test]
    fn test_broadcast_scope_local() {
        let scope = BroadcastScope::Local;
        
        // Exact match
        assert!(scope.matches(ConceptID(0x1234), ConceptID(0x1234)));
        
        // Different ID - no match
        assert!(!scope.matches(ConceptID(0x1234), ConceptID(0x5678)));
        
        // Wildcard does NOT match in Local scope
        assert!(!scope.matches(ConceptID(0), ConceptID(0x1234)));
    }
    
    #[test]
    fn test_broadcast_scope_subsystem() {
        let scope = BroadcastScope::Subsystem;
        
        // Same subsystem (first 2 bytes match)
        let handler_id = ConceptID(0x0001_0000_0000_0001);
        let target_id = ConceptID(0x0001_0000_0000_9999);
        assert!(scope.matches(handler_id, target_id));
        
        // Different subsystem
        let different_subsystem = ConceptID(0x0002_0000_0000_0001);
        assert!(!scope.matches(different_subsystem, target_id));
        
        // Wildcard always matches
        assert!(scope.matches(ConceptID(0), target_id));
    }
    
    #[test]
    fn test_broadcast_scope_global() {
        let scope = BroadcastScope::Global;
        
        // Exact match
        assert!(scope.matches(ConceptID(0x1234), ConceptID(0x1234)));
        
        // Different ID - no match (Global still requires exact or wildcard)
        assert!(!scope.matches(ConceptID(0x1234), ConceptID(0x5678)));
        
        // Wildcard matches
        assert!(scope.matches(ConceptID(0), ConceptID(0x9999)));
    }
    
    #[test]
    fn test_broadcast_response_aggregation() {
        let mut registry = HandlerRegistry::new();
        TEST_COUNTER.store(0, Ordering::Relaxed);
        
        registry.register(ConceptID(0x0001), test_handler_a, "handler_1");
        registry.register(ConceptID(0x0001), test_handler_a, "handler_2");
        
        let intent = Intent::new(ConceptID(0x0001));
        let result = registry.broadcast(&intent, |_| true, 1000);
        
        // Check aggregation
        assert_eq!(result.handled_count, 2);
        assert_eq!(result.responses.len(), 2);
        assert_eq!(result.success_count(), 2);
        assert_eq!(result.error_count(), 0);
    }
    
    #[test]
    fn test_broadcast_stats() {
        let mut registry = HandlerRegistry::new();
        TEST_COUNTER.store(0, Ordering::Relaxed);
        
        registry.register(ConceptID(0x0001), test_handler_a, "handler_1");
        registry.register(ConceptID(0x0001), test_handler_a, "handler_2");
        
        let intent = Intent::new(ConceptID(0x0001));
        let result = registry.broadcast(&intent, |_| true, 1000);
        
        let stats = result.stats();
        assert_eq!(stats.total_handlers, 2);
        assert_eq!(stats.handled, 2);
        assert_eq!(stats.errors, 0);
        assert!(!stats.stopped);
    }
    
    fn modulating_handler(_: &Intent) -> HandlerResult {
        HandlerResult::Modulate(1.5)
    }
    
    #[test]
    fn test_conflict_resolution_highest_priority() {
        let mut registry = HandlerRegistry::new();
        
        registry.register_with_options(
            ConceptID(0x0001),
            test_handler_a,
            "low_priority",
            50,
            None,
        );
        registry.register_with_options(
            ConceptID(0x0001),
            test_handler_a,
            "high_priority",
            200,
            None,
        );
        
        let intent = Intent::new(ConceptID(0x0001));
        let mut result = registry.broadcast(&intent, |_| true, 1000);
        
        let winner = result.resolve_winner(ConflictResolution::HighestPriority);
        assert_eq!(winner, Some("high_priority"));
    }
    
    #[test]
    fn test_conflict_resolution_first_claims() {
        let mut registry = HandlerRegistry::new();
        
        registry.register_with_options(
            ConceptID(0x0001),
            test_handler_b, // Returns StopPropagation
            "first",
            200,
            None,
        );
        registry.register_with_options(
            ConceptID(0x0001),
            test_handler_a,
            "second",
            50,
            None,
        );
        
        let intent = Intent::new(ConceptID(0x0001));
        let result = registry.broadcast(&intent, |_| true, 1000);
        
        // StopPropagation handler should be winner
        assert_eq!(result.winner, Some("first"));
        assert!(result.stopped);
    }
    
    #[test]
    fn test_conflict_resolution_highest_activation() {
        let mut registry = HandlerRegistry::new();
        
        registry.register(ConceptID(0x0001), test_handler_a, "normal");
        registry.register(ConceptID(0x0001), modulating_handler, "high_activation");
        
        let intent = Intent::new(ConceptID(0x0001));
        let mut result = registry.broadcast(&intent, |_| true, 1000);
        
        let winner = result.resolve_winner(ConflictResolution::HighestActivation);
        assert_eq!(winner, Some("high_activation"));
    }
    
    #[test]
    fn test_broadcast_local_excludes_wildcards() {
        let mut registry = HandlerRegistry::new();
        TEST_COUNTER.store(0, Ordering::Relaxed);
        
        // Wildcard handler
        registry.register_wildcard(test_handler_a, "wildcard", 100);
        // Specific handler
        registry.register(ConceptID(0x0001), test_handler_a, "specific");
        
        let intent = Intent::new(ConceptID(0x0001));
        
        // Local scope should only match specific, not wildcard
        let result = registry.broadcast_local(&intent, |_| true, 1000);
        
        // Only specific handler should fire (Local scope excludes wildcards)
        assert_eq!(result.handled_count, 1);
        assert_eq!(result.responses.len(), 1);
        assert_eq!(result.responses[0].handler_name, "specific");
    }
    
    #[test]
    fn test_broadcast_with_resolution() {
        let mut registry = HandlerRegistry::new();
        
        registry.register(ConceptID(0x0001), test_handler_a, "handler_1");
        registry.register(ConceptID(0x0001), test_handler_a, "handler_2");
        
        let intent = Intent::new(ConceptID(0x0001));
        let result = registry.broadcast_with_resolution(
            &intent, 
            |_| true, 
            1000,
            ConflictResolution::Consensus
        );
        
        // Winner should be populated
        assert!(result.winner.is_some());
    }
    
    #[test]
    fn test_matching_handlers_count() {
        let mut registry = HandlerRegistry::new();
        
        registry.register(ConceptID(0x0001), test_handler_a, "handler_1");
        registry.register(ConceptID(0x0001), test_handler_a, "handler_2");
        registry.register(ConceptID(0x0002), test_handler_a, "handler_3");
        registry.register_wildcard(test_handler_a, "wildcard", 50);
        
        // Local scope - only exact matches
        assert_eq!(registry.matching_handlers(ConceptID(0x0001), BroadcastScope::Local), 2);
        
        // Global scope - includes wildcards
        assert_eq!(registry.matching_handlers(ConceptID(0x0001), BroadcastScope::Global), 3);
    }
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
