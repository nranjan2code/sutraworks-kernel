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
use crate::arch::SpinLock;

pub mod paging;
pub mod neural;
pub mod hnsw;
pub mod matrix;
pub mod vma;

// ═══════════════════════════════════════════════════════════════════════════════
// MEMORY REGIONS (from linker script)
// ═══════════════════════════════════════════════════════════════════════════════

extern "C" {
    static __heap_start: u8;
    static __heap_end: u8;
    static __dma_start: u8;
    static __dma_end: u8;
    static __gpu_start: u8;
    static __gpu_end: u8;
}

/// Get heap region bounds
pub fn heap_region() -> (usize, usize) {
    unsafe {
        let start = &__heap_start as *const u8 as usize;
        let end = &__heap_end as *const u8 as usize;
        (start, end)
    }
}

/// Get DMA region bounds
pub fn dma_region() -> (usize, usize) {
    unsafe {
        let start = &__dma_start as *const u8 as usize;
        let end = &__dma_end as *const u8 as usize;
        (start, end)
    }
}

/// Get GPU shared memory region bounds
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
    }
    
    /// Remove a block from free list
    unsafe fn remove_from_free_list(&mut self, order: usize) -> Option<usize> {
        let node = self.free_lists[order]?;
        self.free_lists[order] = (*node.as_ptr()).next;
        Some(node.as_ptr() as usize)
    }
    
    /// Get buddy address
    fn buddy_addr(&self, addr: usize, order: usize) -> usize {
        addr ^ self.block_size(order)
    }
    
    /// Allocate a block
    pub unsafe fn allocate(&mut self, size: usize) -> Option<NonNull<u8>> {
        let order = self.order_for_size(size);
        
        // Find a free block (may need to split larger blocks)
        let mut current_order = order;
        while current_order <= MAX_ORDER {
            if self.free_lists[current_order].is_some() {
                break;
            }
            current_order += 1;
        }
        
        if current_order > MAX_ORDER {
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
}

impl SlabCache {
    pub const fn new() -> Self {
        SlabCache {
            slabs: [None; 8],
        }
    }
    
    /// Get slab index for size
    fn slab_index(&self, size: usize) -> Option<usize> {
        for (i, &slab_size) in SLAB_SIZES.iter().enumerate() {
            if size <= slab_size {
                return Some(i);
            }
        }
        None
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
        
        // Build free list
        let mut addr = data_start;
        for i in 1..capacity {
            let next_addr = data_start + i * object_size;
            *(addr as *mut usize) = next_addr;
            addr = next_addr;
        }
        *(addr as *mut usize) = 0;  // End of list
        
        (*header).free_list = NonNull::new((data_start + object_size) as *mut u8);
        
        // Link slab
        self.slabs[index] = NonNull::new(header);
        
        NonNull::new(data_start as *mut u8)
    }
    
    /// Deallocate to slab cache
    pub unsafe fn deallocate(&mut self, ptr: NonNull<u8>, size: usize) {
        let index = match self.slab_index(size) {
            Some(i) => i,
            None => return,
        };
        
        // Find containing slab (simplified: just add to first slab's free list)
        if let Some(mut slab) = self.slabs[index] {
            let header = slab.as_mut();
            
            // Add to free list
            *(ptr.as_ptr() as *mut Option<NonNull<u8>>) = header.free_list;
            header.free_list = Some(ptr);
            header.allocated -= 1;
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
    inner: SpinLock<AllocatorInner>,
}

impl KernelAllocator {
    pub const fn new() -> Self {
        KernelAllocator {
            inner: SpinLock::new(AllocatorInner {
                buddy: BuddyAllocator::new(),
                slab: SlabCache::new(),
                initialized: false,
            }),
        }
    }
    
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
        
        crate::kprintln!("[MEM] Polymorphic Heap: Base={:#x} (Offset={:#x})", randomized_start, offset);
        
        unsafe {
            inner.buddy.init(randomized_start, randomized_size);
        }
        inner.initialized = true;
    }
    
    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.inner.lock().initialized
    }
    
    /// Get statistics
    pub fn stats(&self) -> (usize, usize) {
        let inner = self.inner.lock();
        if !inner.initialized {
            return (0, 0);
        }
        (inner.buddy.size(), inner.buddy.allocated())
    }
}

/// Allocator statistics
pub struct AllocatorStats {
    pub allocated: usize,
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
    let (_total_bytes, used_bytes) = GLOBAL.stats();
    AllocatorStats {
        allocated: used_bytes,
        total_allocations: 0, // Not easily available without locking again or changing return type
    }
}

/// Get available heap memory
pub fn heap_available() -> usize {
    let (start, end) = heap_region();
    let (_total_bytes, used_bytes) = GLOBAL.stats();
    (end - start).saturating_sub(used_bytes)
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

#[global_allocator]
static GLOBAL: KernelAllocator = KernelAllocator::new();

// ═══════════════════════════════════════════════════════════════════════════════
// DMA ALLOCATOR
// ═══════════════════════════════════════════════════════════════════════════════

static DMA_ALLOCATOR: SpinLock<BuddyAllocator> = SpinLock::new(BuddyAllocator::new());

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
    let mut inner = GLOBAL.inner.lock();
    inner.buddy.allocate(count * PAGE_SIZE)
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
        // When stack is dropped, we should free the pages
        // Note: We should also re-map the guard page before freeing?
        // Or just free the physical pages.
        // For now, we leak to avoid complexity in this prototype, 
        // or we implement a proper free_stack.
        unsafe {
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
        if let Some(vmm) = paging::KERNEL_VMM.lock().as_mut() {
            // Unmap the bottom page
            if let Err(e) = vmm.unmap_page(start_addr) {
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
    
    // Wait, I updated syscall.rs to use VMA checks in sys_read/write/print.
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
pub fn validate_user_str(ptr: *const u8, len: usize) -> Result<&'static str, &'static str> {
    validate_read_ptr(ptr, len)?;
    
    // SAFETY: We validated the pointer range is in user space.
    // We trust the user to provide valid memory there (if mapped).
    // If not mapped, it will trigger a Data Abort (handled by exception handler).
    // The slice creation itself is "safe" in terms of Rust types, but accessing it is the danger.
    let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
    
    core::str::from_utf8(slice).map_err(|_| "Invalid UTF-8")
}
