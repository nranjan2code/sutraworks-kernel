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
    // Data follows immediately after this struct
}

/// Content-Addressable Pointer
#[derive(Clone, Copy, Debug)]
pub struct IntentPtr {
    pub id: ConceptID,
    pub ptr: NonNull<u8>,
    pub size: usize,
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

    /// Allocate memory with a concept ID tag
    pub unsafe fn alloc(&mut self, size: usize, concept_id: ConceptID) -> Option<IntentPtr> {
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
        })
    }

    /// Retrieve memory by concept ID
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
                    });
                }
            }
        }
        None
    }
    
    /// Get count of allocated blocks
    pub fn count(&self) -> usize {
        self.count
    }
}
