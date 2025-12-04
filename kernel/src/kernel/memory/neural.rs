//! Neural Memory Allocator (HDC Edition)
//!
//! Hyperdimensional Computing (HDC) memory for stroke-native kernel.
//! Uses 1024-bit binary hypervectors for holographic, robust, and efficient
//! semantic representation.
//!
//! "The brain doesn't do floating point math."

use core::ptr::NonNull;
// use alloc::alloc::alloc;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use crate::intent::ConceptID;
use crate::arch::SpinLock;
use super::hnsw::HnswIndex;
use super::matrix::Matrix;

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

/// Page Header for Semantic Memory
#[repr(C)]
struct SemanticPage {
    next: Option<NonNull<SemanticPage>>,
    used: usize,
    // Data follows. We use the rest of the 4KB page.
}

/// Neural Allocator (Dynamic Page-Based)
pub struct NeuralAllocator {
    head_page: Option<NonNull<SemanticPage>>,
    current_page: Option<NonNull<SemanticPage>>,
    total_items: usize,
    index: BTreeMap<ConceptID, IntentPtr>,
    hnsw: HnswIndex,
    projection_matrix: Option<Matrix>,
}

// SAFETY: NeuralAllocator is protected by SpinLock.
unsafe impl Send for NeuralAllocator {}

impl NeuralAllocator {
    pub const fn new() -> Self {
        NeuralAllocator {
            head_page: None,
            current_page: None,
            total_items: 0,
            index: BTreeMap::new(),
            hnsw: HnswIndex::new(),
            projection_matrix: None, // Will be initialized on first use
        }
    }

    /// Ensure projection matrix is initialized
    fn ensure_matrix_init(&mut self) {
        if self.projection_matrix.is_none() {
            self.projection_matrix = Some(Matrix::new_random(0xCAFEBABE));
        }
    }

    /// Project feature vector to Hypervector
    pub fn project(&mut self, features: &[f32]) -> Hypervector {
        self.ensure_matrix_init();
        self.projection_matrix.as_ref().unwrap().project(features)
    }

    /// Allocate memory with a concept ID tag and semantic hypervector
    pub unsafe fn alloc(&mut self, size: usize, concept_id: ConceptID, hv: Hypervector) -> Option<IntentPtr> {
        let block_size = core::mem::size_of::<SemanticBlock>();
        let total_needed = block_size + size;
        let align_padding = (16 - (total_needed % 16)) % 16;
        let alloc_size = total_needed + align_padding;
        
        // Ensure we have a current page with enough space
        if self.current_page.is_none() || !self.has_space(alloc_size) {
            self.grow_heap()?;
        }
        
        let page_ptr = self.current_page.unwrap();
        let page = page_ptr.as_ptr();
        
        // Calculate placement
        let offset = (*page).used;
        let start_ptr = (page as *mut u8).add(core::mem::size_of::<SemanticPage>() + offset);
        
        // Initialize Block
        let block = start_ptr as *mut SemanticBlock;
        (*block).concept_id = concept_id;
        (*block).access_count = 0;
        (*block).size = size;
        (*block).hypervector = hv;
        
        // Update Page State
        (*page).used += alloc_size;
        self.total_items += 1;
        
        let ptr = IntentPtr {
            id: concept_id,
            ptr: NonNull::new_unchecked(start_ptr.add(block_size)),
            size,
            similarity: 1.0,
        };

        // Update Index (O(log N))
        self.index.insert(concept_id, ptr);
        
        // Update HNSW Index
        self.hnsw.insert(concept_id, hv);
        
        Some(ptr)
    }
    
    /// Check if current page has space
    unsafe fn has_space(&self, size: usize) -> bool {
        if let Some(page_ptr) = self.current_page {
            let page = page_ptr.as_ref();
            let available = 4096 - core::mem::size_of::<SemanticPage>() - page.used;
            available >= size
        } else {
            false
        }
    }
    
    /// Allocate a new page and link it
    unsafe fn grow_heap(&mut self) -> Option<()> {
        // Allocate 1 page (4KB)
        let ptr = crate::kernel::memory::alloc_pages(1)?;
        let page = ptr.as_ptr() as *mut SemanticPage;
        
        // Initialize Header
        (*page).next = None;
        (*page).used = 0;
        
        // Link
        if let Some(curr) = self.current_page {
            (*curr.as_ptr()).next = Some(ptr.cast());
        } else {
            self.head_page = Some(ptr.cast());
        }
        
        self.current_page = Some(ptr.cast());
        Some(())
    }

    /// Retrieve memory by concept ID (Exact Match)
    /// Uses BTreeMap Index for O(log N) lookup.
    pub unsafe fn retrieve(&self, concept_id: ConceptID) -> Option<IntentPtr> {
        self.index.get(&concept_id).copied()
    }


        

    
    /// Retrieve nearest memory block by semantic hypervector (Fuzzy Match)
    /// Uses HNSW Index for O(log N) lookup.
    pub unsafe fn retrieve_nearest(&self, query: &Hypervector) -> Option<IntentPtr> {
        // Search HNSW for k=1 nearest neighbor
        let results = self.hnsw.search(query, 1);
        
        if let Some((id, _sim)) = results.first() {
            // Retrieve the pointer from the exact index
            // Note: HNSW returns the ConceptID, we use that to look up the pointer
            self.retrieve(*id)
        } else {
            None
        }
    }
    
    /// Get count of allocated blocks
    pub fn count(&self) -> usize {
        self.total_items
    }

    /// Get all allocated nodes (for visualization)
    pub fn get_all_nodes(&self) -> Vec<IntentPtr> {
        self.index.values().copied().collect()
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
