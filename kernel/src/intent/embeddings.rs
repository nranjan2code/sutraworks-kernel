
// ═══════════════════════════════════════════════════════════════════════════════
// STATIC EMBEDDINGS (Pre-computed)
// ═══════════════════════════════════════════════════════════════════════════════

// These vectors are manually crafted to be "orthogonal" (mostly zeros in different spots)
// or "aligned" (similar values) to demonstrate cosine similarity.

/// Vector for "Display" concept (High values in first quadrant)
pub const VEC_DISPLAY: [i8; 64] = [
    100, 100, 100, 100, 50, 50, 50, 50, 
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
];

/// Vector for "Compute" concept (High values in second quadrant)
pub const VEC_COMPUTE: [i8; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0,
    100, 100, 100, 100, 50, 50, 50, 50,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
];

/// Vector for "System" concept (High values in third quadrant)
pub const VEC_SYSTEM: [i8; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    100, 100, 100, 100, 50, 50, 50, 50,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
];

/// Vector for "Store" concept (High values in fourth quadrant)
pub const VEC_STORE: [i8; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    100, 100, 100, 100, 50, 50, 50, 50,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
];

/// Vector for "Retrieve" concept (Similar to Store but slightly different)
pub const VEC_RETRIEVE: [i8; 64] = [
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    80, 80, 80, 80, 40, 40, 40, 40, // Similar to Store
    20, 20, 20, 20, 0, 0, 0, 0,     // But with some variation
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
];

use crate::intent::{Embedding, ConceptID};

/// Helper to get embedding for a token
pub fn get_static_embedding(token: &str) -> Embedding {
    let vector = match token {
        // Synonyms for Display (should be close to VEC_DISPLAY)
        "show" | "display" | "print" | "screen" => VEC_DISPLAY,
        
        // Synonyms for Compute (should be close to VEC_COMPUTE)
        "calc" | "calculate" | "compute" | "math" | "add" => VEC_COMPUTE,
        
        // Synonyms for System (should be close to VEC_SYSTEM)
        "reboot" | "restart" | "system" | "reset" => VEC_SYSTEM,

        // Synonyms for Store
        "store" | "save" | "remember" | "keep" => VEC_STORE,

        // Synonyms for Retrieve
        "retrieve" | "load" | "recall" | "get" | "fetch" => VEC_RETRIEVE,
        
        // Unknown: Return a "noise" vector (all 10s)
        _ => [10; 64],
    };
    
    // We use a dummy ID for the query embedding
    Embedding::new(ConceptID(0), vector)
}
