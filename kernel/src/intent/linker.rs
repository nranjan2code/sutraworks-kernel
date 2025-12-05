//! Semantic Linker - Runtime resolution of intents to capabilities
//!
//! Simulates the HDC-based linking process.

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Trait for resolving semantic descriptions to concrete capabilities
pub trait SemanticLinker {
    /// Find a capability matching the description
    fn find_capability(&self, description: &str) -> Option<String>;
}

/// A linker that uses (simulated) Hyperdimensional Computing
pub struct HdcLinker {
    // In a real implementation, this would hold the HDC memory
    known_capabilities: Vec<(String, String)>, // (Description, ID)
}

impl HdcLinker {
    pub fn new() -> Self {
        Self {
            known_capabilities: Vec::new(),
        }
    }

    pub fn register_capability(&mut self, description: &str, id: &str) {
        self.known_capabilities.push((description.to_string(), id.to_string()));
    }
}

impl SemanticLinker for HdcLinker {
    fn find_capability(&self, description: &str) -> Option<String> {
        // Simulating vector similarity search
        // For now, just simple string matching or "fuzzy" match simulation
        for (desc, id) in &self.known_capabilities {
            if desc.contains(description) || description.contains(desc) {
                return Some(id.clone());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linker_resolution() {
        let mut linker = HdcLinker::new();
        linker.register_capability("Food Database", "cap.nutrition.db");
        linker.register_capability("User Journal", "cap.storage.journal");

        // Exact match
        assert_eq!(linker.find_capability("Food Database"), Some("cap.nutrition.db".to_string()));
        
        // Partial match (simulating semantic match)
        assert_eq!(linker.find_capability("Database"), Some("cap.nutrition.db".to_string()));
        
        // No match
        assert_eq!(linker.find_capability("Rocket Ship"), None);
    }
}

impl Default for HdcLinker {
    fn default() -> Self {
        Self::new()
    }
}
