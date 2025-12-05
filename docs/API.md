# Intent Kernel API Documentation

This document describes the public APIs exposed by the Intent Kernel, specifically focusing on the new health monitoring, recovery, and synchronization features added in Sprint 13.

## 1. Scheduler & Health Metrics

### `kernel::scheduler::get_core_stats(core_id: usize) -> CoreStats`
Retrieves performance statistics for a specific CPU core.

**Returns:**
- `CoreStats`: Struct containing:
    - `idle_cycles`: Total cycles spent in idle loop.
    - `total_cycles`: Total cycles since boot.
    - `queue_length`: Current number of tasks in the run queue.

### `kernel::scheduler::record_idle_start(core_id: usize) -> u64`
Records the start of an idle period. Called by the idle loop.

### `kernel::scheduler::record_idle_end(core_id: usize, start_time: u64)`
Records the end of an idle period and updates `idle_cycles`.

## 2. Memory Management

### `kernel::memory::stats() -> AllocatorStats`
Retrieves global memory allocator statistics.

**Returns:**
- `AllocatorStats`: Struct containing:
    - `allocated`: Total bytes currently allocated (Buddy + Slab).
    - `slab_allocated`: Bytes allocated specifically by the Slab cache.
    - `total_allocations`: Total number of allocations (currently 0/reserved).

### `kernel::memory::heap_available() -> usize`
Returns the approximate number of bytes available in the kernel heap.

### `kernel::memory::force_compact()`
Triggers a memory compaction attempt (Recovery Action).
*Note: Current allocator is non-moving; this logs the attempt.*

## 3. Deadlock Detection & Recovery

### `kernel::watchdog::deadlock::detect_circular_wait() -> Option<Vec<u64>>`
Detects if there is a deadlock cycle in the system.

**Returns:**
- `Some(Vec<u64>)`: A list of Task IDs involved in the cycle.
- `None`: No deadlock detected.

### `kernel::watchdog::recovery::recover_hung_core(core_id: usize) -> Result<(), &'static str>`
Attempts to recover a hung core by sending an IPI.

### `kernel::watchdog::recovery::break_deadlock()`
Attempts to break a detected deadlock by killing the current task (failsafe).

## 4. Synchronization

### `kernel::sync::SpinLock<T>`
A spinlock that tracks ownership and wait dependencies for deadlock detection.
Use this for all high-level kernel synchronization.

### `kernel::sync::RawSpinLock<T>`
A raw spinlock WITHOUT tracking.
Use this ONLY for:
- Memory Allocator internals (to avoid recursion).
- Lock Registry internals.
- Low-level architectural primitives.

## 5. Network

### `kernel::net::checksum(data: &[u8]) -> u16`
Calculates the Internet Checksum (RFC 1071) for a buffer.
Used by IP, ICMP, UDP, and TCP.

## 6. Neural Intent Architecture

### `intent::temporal::summate(concept_id: ConceptID, strength: f32, timestamp: u64)`
Accumulates weak activation signals over time for a concept.
- `strength`: Signal strength (0.0-1.0).
- If accumulated activation > 0.5, the concept fires.

### `intent::temporal::predict(source: ConceptID, predicted: ConceptID, confidence: f32, timestamp: u64)`
Registers a predictive priming link (Next-Token Prediction).
- `source`: The concept currently active.
- `predicted`: The concept expected to follow.

### `intent::hierarchy::input_intent(intent: &Intent)`
Injects an intent into the hierarchical processing system at its designated level.
- Levels: Raw → Feature → Object → Semantic → Action.

### `intent::scheduling::submit_intent(request: IntentRequest) -> bool`
Submits an intent to the Neural Scheduler.
- `request.urgency`: Dynamic urgency (0.0-1.0) from Basal Ganglia model.
- `request.priority`: Static priority (admin/user).
- `request.surprise_boost`: Boost from Feedback Loop (unexpected events).

### `intent::feedback::process_input(concept_id: ConceptID, timestamp: u64) -> FeedbackResult`
Processes an input through the feedback loop to check against predictions.
- Returns `surprise` level (0.0 = predicted, 1.0 = unexpected).

