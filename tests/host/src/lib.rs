//! Intent Kernel - Host-Based Test Library
//!
//! This crate contains pure Rust implementations of kernel logic
//! that can be tested natively on the host machine (Mac/Linux/Windows).
//!
//! These are NOT the kernel modules - they are test-friendly re-implementations
//! of the core algorithms without hardware dependencies.

pub mod stroke;
pub mod capability;
pub mod dictionary;
pub mod concept;
pub mod history;
pub mod queue;
pub mod handlers;

// Re-exports for convenience
pub use concept::concepts;
pub use concept::{ConceptID, Intent, IntentData};
