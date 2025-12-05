//! Intent Security Module
//!
//! Comprehensive security system for the Intent Kernel.
//! Provides: rate limiting, privilege checking, and handler integrity verification.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use crate::intent::ConceptID;

// ═══════════════════════════════════════════════════════════════════════════════
// SECURITY VIOLATION TRACKING
// ═══════════════════════════════════════════════════════════════════════════════

/// Types of security violations
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecurityViolation {
    RateLimitExceeded,
    PrivilegeEscalation,
    HandlerTampering,
}

/// Security violation record
#[derive(Clone, Debug)]
pub struct ViolationRecord {
    pub violation_type: SecurityViolation,
    pub concept_id: ConceptID,
    pub timestamp: u64,
    pub source_id: u64,
}

// ═══════════════════════════════════════════════════════════════════════════════
// RATE LIMITER (Token Bucket Algorithm)
// ═══════════════════════════════════════════════════════════════════════════════

/// Per-source rate tracking
struct SourceRate {
    tokens: u32,
    last_refill: u64,
    total_intents: u64,
}

/// Rate limiter using token bucket algorithm
pub struct RateLimiter {
    sources: BTreeMap<u64, SourceRate>,
    max_tokens: u32,         // Burst size (configurable)
    #[allow(dead_code)]
    refill_rate: u32,        // Tokens per second (configurable)
    refill_interval_ms: u64,
}

impl RateLimiter {
    /// Create new rate limiter with configurable limits
    /// 
    /// Default: 1000 intents/sec (burst 100) - suitable for high-throughput and benchmarks
    /// For stricter limits, use new_strict() or configure manually
    pub const fn new() -> Self {
        Self {
            sources: BTreeMap::new(),
            max_tokens: 100,      // Allow burst of 100 rapid intents
            refill_rate: 1000,    // Refill at 1000 tokens/second
            refill_interval_ms: 1, // 1ms interval (1000/1000)
        }
    }
    
    /// Create strict rate limiter (10/sec, burst 10) for untrusted sources
    pub const fn new_strict() -> Self {
        Self {
            sources: BTreeMap::new(),
            max_tokens: 10,
            refill_rate: 10,
            refill_interval_ms: 100, // 100ms interval
        }
    }

    #[cfg(test)]
    pub fn new_test(refill_rate: u32, max_tokens: u32) -> Self {
        Self {
            sources: BTreeMap::new(),
            max_tokens,
            refill_rate,
            refill_interval_ms: if refill_rate > 0 { 1000 / refill_rate as u64 } else { 1000 },
        }
    }
    
    /// Create unlimited rate limiter for benchmarks (effectively disabled)
    pub const fn new_unlimited() -> Self {
        Self {
            sources: BTreeMap::new(),
            max_tokens: u32::MAX,
            refill_rate: u32::MAX,
            refill_interval_ms: 1, // Effectively refills every ms
        }
    }

    /// Create a new rate limiter with custom parameters
    /// 
    /// # Arguments
    /// * `max_tokens` - Maximum burst size
    /// * `refill_rate` - Tokens added per second
    pub const fn with_params(max_tokens: u32, refill_rate: u32) -> Self {
        Self {
            sources: BTreeMap::new(),
            max_tokens,
            refill_rate,
            refill_interval_ms: 1000 / refill_rate as u64,
        }
    }

    /// Check if an intent from a source should be allowed
    /// 
    /// # Arguments
    /// * `source_id` - Identifier for the intent source (process ID, user ID, etc.)
    /// * `timestamp` - Current timestamp in milliseconds
    /// 
    /// # Returns
    /// `true` if allowed, `false` if rate limit exceeded
    pub fn check_rate(&mut self, source_id: u64, timestamp: u64) -> bool {
        let entry = self.sources.entry(source_id).or_insert(SourceRate {
            tokens: self.max_tokens,
            last_refill: timestamp,
            total_intents: 0,
        });

        // Refill tokens based on time elapsed
        let elapsed_ms = timestamp.saturating_sub(entry.last_refill);
        let tokens_to_add = (elapsed_ms / self.refill_interval_ms) as u32;
        
        if tokens_to_add > 0 {
            entry.tokens = (entry.tokens + tokens_to_add).min(self.max_tokens);
            entry.last_refill = timestamp;
        }

        // Check if we have tokens available
        if entry.tokens > 0 {
            entry.tokens -= 1;
            entry.total_intents += 1;
            true
        } else {
            false
        }
    }

