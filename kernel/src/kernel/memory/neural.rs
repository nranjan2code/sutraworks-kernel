//! Neural Memory Allocator (ConceptID Edition)
//!
//! Semantic memory for the Intent Kernel.
//! Maps unique ConceptIDs directly to memory blocks.
//! No hypervectors. No approximate matching. Pure semantic addressing.

use core::ptr::NonNull;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use crate::intent::ConceptID;
use crate::kernel::sync::SpinLock;

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
        }
    }

    /// Allocate memory with a concept ID tag
    pub unsafe fn alloc(&mut self, size: usize, concept_id: ConceptID) -> Option<IntentPtr> {
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
        
        // Update Page State
        (*page).used += alloc_size;
        self.total_items += 1;
        
        let ptr = IntentPtr {
            id: concept_id,
            ptr: NonNull::new_unchecked(start_ptr.add(block_size)),
            size,
        };

        // Update Index (O(log N))
        self.index.insert(concept_id, ptr);
        
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
        #[cfg(not(feature = "test_mocks"))]
        let ptr = crate::kernel::memory::alloc_pages(1)?;

        #[cfg(feature = "test_mocks")]
        let ptr = {
            use alloc::alloc::{alloc, Layout};
            let layout = Layout::from_size_align(4096, 4096).ok()?;
            let raw = alloc(layout);
            if raw.is_null() { return None; }
            NonNull::new(raw)?
        };
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
    
    /// Get count of allocated blocks
    pub fn count(&self) -> usize {
        self.total_items
    }

    /// Get all allocated nodes (for visualization)
    pub fn get_all_nodes(&self) -> Vec<IntentPtr> {
        self.index.values().copied().collect()
    }
}


impl Default for NeuralAllocator { fn default() -> Self { Self::new() } }
