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

### Infrastructure ✅
- [x] **PCIe Driver (BCM2712)**: Root Complex for RP1 and Hailo-8.
- [x] **Hailo-8 Driver**: HEF model loading and inference.
- [x] **Persistent Storage**: TAR RamDisk, Read-Write Overlay.

## 🚀 Next Strategic Steps

### Phase 4: Hardware Integration (Current Focus)
Connect to real physical hardware.

- [ ] **USB HID Driver**: Connect real steno machines (Georgi, Plover HID).
- [ ] **Camera Driver**: MIPI CSI-2 / ISP for real image capture.

### Phase 5: Connectivity & Expansion
- [ ] **Networking**: Ethernet driver (via RP1/PCIe) for remote intent processing.
- [ ] **SD Card Write**: FAT32 driver to persist RamDisk state.
- [ ] **Multi-Core SMP**: Run Perception and Intent Engine on separate cores.
- [ ] **Framebuffer**: Visual display of intents and system status.

### Phase 6: Cognitive Evolution
- [ ] **LLM Integration**: Small quantized LLM on CPU/NPU hybrid.
- [ ] **Voice Interface**: Audio I/O for spoken intents.
- [ ] **Dictionary Learning**: Track stroke frequency, suggest optimizations.

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
