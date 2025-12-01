# Intent Kernel - TODO

A bare-metal stenographic operating system for Raspberry Pi 5.

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
- [x] UART driver for debug output
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

## Phase 2: Stenographic Engine ✅

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

## Phase 3: Intent Execution (Current)

### Intent System
- [x] `Intent` struct with ConceptID + data
- [x] `IntentExecutor` with capability checks
- [x] System intents (help, status, shutdown)
- [ ] User-defined intent handlers
- [ ] Intent queueing and prioritization

### Memory Integration
- [x] ConceptID-based memory allocation
- [x] Neural memory regions
- [ ] Stroke history buffer
- [ ] Dictionary caching

---

## Phase 4: Hardware Integration

### Steno Input
- [ ] USB HID driver for steno machines
- [ ] Georgi/Plover HID protocol
- [ ] N-key rollover detection
- [ ] Stroke timing (for disambiguation)

### Display Output
- [ ] Framebuffer driver
- [ ] Intent visualization
- [ ] Stroke echo display
- [ ] System status display

### AI Acceleration
- [ ] Hailo-8L PCIe driver
- [ ] Model loading from ramdisk
- [ ] Intent augmentation (context-aware)

---

## Phase 5: Advanced Features

### Multi-stroke Processing
- [ ] Stroke sequence timeout
- [ ] Prefix/suffix strokes
- [ ] Stroke correction (undo stroke)
- [ ] Fingerspelling mode

### Dictionary Management
- [ ] User dictionary overlay
- [ ] Dictionary import (JSON format)
- [ ] Stroke frequency tracking
- [ ] Dynamic dictionary updates

### System Services
- [ ] File system (ramfs)
- [ ] Process spawning via stroke
- [ ] Inter-process messaging
- [ ] Power management

---

## Phase 6: Polish

### Testing
- [ ] Unit tests for stroke parsing
- [ ] Dictionary lookup tests
- [ ] Integration tests on QEMU
- [ ] Hardware tests on Pi 5

### Documentation
- [x] Copilot instructions
- [x] Architecture docs
- [ ] API reference
- [ ] User guide

### Performance
- [ ] Dictionary hash optimization
- [ ] Zero-copy stroke processing
- [ ] DMA for USB transfers
- [ ] Power-aware scheduling

---

## Non-Goals

These are explicitly **NOT** planned:
- ❌ Character/word parsing
- ❌ NLP or tokenization
- ❌ Embedding vectors or similarity search
- ❌ Backward compatibility with word-based systems
- ❌ Traditional shell/terminal
- ❌ POSIX compatibility
