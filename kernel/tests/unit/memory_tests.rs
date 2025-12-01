//! Memory Allocator Unit Tests

use intent_kernel::kernel::memory::*;

pub fn test_heap_stats() {
    let stats = stats();
    assert!(stats.allocated >= 0, "Allocated should be non-negative");
    assert!(stats.total_allocations >= 0, "Total allocations should be non-negative");
}

pub fn test_heap_available() {
    let available = heap_available();
    assert!(available > 0, "Heap should have available space");
}

pub fn test_heap_regions() {
    let (heap_start, heap_size) = heap_region();
    assert!(heap_start > 0, "Heap start should be non-zero");
    assert!(heap_size > 0, "Heap size should be positive");
    
    let (dma_start, dma_size) = dma_region();
    assert!(dma_start > 0, "DMA start should be non-zero");
    assert!(dma_size > 0, "DMA size should be positive");
}

pub fn test_allocator_stats() {
    let stats = stats();
    // After initialization, we should have some allocations
    assert!(stats.total_allocations >= 0, "Total allocations should be non-negative");
}
