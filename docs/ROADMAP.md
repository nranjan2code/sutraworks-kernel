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
- [x] **Userspace & Scheduling**: ELF Loader, Preemptive Scheduler, Syscalls, **Intent-Native Shell**.

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

### Phase 10: Neural Architecture Upgrade ✅
- [x] **Core Neural Primitives**: Activation levels, spreading activation, lateral inhibition.
- [x] **Temporal Dynamics**: Decay, summation, predictive priming.
- [x] **Hierarchical Processing**: 5-layer propagation, top-down modulation, attention.
- [x] **Feedback Loops**: Efference copy, expectation matching, surprise detection.
- [x] **Neural Scheduler**: Urgency-based preemption, basal ganglia model.

### Phase 11: Core Performance Optimization (Sprint 13.3) ✅
- [x] **ASID Support**: 16-bit ASID tagging for O(1) context switches (avoid `vmalle1`).
- [x] **O(1) Allocator**: Slab CLZ + Buddy FreeMask optimizations (28 cycles/op).
- [x] **Zero-Copy Parser**: Optimized English parser (133 cycles, ~15x faster).
- [x] **Benchmark Consistency**: Full 40-benchmark suite passing on QEMU.

### Phase 12: Semantic Multi-Tasking (Sprint 15) ✅
- [x] **Process Manager**: PID tracking, process states, and lifecycle management.
- [x] **Syscall Spawn**: Loading ELF binaries (with simulated embedded loading).
- [x] **Biological IPC**: `sys_ipc_send`/`sys_ipc_recv` for direct agent communication.
- [x] **Semantic Binding**: `sys_announce` allows processes to register as Concept Handlers.
- [x] **Skill Proxy**: Kernel-side proxy (`ProcessSkill`) ensuring transparent Intent->IPC routing.
- [x] **Architecture Audit**: Removal of legacy BSD socket artifacts.

### Phase 13: Neural Architecture Verification (Dec 2025) ✅
- [x] **Shell Refactor**: Routes commands through `SYS_PARSE_INTENT` syscall instead of classical string matching.
- [x] **Temporal Wiring**: `decay_tick()` (100ms interval) integrated into scheduler timer interrupt.
- [x] **Hierarchical Wiring**: `propagate_all()` (50ms interval) integrated into scheduler timer interrupt.
- [x] **Skill Registry Connection**: Intent broadcast falls back to registered skills.
- [x] **Verification Proof**: QEMU output confirms `[NEURAL] tick=...` messages firing every second.

### Phase 16: Intent App Framework (Sprint 19) ✅
- [x] **AppManager**: Lifecycle management for declarative apps.
- [x] **Manifest Loader**: YAML-like parsing of `.intent` manifests.
- [x] **Semantic Linker**: Runtime binding of intents to skills (`Resolve -> Bind -> Execute`).
- [x] **End-to-End Demo**: "Smart Doorknob" and "Hello World" apps verification.
- [x] **Steno Integration**: Fallback to AppManager for unknown English commands.

### Phase 17: File System & Pure Intent Features (Sprint 17) ✅
- [x] **VFS Support**: `readdir` implementation for FAT32 (Root directory).
- [x] **Directory Listing**: `sys_getdents64` syscall.
- [x] **Pure Intent `ls`**: Kernel-side directory listing intent.
- [x] **Pure Intent `cat`**: Kernel-side file reading with argument parsing, avoiding hybrid syscalls.

### Phase 15: Code Quality & Technical Debt (Sprint 18) ✅
- [x] **28 Clippy Warnings Eliminated**: Unnecessary unsafe blocks, dead code, unused variables, style issues.
- [x] **Zero-Warning Build**: Clean `cargo clippy` output achieved.
- [x] **Public Driver Interfaces**: `VirtioNet`, `VirtioBlock` visibility corrected.
- [x] **Code Hygiene**: Consistent `#[allow(dead_code)]` for reserved/conditional functions.
- [x] **Style Improvements**: Auto-deref, strip_prefix, slice params, is_empty methods.
### Phase 18: System 2 Cognitive Upgrade (Sprint 20) ✅
- [x] **LLM Loader**: `llm::loader` with FAT32 weight loading (.bin files).
- [x] **Memory Management**: `OwnedWeights` vs `Weights` split for zero-copy inference.
- [x] **Inference Engine**: Transformer forward pass (Llama 2 architecture) in `no_std`.
- [x] **Fail-Safe Boot**: Fallback dummy weights for resilient testing without SD card.

## Test Coverage

| Module | Tests | Status |
|--------|-------|--------|
| Stroke | 25 | ✅ |
| Capability | 20 | ✅ |
| Dictionary | 20 | ✅ |
| Concept | 22 | ✅ |
| History | 12 | ✅ |
| Queue | 12 | ✅ |
| Handlers | 40 | ✅ |
| Audio | 3 | ✅ |
| Manifest | 1 | ✅ |
| Registry | 1 | ✅ |
| **Total** | **129** | ✅ |

