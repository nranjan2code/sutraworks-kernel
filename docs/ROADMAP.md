# Project Roadmap

## ✅ Completed Milestones

### Phase 1: Foundation ✅
- [x] **Core Kernel**: Bootloader, UART, GPIO, Mailbox.
- [x] **Memory Management**: Buddy Allocator, Heapless support.
- [x] **Capability Security**: Token-based access control with derivation.
### v0.2.0 (Completed)
- [x] Virtual memory management (VMM, Paging, UserAddressSpace)
- [x] Basic process model (Agents, Context Switching)
- [x] Kernel/userspace separation (EL0/EL1, TTBR0/TTBR1)
- [x] System call interface (SVC handler)

### Phase 2: Input Engine ✅
- [x] **Stroke Processing**: 23-bit stroke patterns, RTFCRE notation.
- [x] **Dictionary**: Stroke→Intent mapping, multi-stroke sequences.
- [x] **Multi-Stroke Briefs**: Real prefix matching, 500ms timeout, 20+ multi-stroke entries.
- [x] **Engine**: StenoEngine state machine, StrokeProducer/IntentConsumer traits.

### Phase 3: Intent Execution ✅
- [x] **Intent System**: ConceptID, Intent struct, IntentExecutor.
- [x] **User Handlers**: 128-handler registry with priority dispatch.
- [x] **Intent Queue**: 32-entry priority queue with deadlines.
- [x] **Stroke History**: 64-entry ring buffer with undo/redo.
- [x] **Testing**: 122 host-based tests across 7 modules.

### Phase 4: Perception & UI ✅
- [x] **Perception Manager**: Adaptive backend selection (Hailo/CPU).
- [x] **Heads-Up Display (HUD)**: Real-time scrolling steno tape and intent log.
- [x] **Framebuffer Driver**: High-performance direct pixel access.

### Infrastructure ✅
- [x] **PCIe Driver (BCM2712)**: Root Complex for RP1 and Hailo-8.
- [x] **Hailo-8 Driver**: Real PCIe Driver Structure (Command Rings, DMA).
- [x] **Persistent Storage**: TAR RamDisk, Read-Write Overlay.

### Phase 5: Input/Output ✅
- [x] **USB HID Driver**: Full xHCI stack for steno machines (Georgi, Uni, Plover HID).
- [x] **Framebuffer Console**: Text output on HDMI display via `cprint!`/`cprintln!`.
- [x] **Dual Input Mode**: Steno strokes OR English text (reverse dictionary lookup).
- [x] **English→Steno Bridge**: `process_english("help")` → finds stroke `PH-FPL` → executes.

### Phase 5.5: Broadcast & Fusion Architecture ✅
- [x] **Broadcast Intent**: 1:N intent execution (Motor Control theory).
- [x] **Sensor Fusion**: N:1 perception aggregation (Virtual Camera + Lidar).
- [x] **Hot-Pluggable Sensors**: Dynamic sensor registration.

### Phase 5.9: Next-Gen Memory (HDC) ✅
- [x] **Semantic Memory**: ConceptID-based O(log N) storage.
- [x] **Cognitive Algebra**: Bind, Bundle, Permute operations.
- [x] **Hamming Similarity**: Bitwise semantic retrieval.
- [x] **Concept Indexing**: BTreeMap retrieval for scalable memory.
- [x] **Stack Safety**: VMM-backed stacks with Guard Pages.
- [x] **Visual Intents**: Vision-to-Memory bridge using ConceptIDs.

### Phase 6: Sensors & Connectivity ✅
- [x] **Hailo-8 Driver**: Full HCP protocol, DMA engine, and YOLO tensor parsing.
- [x] **Audio Perception**: Zero Crossing Rate (ZCR) + Energy feature extraction.
- [x] **Acoustic Intents**: Speech/Noise classification and neural memory storage.

### Phase 6: Connectivity & Expansion ✅
- [x] **Networking**: Real Ethernet driver (RP1/PCIe) and TCP/IP stack.
- [x] **Persistent Storage**: SDHCI driver for SD card read/write.
- [x] **Multi-Core SMP**: 4-core scheduler with work stealing and affinity.
- [x] **Userspace & Scheduling**: ELF Loader, Preemptive Scheduler, Syscalls.

### Phase 7: Semantic Visual Interface (SVI) ✅
- [x] **Visual Layer**: Broadcast listener architecture (`kernel/src/visual`).
- [x] **Projections**: StenoTape, IntentLog, Status, Help, Perception, MemoryGraph.
- [x] **Compositor**: Z-order rendering and intent-reactive updates.
- [x] **Migration**: Replaced legacy HUD with modular projection system.

### Phase 8: Error Recovery & Resilience ✅
- [x] **Driver Watchdogs**: Reset mechanisms for USB, SD, Network, Hailo.
- [x] **Graceful Degradation**: CPU fallback for AI, Serial fallback for display.
- [x] **System Resilience**: Network re-init, Filesystem error recovery.
- [ ] **LLM Integration**: Small quantized LLM on CPU/NPU hybrid (Future).

### Phase 9: Benchmark Suite ✅
- [x] **40-Benchmark Suite**: Intent Engine, Semantic Memory, Perception, Multi-Modal, Process, Lock, Interrupt, I/O.
- [x] **Performance Validation**: ~60M ops/sec, 1M concept stress test verified.
- [x] **Architecture Documentation**: Full [BENCHMARKS.md](BENCHMARKS.md) with algorithms and methodology.

## Test Coverage

| Module | Tests | Status |
|--------|-------|--------|
| Stroke | 25 | ✅ |
| Capability | 20 | ✅ |
| Dictionary | 20 | ✅ |
| Concept | 22 | ✅ |
| History | 12 | ✅ |
| Queue | 12 | ✅ |
| Handlers | 11 | ✅ |
| **Total** | **122** | ✅ |