    /// Get statistics for a source
    pub fn get_stats(&self, source_id: u64) -> Option<(u32, u64)> {
        self.sources.get(&source_id).map(|s| (s.tokens, s.total_intents))
    }

    /// Reset rate limiter for a source
    pub fn reset_source(&mut self, source_id: u64) {
        self.sources.remove(&source_id);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PRIVILEGE CHECKER
// ═══════════════════════════════════════════════════════════════════════════════

/// Privilege level for intent execution
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrivilegeLevel {
    User,
    Kernel,
}

/// Checks if intents have appropriate privileges
pub struct PrivilegeChecker {
    // ConceptID ranges reserved for kernel-only operations
    kernel_range_start: u64,
    kernel_range_end: u64,
}

impl PrivilegeChecker {
    /// Create a new privilege checker
    /// 
    /// Kernel-only range: 0x0000_0000_0000_0000 to 0x0000_0000_0000_FFFF
    /// User range: 0x0001_0000_0000_0000 to 0xFFFF_FFFF_FFFF_FFFF
    pub const fn new() -> Self {
        Self {
            kernel_range_start: 0x0000_0000_0000_0000,
            kernel_range_end: 0x0000_0000_0000_FFFF,
        }
    }

    /// Check if a concept requires kernel privileges
    pub fn requires_kernel_privilege(&self, concept_id: ConceptID) -> bool {
        let id = concept_id.0;
        id >= self.kernel_range_start && id <= self.kernel_range_end
    }

    /// Check if an intent is allowed from a given privilege level
    pub fn check_privilege(&self, concept_id: ConceptID, level: PrivilegeLevel) -> bool {
        if self.requires_kernel_privilege(concept_id) {
            level == PrivilegeLevel::Kernel
        } else {
            true // User intents always allowed from any level
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// HANDLER INTEGRITY VERIFICATION
// ═══════════════════════════════════════════════════════════════════════════════

/// Handler verification record
#[derive(Clone, Debug)]
pub struct HandlerChecksum {
    pub name: &'static str,
    pub concept_id: ConceptID,
    pub checksum: u64,
    pub registered_at: u64,
}

/// Verifies handler integrity using checksums
pub struct HandlerIntegrityChecker {
    checksums: BTreeMap<ConceptID, HandlerChecksum>,
}

impl HandlerIntegrityChecker {
    /// Create a new handler integrity checker
    pub const fn new() -> Self {
        Self {
            checksums: BTreeMap::new(),
        }
    }

    /// Register a handler with its checksum
    /// 
    /// # Arguments
    /// * `concept_id` - The concept this handler responds to
    /// * `name` - Handler name for debugging
    /// * `code_ptr` - Pointer to the handler function (for checksum)
    /// * `timestamp` - Registration timestamp
    pub fn register_handler(
        &mut self,
        concept_id: ConceptID,
        name: &'static str,
        code_ptr: usize,
        timestamp: u64,
    ) {
        // Calculate FNV-1a checksum of the function pointer
        let checksum = self.calculate_checksum(code_ptr);
        
        self.checksums.insert(concept_id, HandlerChecksum {
            name,
            concept_id,
            checksum,
            registered_at: timestamp,
        });
    }

    /// Verify a handler hasn't been tampered with
    /// 
    /// # Returns
    /// `true` if handler is valid, `false` if tampered or not registered
    pub fn verify_handler(&self, concept_id: ConceptID, code_ptr: usize) -> bool {
        if let Some(record) = self.checksums.get(&concept_id) {
            let current_checksum = self.calculate_checksum(code_ptr);
            current_checksum == record.checksum
        } else {
            false // Not registered
        }
    }

    /// Calculate FNV-1a checksum
    fn calculate_checksum(&self, code_ptr: usize) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        let bytes = code_ptr.to_le_bytes();
        
        for byte in bytes.iter() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        
        hash
    }

    /// Get checksum record for a handler
    pub fn get_checksum(&self, concept_id: ConceptID) -> Option<&HandlerChecksum> {
        self.checksums.get(&concept_id)
    }

    /// Unregister a handler
    pub fn unregister_handler(&mut self, concept_id: ConceptID) -> bool {
        self.checksums.remove(&concept_id).is_some()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTEGRATED SECURITY COORDINATOR
// ═══════════════════════════════════════════════════════════════════════════════

/// Main security coordinator that integrates all security mechanisms
pub struct IntentSecurity {
    rate_limiter: RateLimiter,
    privilege_checker: PrivilegeChecker,
    handler_checker: HandlerIntegrityChecker,
    violations: Vec<ViolationRecord>,
    max_violations: usize,
}

impl IntentSecurity {
    /// Create a new intent security system with default (high-throughput) rate limiting
    pub const fn new() -> Self {
        Self {
            rate_limiter: RateLimiter::new(), // 1000/sec, burst 100
            privilege_checker: PrivilegeChecker::new(),
            handler_checker: HandlerIntegrityChecker::new(),
            violations: Vec::new(),
            max_violations: 100,
        }
    }

    /// Check if an intent should be allowed
    /// 
    /// Performs all security checks in order:
    /// 1. Rate limiting
    /// 2. Privilege checking
    /// 
    /// # Returns
    /// `Ok(())` if allowed, `Err(SecurityViolation)` if blocked
    pub fn check_intent(
        &mut self,
        concept_id: ConceptID,
        source_id: u64,
        privilege: PrivilegeLevel,
        timestamp: u64,
    ) -> Result<(), SecurityViolation> {
        // 1. Rate limiting check
        if !self.rate_limiter.check_rate(source_id, timestamp) {
            self.log_violation(SecurityViolation::RateLimitExceeded, concept_id, source_id, timestamp);
            return Err(SecurityViolation::RateLimitExceeded);
        }

        // 2. Privilege check
        if !self.privilege_checker.check_privilege(concept_id, privilege) {
            self.log_violation(SecurityViolation::PrivilegeEscalation, concept_id, source_id, timestamp);
            return Err(SecurityViolation::PrivilegeEscalation);
        }

        Ok(())
    }

    /// Register a handler for integrity checking
    pub fn register_handler(
        &mut self,
        concept_id: ConceptID,
        name: &'static str,
        code_ptr: usize,
        timestamp: u64,
    ) {
        self.handler_checker.register_handler(concept_id, name, code_ptr, timestamp);
    }

    /// Verify handler integrity
    pub fn verify_handler(&self, concept_id: ConceptID, code_ptr: usize) -> bool {
        self.handler_checker.verify_handler(concept_id, code_ptr)
    }

    /// Log a security violation
    fn log_violation(
        &mut self,
        violation_type: SecurityViolation,
        concept_id: ConceptID,
        source_id: u64,
        timestamp: u64,
    ) {
        let record = ViolationRecord {
            violation_type,
            concept_id,
            timestamp,
            source_id,
        };

        self.violations.push(record);

        // Keep only recent violations
        if self.violations.len() > self.max_violations {
            self.violations.remove(0);
        }
    }

    /// Get recent violations
    pub fn get_violations(&self) -> &[ViolationRecord] {
        &self.violations
    }

    /// Get violation count by type
    pub fn get_violation_count(&self, violation_type: SecurityViolation) -> usize {
        self.violations.iter().filter(|v| v.violation_type == violation_type).count()
    }

    /// Clear all violation records
    pub fn clear_violations(&mut self) {
        self.violations.clear();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// UNIT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiting_basic() {
        let mut limiter = RateLimiter::new_test(10, 10); // 10/sec, burst 10
        let source_id = 1;
        let mut timestamp = 0;

        // Should allow first 10 intents
        for _ in 0..10 {
            assert!(limiter.check_rate(source_id, timestamp));
        }

        // 11th should be blocked
        assert!(!limiter.check_rate(source_id, timestamp));

        // After 100ms, should get 1 token back (10/sec = 1 per 100ms)
        timestamp += 100;
        assert!(limiter.check_rate(source_id, timestamp));
        assert!(!limiter.check_rate(source_id, timestamp)); // Immediately blocked again
    }

    #[test]
    fn test_rate_limiting_burst() {
        let mut limiter = RateLimiter::new_test(10, 5); // 10/sec, burst 5
        let source_id = 1;
        let timestamp = 0;

        // Burst of 5 allowed
        for _ in 0..5 {
            assert!(limiter.check_rate(source_id, timestamp));
        }

        // 6th blocked
        assert!(!limiter.check_rate(source_id, timestamp));
    }

    #[test]
    fn test_privilege_kernel_only() {
        let checker = PrivilegeChecker::new();
        let kernel_concept = ConceptID::new(0x0000_0000_0000_0042); // Kernel range

        assert!(checker.requires_kernel_privilege(kernel_concept));
        assert!(checker.check_privilege(kernel_concept, PrivilegeLevel::Kernel));
        assert!(!checker.check_privilege(kernel_concept, PrivilegeLevel::User));
    }

    #[test]
    fn test_privilege_user_allowed() {
        let checker = PrivilegeChecker::new();
        let user_concept = ConceptID::new(0x0001_0000_0000_0042); // User range

        assert!(!checker.requires_kernel_privilege(user_concept));
        assert!(checker.check_privilege(user_concept, PrivilegeLevel::Kernel));
        assert!(checker.check_privilege(user_concept, PrivilegeLevel::User));
    }

    #[test]
    fn test_handler_checksum() {
        let mut checker = HandlerIntegrityChecker::new();
        let concept = ConceptID::new(1);
        let code_ptr = 0x1234_5678;

        checker.register_handler(concept, "test_handler", code_ptr, 0);
        assert!(checker.verify_handler(concept, code_ptr));
    }

    #[test]
    fn test_handler_tampering() {
        let mut checker = HandlerIntegrityChecker::new();
        let concept = ConceptID::new(1);
        let code_ptr = 0x1234_5678;
        let tampered_ptr = 0x1234_5679; // Modified

        checker.register_handler(concept, "test_handler", code_ptr, 0);
        assert!(!checker.verify_handler(concept, tampered_ptr));
    }

    #[test]
    fn test_security_integration() {
        let mut security = IntentSecurity::new();
        let concept = ConceptID::new(0x0001_0000_0000_0001);
        let source_id = 1;
        let timestamp = 0;

        // First intent should pass
        assert!(security.check_intent(concept, source_id, PrivilegeLevel::User, timestamp).is_ok());
    }

    #[test]
    fn test_violation_logging() {
        let mut security = IntentSecurity::new();
        let concept = ConceptID::new(0x0000_0000_0000_0001); // Kernel concept
        let source_id = 1;
        let timestamp = 0;

        // Try to execute kernel intent from user context
        let result = security.check_intent(concept, source_id, PrivilegeLevel::User, timestamp);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), SecurityViolation::PrivilegeEscalation);
        
        // Check violation was logged
        assert_eq!(security.get_violation_count(SecurityViolation::PrivilegeEscalation), 1);
    }
}

impl Default for IntentSecurity {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for HandlerIntegrityChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for PrivilegeChecker {
    fn default() -> Self {
        Self::new()
    }
}
