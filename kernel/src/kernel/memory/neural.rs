//! Neural Memory Allocator
//!
//! Implements Semantic Memory where blocks are allocated based on vector similarity.

use core::ptr::NonNull;
use alloc::alloc::{alloc, Layout};
use crate::intent::{Embedding, ConceptID};
use crate::arch::SpinLock;

/// Global Neural Allocator instance
pub static NEURAL_ALLOCATOR: SpinLock<NeuralAllocator> = SpinLock::new(NeuralAllocator::new());

/// A Semantic Block of memory
///
/// Contains the data and its semantic embedding.
#[repr(C)]
pub struct SemanticBlock {
    pub embedding: Embedding,
    pub access_count: u64,
    // Data follows immediately after this struct
}

/// Content-Addressable Pointer
#[derive(Clone, Copy, Debug)]
pub struct IntentPtr {
    pub id: ConceptID,
    pub ptr: NonNull<u8>,
}

/// Neural Allocator
pub struct NeuralAllocator {
    // In a real system, this would be a sophisticated vector index (HNSW, etc.)
    // For now, we use a simple list of blocks.
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

    /// Allocate memory semantically
    pub unsafe fn alloc(&mut self, size: usize, embedding: Embedding) -> Option<IntentPtr> {
        // 1. Allocate raw memory for header + data
        let total_size = core::mem::size_of::<SemanticBlock>() + size;
        let layout = Layout::from_size_align(total_size, 16).ok()?;
        
        // Use the global allocator (Buddy/Slab) to get raw bytes
        let ptr = alloc(layout);
        if ptr.is_null() {
            return None;
        }

        // 2. Initialize SemanticBlock header
        let block = ptr as *mut SemanticBlock;
        (*block).embedding = embedding;
        (*block).access_count = 0;
        
        // 3. Register in our "Neural Index"
        if self.count < 128 {
            self.blocks[self.count] = NonNull::new(block);
            self.count += 1;
        } else {
            // Eviction policy: Remove least accessed (simplified)
            // For demo, just overwrite last one
             self.blocks[127] = NonNull::new(block);
        }

        Some(IntentPtr {
            id: embedding.id,
            ptr: NonNull::new_unchecked(ptr.add(core::mem::size_of::<SemanticBlock>())),
        })
    }

    /// Retrieve memory by semantic query
    pub unsafe fn retrieve(&self, query: &Embedding) -> Option<IntentPtr> {
        let mut best_match = None;
        let mut best_score = 0;

        for i in 0..self.count {
            if let Some(block_ptr) = self.blocks[i] {
                let block = block_ptr.as_ref();
                let score = query.similarity(&block.embedding);
                
                if score > best_score {
                    best_score = score;
                    best_match = Some(block_ptr);
                }
            }
        }

        if best_score > 70 { // Threshold
             best_match.map(|block_ptr| {
                 let data_ptr = (block_ptr.as_ptr() as *mut u8).add(core::mem::size_of::<SemanticBlock>());
                 IntentPtr {
                     id: block_ptr.as_ref().embedding.id,
                     ptr: NonNull::new_unchecked(data_ptr),
                 }
             })
        } else {
            None
        }
    }
}
