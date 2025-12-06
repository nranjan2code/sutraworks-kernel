//! Advanced Memory Allocator for Intent Kernel
//!
//! A world-class bare-metal memory allocator with:
//! - Buddy allocator for large blocks
//! - Slab allocator for small, fixed-size objects
//! - DMA-safe allocations
//! - Memory regions (kernel, user, DMA, GPU)

use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};
use crate::kernel::sync::RawSpinLock;
use crate::kernel::scheduler::CoreStats;

pub mod paging;
pub mod vma;
pub mod neural;


// ...

/// Global storage for per-core statistics (4 cores max)
// Note: We use RawSpinLock for allocator to avoid recursion with LockRegistry
pub static CORE_STATS: [RawSpinLock<CoreStats>; 4] = [
    RawSpinLock::new(CoreStats { idle_cycles: 0, total_cycles: 0, queue_length: 0 }),
    RawSpinLock::new(CoreStats { idle_cycles: 0, total_cycles: 0, queue_length: 0 }),
    RawSpinLock::new(CoreStats { idle_cycles: 0, total_cycles: 0, queue_length: 0 }),
    RawSpinLock::new(CoreStats { idle_cycles: 0, total_cycles: 0, queue_length: 0 }),
];

// ...


// ...

// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(not(feature = "test_mocks"))]
extern "C" {
    static __heap_start: u8;
    static __heap_end: u8;
    static __dma_start: u8;
    static __dma_end: u8;
    static __gpu_start: u8;
    static __gpu_end: u8;
}

#[cfg(feature = "test_mocks")]
mod mocks {
    #[no_mangle]
    pub static __heap_start: u8 = 0;
    #[no_mangle]
    pub static __heap_end: u8 = 0;
    #[no_mangle]
    pub static __dma_start: u8 = 0;
    #[no_mangle]
    pub static __dma_end: u8 = 0;
    #[no_mangle]
    pub static __gpu_start: u8 = 0;
    #[no_mangle]
    pub static __gpu_end: u8 = 0;
}

#[cfg(feature = "test_mocks")]
use mocks::*;

/// Get heap region bounds
#[allow(unused_unsafe)]
pub fn heap_region() -> (usize, usize) {
    unsafe {
        let start = &__heap_start as *const u8 as usize;
        let end = &__heap_end as *const u8 as usize;
        (start, end)
    }
}

/// Get DMA region bounds
#[allow(unused_unsafe)]
pub fn dma_region() -> (usize, usize) {
    unsafe {
        let start = &__dma_start as *const u8 as usize;
        let end = &__dma_end as *const u8 as usize;
        (start, end)
    }
}

