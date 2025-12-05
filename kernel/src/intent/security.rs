//! Intent Security Module
//!
//! Comprehensive security system using Hyperdimensional Computing for anomaly detection.
//! Provides: rate limiting, privilege checking, handler integrity, and semantic anomaly detection.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;
use crate::intent::ConceptID;
use crate::kernel::memory::neural::{Hypervector, hamming_similarity};

// ═══════════════════════════════════════════════════════════════════════════════
// SECURITY VIOLATION TRACKING
// ═══════════════════════════════════════════════════════════════════════════════

/// Types of security violations
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecurityViolation {
    RateLimitExceeded,
    PrivilegeEscalation,
    HandlerTampering,
    SemanticAnomaly,
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
            refill_interval_ms: 1000 / 1000 as u64, // 1ms interval
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
// HDC-BASED SEMANTIC ANOMALY DETECTION
// ═══════════════════════════════════════════════════════════════════════════════

/// Semantic baseline for normal intent patterns
pub struct SemanticBaseline {
    /// Normal intent pattern (bundled hypervector of typical intents)
    baseline: Hypervector,
    /// Number of samples used to build baseline
    sample_count: usize,
    /// Recent intent history for online learning
    history: VecDeque<Hypervector>,
    /// Maximum history size
    max_history: usize,
}

impl SemanticBaseline {
    /// Create a new semantic baseline
    pub const fn new() -> Self {
        Self {
            baseline: [0u64; 16],
            sample_count: 0,
            history: VecDeque::new(),
            max_history: 10,
        }
    }

    /// Learn baseline from a set of normal intent hypervectors
    /// 
    /// Uses HDC bundling (majority rule) to create a composite "normal" pattern
    pub fn learn_baseline(&mut self, samples: &[Hypervector]) {
        if samples.is_empty() {
            return;
        }

        // For bundling in binary HDC, we use majority voting across all samples
        // This creates a prototype that is similar to all inputs
        let mut result = [0u64; 16];
        
        for i in 0..16 {
            let mut bit_counts = [0u32; 64];
            
            // Count bits across all samples
            for sample in samples.iter() {
                for bit_pos in 0..64 {
                    if (sample[i] >> bit_pos) & 1 == 1 {
                        bit_counts[bit_pos] += 1;
                    }
                }
            }
            
            // Majority vote
            let threshold = (samples.len() / 2) as u32;
            let mut word = 0u64;
            for bit_pos in 0..64 {
                if bit_counts[bit_pos] > threshold {
                    word |= 1u64 << bit_pos;
                }
            }
            
            result[i] = word;
        }

        self.baseline = result;
        self.sample_count = samples.len();
    }

    /// Add a sample to the history and update baseline online
    pub fn update_baseline(&mut self, sample: Hypervector) {
        self.history.push_back(sample);
        
        if self.history.len() > self.max_history {
            self.history.pop_front();
        }

        // Rebuild baseline from history if we have enough samples
        if self.history.len() >= 3 {
            let samples: Vec<Hypervector> = self.history.iter().copied().collect();
            self.learn_baseline(&samples);
        }
    }

    /// Check if a hypervector is similar to the baseline
    /// 
    /// # Arguments
    /// * `query` - Hypervector to check
    /// * `threshold` - Similarity threshold (0.0 to 1.0)
    /// 
    /// # Returns
    /// `true` if similar (normal), `false` if anomalous
    pub fn is_normal(&self, query: &Hypervector, threshold: f32) -> bool {
        if self.sample_count == 0 {
            // No baseline learned yet, assume normal
            return true;
        }

        let similarity = hamming_similarity(&self.baseline, query);
        similarity >= threshold
    }

    /// Get baseline similarity to a query
    pub fn similarity(&self, query: &Hypervector) -> f32 {
        if self.sample_count == 0 {
            return 1.0; // No baseline = everything is normal
        }
        hamming_similarity(&self.baseline, query)
    }

    /// Check if baseline is initialized
    pub fn is_initialized(&self) -> bool {
        self.sample_count > 0
    }
}

/// Anomaly detector using HDC similarity
pub struct AnomalyDetector {
    baseline: SemanticBaseline,
    anomaly_threshold: f32,
    learning_mode: bool,
}

impl AnomalyDetector {
    /// Create a new anomaly detector
    /// 
    /// # Arguments
    /// * `threshold` - Minimum similarity to baseline (default: 0.3)
    pub const fn new(threshold: f32) -> Self {
        Self {
            baseline: SemanticBaseline::new(),
            anomaly_threshold: threshold,
            learning_mode: true,
        }
    }

    /// Train the detector with normal intent patterns
    pub fn train(&mut self, samples: &[Hypervector]) {
        self.baseline.learn_baseline(samples);
        self.learning_mode = false;
    }

    /// Detect if an intent is anomalous
    /// 
    /// # Returns
    /// `(is_anomaly, similarity_score)`
    pub fn detect(&mut self, intent_hv: &Hypervector) -> (bool, f32) {
        let similarity = self.baseline.similarity(intent_hv);
        
        // In learning mode, add to baseline
        if self.learning_mode {
            self.baseline.update_baseline(*intent_hv);
        }
        
        let is_anomaly = !self.baseline.is_normal(intent_hv, self.anomaly_threshold);
        (is_anomaly, similarity)
    }

    /// Enable or disable learning mode
    pub fn set_learning_mode(&mut self, enabled: bool) {
        self.learning_mode = enabled;
    }

    /// Get current anomaly threshold
    pub fn get_threshold(&self) -> f32 {
        self.anomaly_threshold
    }

    /// Set anomaly threshold
    pub fn set_threshold(&mut self, threshold: f32) {
        self.anomaly_threshold = threshold.clamp(0.0, 1.0);
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
    anomaly_detector: AnomalyDetector,
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
            anomaly_detector: AnomalyDetector::new(0.3), // 30% similarity threshold
            violations: Vec::new(),
            max_violations: 100,
        }
    }

    /// Check if an intent should be allowed
    /// 
    /// Performs all security checks in order:
    /// 1. Rate limiting
    /// 2. Privilege checking
    /// 3. Semantic anomaly detection
    /// 
    /// # Returns
    /// `Ok(())` if allowed, `Err(SecurityViolation)` if blocked
    pub fn check_intent(
        &mut self,
        concept_id: ConceptID,
        source_id: u64,
        privilege: PrivilegeLevel,
        intent_hv: &Hypervector,
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

        // 3. Anomaly detection
        let (is_anomaly, _similarity) = self.anomaly_detector.detect(intent_hv);
        if is_anomaly {
            self.log_violation(SecurityViolation::SemanticAnomaly, concept_id, source_id, timestamp);
            // Note: We log but don't block anomalies (could be false positive)
            // In production, this would trigger additional monitoring
        }

        Ok(())
    }

    /// Train anomaly detector with normal intent patterns
    pub fn train_anomaly_detector(&mut self, samples: &[Hypervector]) {
        self.anomaly_detector.train(samples);
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
    fn test_baseline_learning() {
        let mut baseline = SemanticBaseline::new();
        
        // Create 3 similar hypervectors
        let hv1 = [0xFFFF_FFFF_FFFF_FFFF; 16];
        let hv2 = [0xFFFF_FFFF_FFFF_FFFE; 16];
        let hv3 = [0xFFFF_FFFF_FFFF_FFFD; 16];
        
        baseline.learn_baseline(&[hv1, hv2, hv3]);
        assert!(baseline.is_initialized());
        
        // Should recognize similar patterns
        assert!(baseline.is_normal(&hv1, 0.9));
    }

    #[test]
    fn test_anomaly_detection() {
        let mut detector = AnomalyDetector::new(0.6);
        
        // Train with normal patterns
        let normal1 = [0xAAAA_AAAA_AAAA_AAAA; 16];
        let normal2 = [0xAAAA_AAAA_AAAA_BBBB; 16];
        detector.train(&[normal1, normal2]);
        
        // Similar pattern should be normal
        let similar = [0xAAAA_AAAA_AAAA_CCCC; 16];
        let (is_anomaly, _sim) = detector.detect(&similar);
        assert!(!is_anomaly);
        
        // Very different pattern should be anomalous
        let anomalous = [0x0000_0000_0000_0000; 16];
        let (is_anomaly, _sim) = detector.detect(&anomalous);
        assert!(is_anomaly);
    }

    #[test]
    fn test_security_integration() {
        let mut security = IntentSecurity::new();
        let concept = ConceptID::new(0x0001_0000_0000_0001);
        let source_id = 1;
        let timestamp = 0;
        let hv = [0xAAAA_AAAA_AAAA_AAAA; 16];

        // First intent should pass
        assert!(security.check_intent(concept, source_id, PrivilegeLevel::User, &hv, timestamp).is_ok());
    }

    #[test]
    fn test_violation_logging() {
        let mut security = IntentSecurity::new();
        let concept = ConceptID::new(0x0000_0000_0000_0001); // Kernel concept
        let source_id = 1;
        let timestamp = 0;
        let hv = [0x0; 16];

        // Try to execute kernel intent from user context
        let result = security.check_intent(concept, source_id, PrivilegeLevel::User, &hv, timestamp);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), SecurityViolation::PrivilegeEscalation);
        
        // Check violation was logged
        assert_eq!(security.get_violation_count(SecurityViolation::PrivilegeEscalation), 1);
    }
}
