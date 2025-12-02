//! Neural Memory Allocator (HDC Edition)
//!
//! Hyperdimensional Computing (HDC) memory for stroke-native kernel.
//! Uses 1024-bit binary hypervectors for holographic, robust, and efficient
//! semantic representation.
//!
//! "The brain doesn't do floating point math."

use core::ptr::NonNull;
use alloc::alloc::{alloc, Layout};
use crate::intent::ConceptID;
use crate::arch::SpinLock;

/// Global Neural Allocator instance
pub static NEURAL_ALLOCATOR: SpinLock<NeuralAllocator> = SpinLock::new(NeuralAllocator::new());

/// 1024-bit Hypervector (16 x 64-bit integers)
pub type Hypervector = [u64; 16];

/// A Semantic Block of memory
#[repr(C)]
pub struct SemanticBlock {
    pub concept_id: ConceptID,
    pub access_count: u64,
    pub size: usize,
    pub hypervector: Hypervector, // 1024-bit semantic signature
    // Data follows immediately after this struct
}

/// Content-Addressable Pointer
#[derive(Clone, Copy, Debug)]
pub struct IntentPtr {
    pub id: ConceptID,
    pub ptr: NonNull<u8>,
    pub size: usize,
    pub similarity: f32, // 0.0 to 1.0 (Normalized Hamming Similarity)
}

/// Neural Allocator
pub struct NeuralAllocator {
    blocks: [Option<NonNull<SemanticBlock>>; 128],
    count: usize,
}

// SAFETY: NeuralAllocator is protected by SpinLock.
unsafe impl Send for NeuralAllocator {}

impl NeuralAllocator {
    pub const fn new() -> Self {
        NeuralAllocator {
            blocks: [None; 128],
            count: 0,
        }
    }

    /// Allocate memory with a concept ID tag and semantic hypervector
    pub unsafe fn alloc(&mut self, size: usize, concept_id: ConceptID, hv: Hypervector) -> Option<IntentPtr> {
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
        (*block).hypervector = hv;
        
        // 3. Register in our index
        if self.count < 128 {
            self.blocks[self.count] = NonNull::new(block);
            self.count += 1;
        } else {
            // Eviction: overwrite last one (primitive policy for now)
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
    
    /// Retrieve nearest memory block by semantic hypervector (Fuzzy Match)
    /// Uses Hamming Similarity.
    pub unsafe fn retrieve_nearest(&self, query: &Hypervector) -> Option<IntentPtr> {
        let mut best_match: Option<IntentPtr> = None;
        let mut best_sim = -1.0;
        
        for i in 0..self.count {
            if let Some(block_ptr) = self.blocks[i] {
                let block = block_ptr.as_ref();
                let sim = hamming_similarity(query, &block.hypervector);
                
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

// --- HDC Primitives ---

/// Calculate Normalized Hamming Similarity
/// Returns 1.0 for identical vectors, 0.0 for complements, 0.5 for orthogonal.
/// Sim(A, B) = 1 - (HammingDist(A, B) / TotalBits)
pub fn hamming_similarity(a: &Hypervector, b: &Hypervector) -> f32 {
    let mut diff_bits: u32 = 0;
    for i in 0..16 {
        diff_bits += (a[i] ^ b[i]).count_ones();
    }
    
    1.0 - (diff_bits as f32 / 1024.0)
}

/// Bind two hypervectors using XOR
/// This is invertible: Bind(A, B) ^ B = A
/// Preserves distance to neither A nor B (creates a new orthogonal concept).
pub fn bind(a: &Hypervector, b: &Hypervector) -> Hypervector {
    let mut result = [0u64; 16];
    for i in 0..16 {
        result[i] = a[i] ^ b[i];
    }
    result
}

/// Bundle (Superposition) of two hypervectors
/// Uses bitwise majority rule.
/// Since we only have 2 vectors, we break ties randomly (or deterministically here).
/// For 2 vectors, A + B is tricky in binary. Standard way is A + B + Random.
/// Or just OR them (sparse) or AND them.
/// But for dense binary HDC, we usually need odd number of vectors for majority.
/// Simplified "Bundle" for 2 vectors: Randomly take bit from A or B.
/// Deterministic approximation: (A & B) | (A & Mask) | (B & !Mask)
/// Let's use a simple OR for now if we assume sparsity, but standard HDC uses Majority.
/// Let's implement a proper bitwise majority for 3 inputs, and for 2 inputs we just use OR?
/// No, OR saturates.
/// Let's use the "Swap" trick or just XOR for binding.
/// Actually, for bundling 2 vectors in binary, we can just do bitwise OR if we treat 1s as features.
/// But standard dense HDC (MAP) uses majority.
/// Let's implement `bundle_majority` taking 3 vectors.
pub fn bundle_majority(a: &Hypervector, b: &Hypervector, c: &Hypervector) -> Hypervector {
    let mut result = [0u64; 16];
    for i in 0..16 {
        // Majority(a, b, c) = (a & b) | (b & c) | (c & a)
        result[i] = (a[i] & b[i]) | (b[i] & c[i]) | (c[i] & a[i]);
    }
    result
}

/// Permute (Cyclic Shift)
/// Rotates the entire 1024-bit vector by 1 position.
/// Used to encode sequence/order.
pub fn permute(a: &Hypervector) -> Hypervector {
    let mut result = [0u64; 16];
    // We need to carry bits between u64 words
    let mut carry = (a[15] >> 63) & 1;
    
    for i in 0..16 {
        let new_carry = (a[i] >> 63) & 1;
        result[i] = (a[i] << 1) | carry;
        carry = new_carry;
    }
    result
}