/// Get GPU shared memory region bounds
#[allow(unused_unsafe)]
pub fn gpu_region() -> (usize, usize) {
    unsafe {
        let start = &__gpu_start as *const u8 as usize;
        let end = &__gpu_end as *const u8 as usize;
        (start, end)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Minimum allocation size (16 bytes for alignment)
const MIN_BLOCK_SIZE: usize = 16;

/// Maximum buddy order (2^MAX_ORDER * MIN_BLOCK_SIZE = max block)
const MAX_ORDER: usize = 20;  // Up to 16MB blocks

/// Page size
pub const PAGE_SIZE: usize = 4096;

/// Slab sizes for small allocations
const SLAB_SIZES: [usize; 8] = [16, 32, 64, 128, 256, 512, 1024, 2048];

// ═══════════════════════════════════════════════════════════════════════════════
// BUDDY ALLOCATOR
// ═══════════════════════════════════════════════════════════════════════════════

/// Free list node (embedded in free blocks)
#[repr(C)]
struct FreeNode {
    next: Option<NonNull<FreeNode>>,
}

/// Buddy allocator for large allocations
pub struct BuddyAllocator {
    // Free lists for each order
    free_lists: [Option<NonNull<FreeNode>>; MAX_ORDER + 1],
    // Base address
    base: usize,
    // Total size
    size: usize,
    // Bitmap of free orders (bit N set if order N has free blocks)
    free_mask: u32,
    // Statistics
    allocated: AtomicUsize,
    total_allocations: AtomicUsize,
}

// SAFETY: BuddyAllocator manages its own memory and is designed to be used
// with external synchronization (like SpinLock). The raw pointers it holds
// are effectively owned by it.
unsafe impl Send for BuddyAllocator {}

impl BuddyAllocator {
    /// Create a new uninitialized buddy allocator
    pub const fn new() -> Self {
        BuddyAllocator {
            free_lists: [None; MAX_ORDER + 1],
            base: 0,
            size: 0,
            free_mask: 0,
            allocated: AtomicUsize::new(0),
            total_allocations: AtomicUsize::new(0),
        }
    }
    
    /// Initialize with a memory region
    pub unsafe fn init(&mut self, base: usize, size: usize) {
        self.base = base;
        self.size = size;
        
        // Clear free lists
        for i in 0..=MAX_ORDER {
            self.free_lists[i] = None;
        }
        self.free_mask = 0;
        
        // Add entire region as free blocks
        let mut addr = base;
        let mut remaining = size;
        
        // Add largest possible blocks
        while remaining >= MIN_BLOCK_SIZE {
            let order = self.max_order_for_size(remaining);
            let block_size = self.block_size(order);
            
            self.add_to_free_list(addr, order);
            
            addr += block_size;
            remaining -= block_size;
        }
    }
    
    /// Calculate block size for an order
    #[inline]
    fn block_size(&self, order: usize) -> usize {
        MIN_BLOCK_SIZE << order
    }
    
    /// Calculate order needed for a size
    fn order_for_size(&self, size: usize) -> usize {
        let size = size.max(MIN_BLOCK_SIZE);
        let mut order = 0;
        let mut block_size = MIN_BLOCK_SIZE;
        
        while block_size < size && order < MAX_ORDER {
            order += 1;
            block_size <<= 1;
        }
        
        order
    }
    
    /// Calculate maximum order that fits in size
    fn max_order_for_size(&self, size: usize) -> usize {
        let mut order = 0;
        
        while order < MAX_ORDER && self.block_size(order + 1) <= size {
            order += 1;
        }
        
        order
    }
    
    /// Add a block to free list
    unsafe fn add_to_free_list(&mut self, addr: usize, order: usize) {
        let node = addr as *mut FreeNode;
        (*node).next = self.free_lists[order];
        self.free_lists[order] = NonNull::new(node);
        self.free_mask |= 1 << order;
    }
    
    /// Remove a block from free list
    unsafe fn remove_from_free_list(&mut self, order: usize) -> Option<usize> {
        let node = self.free_lists[order]?;
        self.free_lists[order] = (*node.as_ptr()).next;
        if self.free_lists[order].is_none() {
            self.free_mask &= !(1 << order);
        }
        Some(node.as_ptr() as usize)
    }
    
    /// Get buddy address
    fn buddy_addr(&self, addr: usize, order: usize) -> usize {
        addr ^ self.block_size(order)
    }
    
    /// Allocate a block
    pub unsafe fn allocate(&mut self, size: usize) -> Option<NonNull<u8>> {
        // crate::kprintln!("[MEM] Buddy Allocate: {}", size);
        let order = self.order_for_size(size);
        
        // Find a free block using bitmap (O(1))
        // Mask out orders smaller than requested
        let search_mask = self.free_mask & !((1 << order) - 1);
        
        if search_mask == 0 {
            // No block found
            crate::kprintln!("[MEM] Buddy OOM");
            return None;
        }
        
        // Find smallest available order (trailing zeros)
        let mut current_order = search_mask.trailing_zeros() as usize;
        
        if current_order > MAX_ORDER {
            crate::kprintln!("[MEM] Buddy OOM");
            return None;  // Out of memory
        }
        
        // Remove block from free list
        let addr = self.remove_from_free_list(current_order)?;
        
        // Split down to required order
        while current_order > order {
            current_order -= 1;
            let buddy = addr + self.block_size(current_order);
            self.add_to_free_list(buddy, current_order);
        }
        
        // Update statistics
        self.allocated.fetch_add(self.block_size(order), Ordering::Relaxed);
        self.total_allocations.fetch_add(1, Ordering::Relaxed);
        
        NonNull::new(addr as *mut u8)
    }
    
    /// Deallocate a block
    pub unsafe fn deallocate(&mut self, ptr: NonNull<u8>, size: usize) {
        let order = self.order_for_size(size);
        let mut addr = ptr.as_ptr() as usize;
        let mut current_order = order;
        
        // Try to coalesce with buddy
        while current_order < MAX_ORDER {
            let buddy_addr = self.buddy_addr(addr, current_order);
            
            // Check if buddy is free (by searching free list)
            let mut prev: Option<NonNull<FreeNode>> = None;
            let mut curr = self.free_lists[current_order];
            let mut found = false;
            
            while let Some(node) = curr {
                if node.as_ptr() as usize == buddy_addr {
                    // Remove buddy from free list
                    if let Some(p) = prev {
                        (*p.as_ptr()).next = (*node.as_ptr()).next;
                    } else {
                        self.free_lists[current_order] = (*node.as_ptr()).next;
                    }
                    found = true;
                    break;
                }
                prev = curr;
                curr = (*node.as_ptr()).next;
            }
            
            if !found {
                break;
            }
            
            // Coalesce: use lower address
            addr = addr.min(buddy_addr);
            current_order += 1;
        }
        
        // Add to free list
        self.add_to_free_list(addr, current_order);
        
        // Update statistics
        self.allocated.fetch_sub(self.block_size(order), Ordering::Relaxed);
    }
    
    /// Get allocated bytes
    pub fn allocated(&self) -> usize {
        self.allocated.load(Ordering::Relaxed)
    }
    
    /// Get total size
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get total allocations count
    pub fn total_allocations(&self) -> usize {
        self.total_allocations.load(Ordering::Relaxed)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SLAB ALLOCATOR
// ═══════════════════════════════════════════════════════════════════════════════

/// Slab header
#[repr(C)]
struct SlabHeader {
    next_slab: Option<NonNull<SlabHeader>>,
    free_list: Option<NonNull<u8>>,
    allocated: usize,
    capacity: usize,
    object_size: usize,
}

/// Slab cache for fixed-size objects
pub struct SlabCache {
    slabs: [Option<NonNull<SlabHeader>>; 8],
    allocated_bytes: usize,
}

impl SlabCache {
    pub const fn new() -> Self {
        SlabCache {
            slabs: [None; 8],
            allocated_bytes: 0,
        }
    }
}

impl Default for SlabCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SlabCache {
    
    /// Get slab index for size (Optimized O(1))
    fn slab_index(&self, size: usize) -> Option<usize> {
        if size > 2048 { return None; }
        
        // Ensure size is at least minimum (16)
        let sz = size.max(16);
        
        // Calculate log2(ceil(sz))
        // (sz - 1).leading_zeros() gives LZ count.
        // 64 - LZ = bits required to represent sz-1.
        // Example: 16 -> 15 (0...01111) -> LZ=60. 64-60=4.
        // Example: 17 -> 16 (0...10000) -> LZ=59. 64-59=5.
        // Index mapping:
        // 16 (2^4) -> index 0. (4 - 4 = 0)
        // 32 (2^5) -> index 1. (5 - 4 = 1)
        // Formula: (64 - (sz - 1).leading_zeros()) - 4
        
        let lz = (sz - 1).leading_zeros();
        debug_assert!(lz <= 60); // min size 16 means max (15) has 60 leading zeros
        let power = 64 - lz;
        let index = (power - 4) as usize;
        
        Some(index)
    }
    
    /// Get total allocated bytes
    pub fn allocated(&self) -> usize {
        self.allocated_bytes
    }
    
    /// Allocate from slab cache
    pub unsafe fn allocate(&mut self, size: usize, buddy: &mut BuddyAllocator) -> Option<NonNull<u8>> {
        let index = self.slab_index(size)?;
        let object_size = SLAB_SIZES[index];
        
        // Try to get from existing slab
        if let Some(mut slab) = self.slabs[index] {
            let header = slab.as_mut();
            
            if let Some(obj) = header.free_list {
                // Get object from free list
                header.free_list = *(obj.as_ptr() as *mut Option<NonNull<u8>>);
                header.allocated += 1;
                self.allocated_bytes += object_size;
                // crate::kprintln!("[SLAB] Alloc size={} ptr={:p} (reuse)", object_size, obj.as_ptr());
                return Some(obj);
            }
        }
        
        // Need new slab - allocate a page
        let slab_mem = buddy.allocate(PAGE_SIZE)?;
        let header = slab_mem.as_ptr() as *mut SlabHeader;
        
        // Initialize slab header
        let data_start = slab_mem.as_ptr() as usize + core::mem::size_of::<SlabHeader>();
        let data_start = (data_start + object_size - 1) & !(object_size - 1);  // Align
        let capacity = (slab_mem.as_ptr() as usize + PAGE_SIZE - data_start) / object_size;
        
        (*header).next_slab = self.slabs[index];
        (*header).free_list = None;
        (*header).allocated = 1;
        (*header).capacity = capacity;
        (*header).object_size = object_size;
        
        
        (*header).free_list = None;
        
        // Build free list (from end to beginning, so first allocation gets first object)
        let mut next_ptr: Option<NonNull<u8>> = None;
        for i in (1..capacity).rev() {
            let obj_addr = (data_start + i * object_size) as *mut u8;
            *(obj_addr as *mut Option<NonNull<u8>>) = next_ptr;
            next_ptr = NonNull::new(obj_addr);
        }
        (*header).free_list = next_ptr;
        
        // Link slab
        self.slabs[index] = NonNull::new(header);
        
        self.allocated_bytes += object_size;
        NonNull::new(data_start as *mut u8)
    }
    
    /// Deallocate to slab cache
    pub unsafe fn deallocate(&mut self, ptr: NonNull<u8>, _size: usize) {
        // Find the slab header by masking the pointer (slabs are page-aligned)
        let page_start = (ptr.as_ptr() as usize) & !(PAGE_SIZE - 1);
        let header = page_start as *mut SlabHeader;
        
        // crate::kprintln!("[SLAB] Dealloc ptr={:p}, header={:p}", ptr.as_ptr(), header);
        
        self.allocated_bytes -= (*header).object_size;
        
        // Free the object (add to free list)
        *(ptr.as_ptr() as *mut Option<NonNull<u8>>) = (*header).free_list;
        (*header).free_list = Some(ptr);
        
        // Decrement allocated count
        // Note: We trust the slab header is valid and allocated > 0
        if (*header).allocated > 0 {
            (*header).allocated -= 1;
        } else {
            crate::kprintln!("[MEM] Double free or corruption in slab dealloc: ptr={:p}, header={:p}", ptr.as_ptr(), header);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GLOBAL ALLOCATOR
// ═══════════════════════════════════════════════════════════════════════════════

/// Kernel heap allocator inner state
struct AllocatorInner {
    buddy: BuddyAllocator,
    slab: SlabCache,
    initialized: bool,
}

// SAFETY: AllocatorInner is protected by SpinLock, and we ensure single-threaded access
// to the inner state via the lock. The raw pointers in SlabCache/BuddyAllocator are
// managed safely.
unsafe impl Send for AllocatorInner {}

pub struct KernelAllocator {
    inner: RawSpinLock<AllocatorInner>,
}

impl KernelAllocator {
    pub const fn new() -> Self {
        KernelAllocator {
            inner: RawSpinLock::new(AllocatorInner {
                buddy: BuddyAllocator::new(),
                slab: SlabCache::new(),
                initialized: false,
            }),
        }
    }
}

impl Default for KernelAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl KernelAllocator {
    
    /// Initialize the allocator with ASLR seed
    pub fn init(&self, seed: u64) {
        let mut inner = self.inner.lock();
        
        let (heap_start, heap_end) = heap_region();
        let total_size = heap_end - heap_start;
        
        // Polymorphic Heap: Randomize start address
        // Use up to 25% of heap for offset
        let max_offset = total_size / 4;
        // Ensure alignment to 4K page
        let offset = (seed as usize % max_offset) & !(PAGE_SIZE - 1);
        
        let randomized_start = heap_start + offset;
        let randomized_size = total_size - offset;
        
        // crate::kprintln!("[MEM] Polymorphic Heap: Base={:#x} (Offset={:#x})", randomized_start, offset);
        
        unsafe {
            inner.buddy.init(randomized_start, randomized_size);
        }
        inner.initialized = true;
        crate::kprintln!("[MEM] Memory Init Done (Lock held)");
    }
    
    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.inner.lock().initialized
    }
    
    /// Get statistics
    pub fn stats(&self) -> AllocatorStats {
        let inner = self.inner.lock();
        if !inner.initialized {
            return AllocatorStats { allocated: 0, slab_allocated: 0, total_allocations: 0 };
        }
        AllocatorStats {
            allocated: inner.buddy.allocated(),
            slab_allocated: inner.slab.allocated(),
            total_allocations: 0,
        }
    }
}

/// Allocator statistics
pub struct AllocatorStats {
    pub allocated: usize,
    pub slab_allocated: usize,
    pub total_allocations: usize,
}

/// Initialize the global allocator
/// 
/// # Safety
/// Must be called once before any allocations
pub unsafe fn init(seed: u64) {
    GLOBAL.init(seed);
    paging::init();
}

/// Get allocator statistics
pub fn stats() -> AllocatorStats {
    GLOBAL.stats()
}

/// Get available heap memory
pub fn heap_available() -> usize {
    let (start, end) = heap_region();
    let stats = GLOBAL.stats();
    let size = if end >= start { end - start } else { 0 };
    size.saturating_sub(stats.allocated)
}

/// Allocate memory and return a Capability (Forward-Looking API)
pub fn alloc_cap(size: usize) -> Option<crate::kernel::capability::Capability> {
    use crate::kernel::capability::{CapabilityType, Permissions, mint_root};
    use core::alloc::Layout;
    
    // Allocate raw memory
    let layout = Layout::from_size_align(size, 16).ok()?;
    let ptr = unsafe { GLOBAL.alloc(layout) };
    
    if ptr.is_null() {
        return None;
    }
    
    // Mint a new root capability for this memory block
    unsafe {
        mint_root(
            CapabilityType::Memory,
            ptr as u64,
            size as u64,
            Permissions::ALL
        )
    }
}

/// Force memory compaction (Recovery Action)
pub fn force_compact() {
    // The current Slab/Buddy allocator combination is non-moving, so we cannot compact.
    // However, we could try to release empty slabs back to the buddy allocator.
    // For this implementation, we log the attempt.
    crate::kprintln!("[MEM] Force compaction triggered - Allocator is non-moving (No-op)");
}

// Global allocator implementation
unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut inner = self.inner.lock();
        
        if !inner.initialized {
            return core::ptr::null_mut();
        }
        
        let size = layout.size().max(layout.align());
        
        let inner_ref = &mut *inner;
        
        if size <= SLAB_SIZES[SLAB_SIZES.len() - 1] {
            if let Some(ptr) = inner_ref.slab.allocate(size, &mut inner_ref.buddy) {
                return ptr.as_ptr();
            } else {
                // CRITICAL: Do NOT fall back to buddy allocator for small sizes.
                // dealloc() uses size to decide between slab and buddy.
                // If we allocate from buddy but dealloc via slab, we corrupt memory
                // because slab expects a SlabHeader at the page start.
                return core::ptr::null_mut();
            }
        }
        
        // Use buddy for larger allocations
        if let Some(ptr) = inner_ref.buddy.allocate(size) {
            ptr.as_ptr()
        } else {
            core::ptr::null_mut()
        }
    }
    
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut inner = self.inner.lock();
        
        if ptr.is_null() || !inner.initialized {
            return;
        }
        
        let size = layout.size().max(layout.align());
        
        if let Some(ptr) = NonNull::new(ptr) {
            if size <= SLAB_SIZES[SLAB_SIZES.len() - 1] {
                inner.slab.deallocate(ptr, size);
            } else {
                inner.buddy.deallocate(ptr, size);
            }
        }
    }
}

// Global allocator - only active in kernel mode, not during host tests
// Tests use the standard library allocator which works on the host
#[cfg(not(test))]
#[global_allocator]
static GLOBAL: KernelAllocator = KernelAllocator::new();

// For non-test contexts, expose GLOBAL
#[cfg(not(test))]
pub fn global_allocator() -> &'static KernelAllocator {
    &GLOBAL
}

// For test contexts, provide a mock that does nothing
#[cfg(test)]
static GLOBAL: KernelAllocator = KernelAllocator::new();

// ═══════════════════════════════════════════════════════════════════════════════
// DMA ALLOCATOR
// ═══════════════════════════════════════════════════════════════════════════════

static DMA_ALLOCATOR: RawSpinLock<BuddyAllocator> = RawSpinLock::new(BuddyAllocator::new());

/// Initialize DMA allocator
pub unsafe fn init_dma() {
    let mut allocator = DMA_ALLOCATOR.lock();
    let (start, end) = dma_region();
    allocator.init(start, end - start);
}

/// Allocate DMA-safe memory
pub unsafe fn alloc_dma(size: usize) -> Option<NonNull<u8>> {
    let mut allocator = DMA_ALLOCATOR.lock();
    allocator.allocate(size)
}

/// Free DMA memory
pub unsafe fn free_dma(ptr: NonNull<u8>, size: usize) {
    let mut allocator = DMA_ALLOCATOR.lock();
    allocator.deallocate(ptr, size);
}

// ═══════════════════════════════════════════════════════════════════════════════
// PAGE ALLOCATOR
// ═══════════════════════════════════════════════════════════════════════════════

/// Allocate pages
pub unsafe fn alloc_pages(count: usize) -> Option<NonNull<u8>> {
    #[cfg(feature = "test_mocks")]
    {
        use alloc::alloc::{alloc, Layout};
        let size = count * PAGE_SIZE;
        let layout = Layout::from_size_align(size, PAGE_SIZE).ok()?;
        let ptr = alloc(layout);
        if ptr.is_null() {
            None
        } else {
            NonNull::new(ptr)
        }
    }
    #[cfg(not(feature = "test_mocks"))]
    {
        // Verbose logging removed - was printing on every page allocation
        // which added significant overhead during high-allocation scenarios
        let mut inner = GLOBAL.inner.lock();
        inner.buddy.allocate(count * PAGE_SIZE)
    }
}

/// Allocate pages for user space
pub unsafe fn alloc_user_pages(count: usize) -> Option<NonNull<u8>> {
    // Future: Account to process or use separate pool
    alloc_pages(count)
}

/// Free pages
pub unsafe fn free_pages(ptr: NonNull<u8>, count: usize) {
    let mut inner = GLOBAL.inner.lock();
    inner.buddy.deallocate(ptr, count * PAGE_SIZE);
}

// ═══════════════════════════════════════════════════════════════════════════════
// STACK ALLOCATOR (VMM Backed with Guard Pages)
// ═══════════════════════════════════════════════════════════════════════════════

/// A VMM-backed Stack
pub struct Stack {
    pub top: u64,
    pub bottom: u64,
    pub size: usize,
    ptr: NonNull<u8>, // Pointer to the allocation (including guard page)
}

// SAFETY: Stack owns the memory
unsafe impl Send for Stack {}

impl Stack {
    /// Create a new stack from raw parts
    pub unsafe fn from_raw(top: u64, bottom: u64, size: usize, ptr: NonNull<u8>) -> Self {
        Stack { top, bottom, size, ptr }
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        unsafe {
            // 1. Re-map the Guard Page (the first page)
            // We need to lock the Kernel VMM to restore the mapping so the allocator can write to it.
            if let Some(vmm) = self::paging::KERNEL_VMM.lock().as_mut() {
                let start_addr = self.ptr.as_ptr() as u64;
                // Identity map it back to Normal Memory
                // Flags: Normal, RW, EL1, Inner Shareable
                let flags = paging::EntryFlags::ATTR_NORMAL | paging::EntryFlags::AP_RW_EL1 | paging::EntryFlags::SH_INNER;
                
                if let Err(e) = vmm.map_page(start_addr, start_addr, flags) {
                    crate::kprintln!("[MEM] Failed to re-map stack guard page during drop: {}", e);
                } else {
                    crate::kprintln!("[MEM] Restored stack guard page at {:#x}", start_addr);
                }
            }
            
            // 2. Free the pages
            free_pages(self.ptr, (self.size / PAGE_SIZE) + 1);
        }
    }
}

/// Allocate a VMM-backed stack with a Guard Page
/// 
/// Allocates `size_in_pages + 1` pages.
/// The first page (lowest address) is the Guard Page.
/// We unmap it in the Kernel VMM so any access triggers a Data Abort.
pub fn alloc_stack(size_in_pages: usize) -> Option<Stack> {
    unsafe {
        // 1. Allocate pages (Size + 1 for Guard)
        let total_pages = size_in_pages + 1;
        let ptr = alloc_pages(total_pages)?;
        let start_addr = ptr.as_ptr() as u64;
        
        // 2. Unmap the Guard Page (the first page)
        // We need to lock the Kernel VMM
        if let Some(vmm) = self::paging::KERNEL_VMM.lock().as_mut() {
            // Unmap the bottom page
            if let Err(e) = paging::VMM::unmap_page(vmm, start_addr) {
                crate::kprintln!("[MEM] Failed to unmap stack guard page: {}", e);
                // Continue anyway? Or fail?
                // If we fail to unmap, we just don't have a guard page.
            } else {
                // crate::kprintln!("[MEM] Stack Guard Page active at {:#x}", start_addr);
            }
        }
        
        // 3. Calculate Stack Bounds
        // Stack grows down from top.
        // Bottom is start_addr + PAGE_SIZE (Guard is at start_addr)
        let bottom = start_addr + PAGE_SIZE as u64;
        let size_bytes = size_in_pages * PAGE_SIZE;
        let top = bottom + size_bytes as u64;
        
        Some(Stack {
            top,
            bottom,
            size: size_bytes,
            ptr,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECURITY & VALIDATION
// ═══════════════════════════════════════════════════════════════════════════════

/// Check if an address is in user space (EL0)
/// 
/// In AArch64, user space is typically the lower half of the address space (TTBR0).
/// We assume user space is 0x0000_0000_0000_0000 to 0x0000_FFFF_FFFF_FFFF.
/// Kernel space is 0xFFFF_0000_0000_0000 to 0xFFFF_FFFF_FFFF_FFFF.
/// 
/// However, for our simple OS, we might have a simpler map.
/// Let's assume anything below 0x8000_0000_0000 is potentially user.
#[inline]
pub fn is_user_addr(addr: u64) -> bool {
    // Check if bit 63 is 0 (Lower half)
    (addr & (1 << 63)) == 0
}

/// Validate a user pointer for reading
/// 
/// Checks:
/// 1. Address is in user space
/// 2. Address + len is in user space (no overflow)
/// 3. Address is not null (optional, but good practice)
pub fn validate_read_ptr(ptr: *const u8, len: usize) -> Result<(), &'static str> {
    let start = ptr as u64;
    let len_u64 = len as u64;
    
    // Check null
    if start == 0 {
        return Err("Null pointer");
    }
    
    // Check user space
    if !is_user_addr(start) {
        return Err("Pointer not in user space");
    }
    
    // Check overflow and end address
    if let Some(end) = start.checked_add(len_u64) {
        if !is_user_addr(end - 1) { // -1 because end is exclusive
            return Err("Buffer spans into kernel space");
        }
    } else {
        return Err("Pointer overflow");
    }
    
    // Check against process memory map (VMA)
    // We need to access the current agent.
    // This requires locking the scheduler, which might be recursive if we are already holding it?
    // validate_read_ptr is often called from syscalls where we might hold the scheduler lock.
    // BUT, we can't easily get the current agent here without passing it in.
    // Refactoring all call sites is a big task.
    // Alternative: Use a thread-local-like access or just check VMM if available?
    // The VMM check is already done in sys_read/sys_write/sys_print inside syscall.rs.
    // This function is a fallback or lower-level check.
    
    // Ideally, we should move VMA checks to a higher level or pass the context.
    // For this Sprint, let's keep this as a basic sanity check (User Space range)
    // and rely on the syscall layer (which we updated) to do the VMA check.
    
    // Wait, I updated syscall.rs to use VMA checks in sys_read/sys_write/sys_print.
    // But sys_sigaction still uses this?
    // In sys_sigaction, I added:
    // if let Some(vmm) = &agent.vmm { if !vmm.is_mapped(...) ... }
    
    // So actually, this function can remain as a "Basic User Pointer Validation"
    // that just checks the address range. The VMA check is done by the caller (syscall handler)
    // who has access to the Agent.
    
    Ok(())
}

/// Validate a user pointer for writing
///
/// Checks:
/// 1. Address is in user space
/// 2. Address + len is in user space (no overflow)
/// 3. Address is not null
pub fn validate_write_ptr(ptr: *mut u8, len: usize) -> Result<(), &'static str> {
    let start = ptr as u64;
    let len_u64 = len as u64;
    
    if start == 0 {
        return Err("Null pointer");
    }
    
    if !is_user_addr(start) {
        return Err("Pointer not in user space");
    }
    
    if let Some(end) = start.checked_add(len_u64) {
        if !is_user_addr(end - 1) {
            return Err("Buffer spans into kernel space");
        }
    } else {
        return Err("Pointer overflow");
    }
    
    Ok(())
}

/// Validate a user string (UTF-8)
pub unsafe fn validate_user_str(ptr: *const u8, len: usize) -> Result<&'static str, &'static str> {
    validate_read_ptr(ptr, len)?;
    
    // SAFETY: We validated the pointer range is in user space.
    // We trust the user to provide valid memory there (if mapped).
    // If not mapped, it will trigger a Data Abort (handled by exception handler).
    // The slice creation itself is "safe" in terms of Rust types, but accessing it is the danger.
    let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
    
    core::str::from_utf8(slice).map_err(|_| "Invalid UTF-8")
}

impl Default for BuddyAllocator { fn default() -> Self { Self::new() } }
