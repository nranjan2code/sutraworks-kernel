# Intent Kernel TODO

A Perceptual Computing Platform for Raspberry Pi 5.
Bare-metal OS where any input (steno, keyboard, vision, audio) becomes semantic intent.

## Architecture

**Core Principle**: Strokes are the native semantic unit.

```
Steno Machine → Stroke (23-bit) → Dictionary → Intent → Executor
```

No characters. No words. No NLP. Pure stroke→intent mapping.

---

## Phase 1: Foundation ✅

### Boot & Hardware
- [x] ARM64 boot sequence (EL2→EL1)
- [x] USB Driver (Real xHCI + HID) for debug output
- [x] Timer driver (ARM generic timer)
- [x] Basic memory allocator
- [x] Exception vectors (EL1)
- [x] GPIO driver

### Kernel Core
- [x] SpinLock synchronization
- [x] Capability-based security model
- [x] Basic scheduler (round-robin)
- [x] Async executor for I/O

---

## Phase 2: Input Engine ✅

### Stroke Processing
- [x] `Stroke` struct (23-bit pattern)
- [x] Plover key layout (23 keys)
- [x] RTFCRE notation parser
- [x] Raw stroke input from hardware

### Dictionary
- [x] `StenoDictionary` with stroke→intent mapping
- [x] Multi-stroke sequences
- [x] `ConceptID` semantic identifiers
- [x] Built-in system concepts (HELP, STATUS, UNDO)

### Engine
- [x] `StenoEngine` state machine
- [x] `StrokeProducer` trait for input sources
- [x] `IntentConsumer` trait for output handlers
- [x] Global engine with `init()`, `process_stroke()`

---

## Phase 3: Intent Execution ✅

### Intent System
- [x] `Intent` struct with ConceptID + data
- [x] `IntentExecutor` with capability checks
- [x] System intents (help, status, shutdown)
- [x] User-defined intent handlers
- [x] Intent queueing and prioritization

### Memory Integration
- [x] ConceptID-based memory allocation
- [x] HDC Memory (Hypervectors)
- [x] HDC Memory (Hypervectors)
- [x] **HNSW Indexing** (O(log N) Retrieval)
- [x] **Stack Safety** (VMM-backed Guard Pages)
- [x] Stroke history buffer
- [ ] Dictionary caching

---

## Phase 4: Hardware Integration ✅

### Steno Input ✅
- [x] USB HID driver for steno machines (xHCI Host Controller)
- [x] Georgi/Plover HID protocol support
- [x] N-key rollover detection
- [x] Stroke timing (for disambiguation)

### Display Output ✅
- [x] Framebuffer driver (1920x1080x32)
- [x] Framebuffer Console (`cprint!`, `cprintln!`)
- [x] Intent visualization in HUD
- [x] Stroke echo display (steno tape)
- [x] System status display

### AI Acceleration ✅
- [x] Hailo-8L PCIe driver (HCP + DMA)
- [x] Model loading from filesystem (HEF Parser)
- [x] Inference Pipeline (YOLO Tensor Parsing)
- [ ] Intent augmentation (context-aware) - Planned

---

## Phase 5: Dual Input Mode ✅

### English Input Support
- [x] Reverse dictionary lookup (`lookup_by_name`)
- [x] `process_english()` function
- [x] Automatic fallback: try English first, then Steno notation
- [x] Output in English via console

### Architecture
- [x] English → ConceptID → Intent (semantic-first)
- [x] No character parsing—direct dictionary lookup
- [x] User types "help", kernel finds `PH-FPL`, executes HELP intent

---

## Phase 6: Advanced Features

### Broadcast Architecture ✅
- [x] Refactor `IntentExecutor` for 1:N broadcast
- [x] Implement `HandlerResult::StopPropagation`
- [x] Update `HandlerRegistry` logic

### Sensor Fusion ✅
- [x] `PerceptionManager` with N:1 fusion
- [x] Hot-pluggable sensor support (`Vec<Box<dyn ObjectDetector>>`)
- [x] Virtual sensor verification
- [ ] Camera Driver (MIPI CSI-2)

