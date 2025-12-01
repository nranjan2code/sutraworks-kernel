//! Intent Parser Unit Tests

use intent_kernel::intent::*;

pub
fn test_concept_id_hashing() {
    let id1 = ConceptID::from_str("test");
    let id2 = ConceptID::from_str("test");
    let id3 = ConceptID::from_str("different");
    
    assert_eq!(id1, id2, "Same strings should produce same ConceptID");
    assert_ne!(id1, id3, "Different strings should produce different ConceptID");
}

pub
fn test_embedding_creation() {
    let embedding = Embedding::new(
        ConceptID::from_str("test"),
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
         11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
         21, 22, 23, 24, 25, 26, 27, 28, 29, 30,
         31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
         41, 42, 43, 44, 45, 46, 47, 48, 49, 50,
         51, 52, 53, 54, 55, 56, 57, 58, 59, 60,
         61, 62, 63, 64]
    );
    
    assert_eq!(embedding.id, ConceptID::from_str("test"));
}

pub
fn test_embedding_similarity_identical() {
    let vec = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
               11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
               21, 22, 23, 24, 25, 26, 27, 28, 29, 30,
               31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
               41, 42, 43, 44, 45, 46, 47, 48, 49, 50,
               51, 52, 53, 54, 55, 56, 57, 58, 59, 60,
               61, 62, 63, 64];
    
    let emb1 = Embedding::new(ConceptID::from_str("test"), vec);
    let emb2 = Embedding::new(ConceptID::from_str("test"), vec);
    
    let similarity = emb1.similarity(&emb2);
    assert!(similarity > 95, "Identical embeddings should have >95% similarity, got {}", similarity);
}

pub
fn test_embedding_similarity_different() {
    let vec1 = [1; 64];
    let vec2 = [-1; 64];
    
    let emb1 = Embedding::new(ConceptID::from_str("test1"), vec1);
    let emb2 = Embedding::new(ConceptID::from_str("test2"), vec2);
    
    let similarity = emb1.similarity(&emb2);
    assert!(similarity < 50, "Opposite embeddings should have <50% similarity, got {}", similarity);
}

pub
fn test_neural_memory_basic() {
    let mut memory = NeuralMemory::new();
    
    let embedding = Embedding::new(
        ConceptID::from_str("test"),
        [1; 64]
    );
    
    memory.remember(embedding);
    
    // Try to recall with same embedding
    let query = Embedding::new(
        ConceptID::from_str("test"),
        [1; 64]
    );
    
    let result = memory.recall(&query);
    assert!(result.is_some(), "Should recall stored embedding");
}

pub
fn test_neural_memory_threshold() {
    let mut memory = NeuralMemory::new();
    
    // Store an embedding
    let stored = Embedding::new(
        ConceptID::from_str("stored"),
        [1; 64]
    );
    memory.remember(stored);
    
    // Query with very different embedding
    let query = Embedding::new(
        ConceptID::from_str("query"),
        [-1; 64]
    );
    
    let result = memory.recall(&query);
    assert!(result.is_none(), "Should not recall below 70% threshold");
}
