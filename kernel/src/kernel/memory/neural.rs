//! Neural Memory Allocator
//!
//! Simplified semantic memory for stroke-native kernel.
//! Blocks are tagged with ConceptIDs for semantic retrieval.

use core::ptr::NonNull;
use alloc::alloc::{alloc, Layout};
use crate::intent::ConceptID;
use crate::arch::SpinLock;

/// Global Neural Allocator instance
pub static NEURAL_ALLOCATOR: SpinLock<NeuralAllocator> = SpinLock::new(NeuralAllocator::new());

/// A Semantic Block of memory
#[repr(C)]
pub struct SemanticBlock {
    pub concept_id: ConceptID,
    pub access_count: u64,
    pub size: usize,
    pub embedding: [f32; 64], // 64-dimensional semantic vector
    // Data follows immediately after this struct
}

/// Content-Addressable Pointer
#[derive(Clone, Copy, Debug)]
pub struct IntentPtr {
    pub id: ConceptID,
    pub ptr: NonNull<u8>,
    pub size: usize,
    pub similarity: f32,
}

/// Neural Allocator
pub struct NeuralAllocator {
    blocks: [Option<NonNull<SemanticBlock>>; 128],
    count: usize,
}

// SAFETY: NeuralAllocator is protected by SpinLock. The raw pointers are
// managed internally and we ensure they are valid.
unsafe impl Send for NeuralAllocator {}

impl NeuralAllocator {
    pub const fn new() -> Self {
        NeuralAllocator {
            blocks: [None; 128],
            count: 0,
        }
    }

    /// Allocate memory with a concept ID tag and semantic vector
    pub unsafe fn alloc(&mut self, size: usize, concept_id: ConceptID, embedding: [f32; 64]) -> Option<IntentPtr> {
        // 1. Allocate raw memory for header + data
        let total_size = core::mem::size_of::<SemanticBlock>() + size;
        let layout = Layout::from_size_align(total_size, 16).ok()?;
        
        let ptr = alloc(layout);
        if ptr.is_null() {
            return None;
        }

        // 2. Initialize SemanticBlock header
        let block = ptr as *mut SemanticBlock;
        (*block).concept_id = concept_id;
        (*block).access_count = 0;
        (*block).size = size;
        (*block).embedding = embedding;
        
        // 3. Register in our index
        if self.count < 128 {
            self.blocks[self.count] = NonNull::new(block);
            self.count += 1;
        } else {
            // Eviction: overwrite last one
            self.blocks[127] = NonNull::new(block);
        }

        Some(IntentPtr {
            id: concept_id,
            ptr: NonNull::new_unchecked(ptr.add(core::mem::size_of::<SemanticBlock>())),
            size,
            similarity: 1.0,
        })
    }

    /// Retrieve memory by concept ID (Exact Match)
    pub unsafe fn retrieve(&self, concept_id: ConceptID) -> Option<IntentPtr> {
        for i in 0..self.count {
            if let Some(block_ptr) = self.blocks[i] {
                let block = block_ptr.as_ref();
                if block.concept_id == concept_id {
                    let data_ptr = (block_ptr.as_ptr() as *mut u8).add(core::mem::size_of::<SemanticBlock>());
                    return Some(IntentPtr {
                        id: block.concept_id,
                        ptr: NonNull::new_unchecked(data_ptr),
                        size: block.size,
                        similarity: 1.0,
                    });
                }
            }
        }
        None
    }
    
    /// Retrieve nearest memory block by semantic vector (Fuzzy Match)
    pub unsafe fn retrieve_nearest(&self, query: &[f32; 64]) -> Option<IntentPtr> {
        let mut best_match: Option<IntentPtr> = None;
        let mut best_sim = -1.0;
        
        for i in 0..self.count {
            if let Some(block_ptr) = self.blocks[i] {
                let block = block_ptr.as_ref();
                let sim = cosine_similarity(query, &block.embedding);
                
                if sim > best_sim {
                    best_sim = sim;
                    let data_ptr = (block_ptr.as_ptr() as *mut u8).add(core::mem::size_of::<SemanticBlock>());
                    best_match = Some(IntentPtr {
                        id: block.concept_id,
                        ptr: NonNull::new_unchecked(data_ptr),
                        size: block.size,
                        similarity: sim,
                    });
                }
            }
        }
        
        best_match
    }
    
    /// Get count of allocated blocks
    pub fn count(&self) -> usize {
        self.count
    }
}

/// Calculate Cosine Similarity between two vectors
/// Sim(A, B) = (A . B) / (||A|| * ||B||)
fn cosine_similarity(a: &[f32; 64], b: &[f32; 64]) -> f32 {
    let mut dot_product = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    
    for i in 0..64 {
        dot_product += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }
    
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    
    // Use libm sqrt if available, or a fast approximation.
    // Since we are no_std and might not have libm linked easily, let's use a simple approximation
    // or just assume we have sqrt from core::intrinsics or similar if we enabled features.
    // For now, let's use a Newton-Raphson sqrt approximation for f32.
    
    let sqrt_a = sqrt_f32(norm_a);
    let sqrt_b = sqrt_f32(norm_b);
    
    dot_product / (sqrt_a * sqrt_b)
}

fn sqrt_f32(n: f32) -> f32 {
    if n < 0.0 { return 0.0; }
    if n == 0.0 { return 0.0; }
    let mut x = n;
    for _ in 0..10 {
        x = 0.5 * (x + n / x);
    }
    x
}