### Next-Gen Memory (HDC) ✅
- [x] Hyperdimensional Computing (1024-bit vectors)
- [x] Cognitive Algebra (Bind, Bundle, Permute)
- [x] Hamming Similarity (Bitwise retrieval)

### Multi-stroke Processing ✅
- [x] Stroke sequence timeout (500ms)
- [x] Prefix matching for buffered strokes
- [x] Multi-stroke dictionary with 20+ entries
- [x] 2-stroke and 3-stroke brief support
- [ ] Prefix/suffix strokes (modifiers)
- [ ] Stroke correction (undo stroke)
- [ ] Fingerspelling mode

### Dictionary Management
- [ ] User dictionary overlay
- [ ] Dictionary import (JSON format)
- [ ] Stroke frequency tracking
- [ ] Dynamic dictionary updates

### v1.0.0 (Vision)
- [ ] Stable API
- [x] Audio support
- [ ] Multi-process
- [x] Security audit (Base hardening complete)
- [ ] Performance optimization
- [x] Real PCIe Driver (Implemented Structure)
- [x] Hailo Driver (Real Driver Structure Implemented)

### System Services
- [x] Virtual memory with page tables
- [x] Process isolationing via stroke
- [ ] Inter-process messaging
- [ ] Power management

### Phase 8: Error Recovery ✅ (Complete)
- [x] Driver Watchdogs (USB, SD, Net, Hailo)
- [x] Graceful Degradation (CPU Fallback)
- [x] System Resilience

### Phase 8.5: TCP Robustness ✅ (Complete)
- [x] TCP Connection Tracking (`TcpConnection`, `TCB_TABLE`)
- [x] Retransmission with RTT-based RTO (Jacobson/Karels algorithm)
- [x] Congestion Control (RFC 5681: Slow Start, Congestion Avoidance, Fast Recovery)
- [x] Fast Retransmit (3 duplicate ACKs)
- [x] ARP Cache (16-entry, `resolve()`, `cache_insert()`)
- [x] Network Configuration (`NetConfig`)
- [x] RFC 1071 Checksum (`checksum()`)
- [x] `tcp_tick()` scheduler integration (every 100ms)
- [x] **TCP Checksum (RFC 793)**
  - [x] `tcp_checksum()` with pseudo-header
  - [x] `verify_tcp_checksum()` 
  - [x] `TcpSegment::to_bytes_with_checksum()`
- [x] **17 TCP Unit Tests**
  - [x] Flags, parsing, checksum tests
  - [x] RTT estimation tests (Jacobson/Karels)
  - [x] Congestion control state tests
  - [x] Connection identity, retransmit queue, sequence wraparound tests
- [x] **Host Test Runner Fix**
  - [x] Add `#[cfg(not(test))]` to `#[global_allocator]`
  - [x] All 18 TCP tests now pass on host

---

## Phase 7: Polish

### Testing
- [x] Unit tests for stroke parsing (25 tests)
- [x] **Codebase Cleanup**
    - [x] Remove legacy floating-point embeddings.
    - [x] Fix all compiler warnings.
    - [x] Clarify stub/fake implementations.
- [x] Integration tests on QEMU
- [ ] Hardware tests on Pi 5

### Documentation
- [x] Copilot instructions
- [x] Architecture docs
- [x] API reference
- [ ] User guide

### Performance
- [x] Dictionary hash optimization (LSH Indexing)
- [ ] Zero-copy stroke processing
- [x] DMA for USB transfers (Non-Cacheable Mapping)
- [ ] Power-aware scheduling

---

## Non-Goals

These are explicitly **NOT** planned:
- ❌ NLP or tokenization (English uses direct lookup, not parsing)
- ❌ Backward compatibility with word-based systems
- ❌ Traditional shell/terminal
- ❌ POSIX compatibility

**Clarification**: English text input IS supported, but it's a convenience layer.
The kernel never parses English—it performs direct dictionary lookup:
`"help"` → finds stroke `PH-FPL` → executes `HELP` intent.
