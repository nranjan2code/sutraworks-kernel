//! Intent Parser Unit Tests

use intent_kernel::intent::*;
use intent_kernel::kernel::memory::neural::{NeuralAllocator, IntentPtr};

pub
fn test_concept_id_hashing() {
    let id1 = ConceptID::from_str("test");
    let id2 = ConceptID::from_str("test");
    let id3 = ConceptID::from_str("different");
    
    assert_eq!(id1, id2, "Same strings should produce same ConceptID");
    assert_ne!(id1, id3, "Different strings should produce different ConceptID");
}

pub
fn test_neural_memory_basic() {
    // NeuralAllocator::new() is const, but we need a mutable instance to alloc
    let mut memory = NeuralAllocator::new();
    
    // Initialize memory subsystem for the allocator to work
    // In kernel_tests environment, heap should be available if initialized.
    
    let id = ConceptID::from_str("test");
    
    unsafe {
        // Allocate 128 bytes
        if let Some(ptr) = memory.alloc(128, id) {
            // Verify allocation properties
            assert_eq!(ptr.id, id, "Allocated ptr should have correct ID");
            assert_eq!(ptr.size, 128, "Allocated ptr should have correct size");
            
            // Verify writing to memory (raw pointer access)
            let slice = core::slice::from_raw_parts_mut(ptr.ptr.as_ptr(), 128);
            slice[0] = 42;
            slice[127] = 255;
            assert_eq!(slice[0], 42);
            assert_eq!(slice[127], 255);

            // Verify Retrieval
            if let Some(retrieved) = memory.retrieve(id) {
                assert_eq!(retrieved.id, id, "Retrieved ID match");
                assert_eq!(retrieved.ptr, ptr.ptr, "Retrieved pointer match");
                
                let r_slice = core::slice::from_raw_parts(retrieved.ptr.as_ptr(), 128);
                assert_eq!(r_slice[0], 42, "Data persistence");
            } else {
                panic!("Failed to retrieve allocated concept");
            }
        } else {
            // Allocation behavior depends on available memory/mocks. 
            // In QEMU test environment, this should succeed.
            panic!("NeuralAllocator::alloc failed - Out of memory or initialization issue?");
        }
    }
}
