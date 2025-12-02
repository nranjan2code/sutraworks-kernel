# Project Roadmap

## ✅ Completed Milestones

### Phase 1: Foundation ✅
- [x] **Core Kernel**: Bootloader, UART, GPIO, Mailbox.
- [x] **Memory Management**: Buddy Allocator, Heapless support.
- [x] **Capability Security**: Token-based access control with derivation.

### Phase 2: Stenographic Engine ✅
- [x] **Stroke Processing**: 23-bit stroke patterns, RTFCRE notation.
- [x] **Dictionary**: Stroke→Intent mapping, multi-stroke sequences.
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
- [x] **Hailo-8 Driver**: HEF model loading and inference.
- [x] **Persistent Storage**: TAR RamDisk, Read-Write Overlay.

### Phase 5: Input/Output ✅
- [x] **USB HID Driver**: Full xHCI stack for steno machines (Georgi, Uni, Plover HID).
- [x] **Framebuffer Console**: Text output on HDMI display via `cprint!`/`cprintln!`.
- [x] **Dual Input Mode**: Steno strokes OR English text (reverse dictionary lookup).
- [x] **English→Steno Bridge**: `process_english("help")` → finds stroke `PH-FPL` → executes.

## 🚀 Next Strategic Steps

### Phase 6: Sensors & Connectivity (Current Focus)
Expand hardware integration.

- [ ] **Camera Driver**: MIPI CSI-2 / ISP for real image capture.

### Phase 7: Connectivity & Expansion
- [ ] **Networking**: Ethernet driver (via RP1/PCIe) for remote intent processing.
- [ ] **SD Card Write**: FAT32 driver to persist RamDisk state.
- [ ] **Multi-Core SMP**: Run Perception and Intent Engine on separate cores.

### Phase 8: Cognitive Evolution
- [ ] **LLM Integration**: Small quantized LLM on CPU/NPU hybrid.
- [ ] **Voice Interface**: Audio I/O for spoken intents.
- [ ] **Dictionary Learning**: Track stroke frequency, suggest optimizations.
- [ ] **Predictive Strokes**: Suggest next stroke based on context.

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
