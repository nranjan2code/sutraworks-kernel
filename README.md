<p align="center">
  <img src="docs/assets/logo.png" alt="Intent Kernel" width="200" />
</p>

<h1 align="center">Intent Kernel</h1>

<p align="center">
  <strong>The world's first stenographic operating system</strong><br>
  <em>Where strokes become intents, and intents become action.</em>
</p>

<p align="center">
  <a href="#quick-start">Quick Start</a> •
  <a href="#why-steno">Why Steno?</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#documentation">Docs</a> •
  <a href="#status">Status</a>
</p>

---

## The Vision

What if your computer understood you at 200+ words per minute?

**Intent Kernel** is a bare-metal operating system for Raspberry Pi 5 that speaks stenography natively. No characters. No parsing. No shell commands. Just pure **stroke → intent → action**.

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ Steno Machine│────▶│   Stroke     │────▶│  Dictionary  │────▶│   Executor   │
│              │     │   (23-bit)   │     │              │     │              │
└──────────────┘     └──────────────┘     └──────────────┘     └──────────────┘
```

One stroke. One concept. Instant execution.

---

## Quick Start

```bash
# Clone the repository
git clone https://github.com/sutraworks/intent-kernel.git
cd intent-kernel

# Build the kernel
make kernel

# Run in QEMU (for testing)
make run

# Run 122 unit tests
make test
```

### Requirements

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | nightly | Compiler |
| aarch64-unknown-none | - | Target triple |
| QEMU | 8.0+ | Emulation (optional) |

---

## Why Steno?

Stenography is 150-year-old technology that **still** outperforms every input method invented since:

| Method | Speed | Accuracy |
|--------|-------|----------|
| Typing | 40-80 WPM | High |
| Voice | 100-150 WPM | Medium |
| **Steno** | **200-300 WPM** | **Very High** |

A stenographer doesn't type "show system status" — they press **one chord** that means exactly that. The Intent Kernel takes this further: that chord maps directly to a semantic concept, skipping all text processing entirely.

### Traditional OS
```
Keyboard → Characters → Shell → Parser → Tokens → Command Lookup → Execute
```

### Intent Kernel (Steno Mode)
```
Steno Machine → Stroke → Intent → Execute
```

### Intent Kernel (English Mode) ✨ NEW!
```
Keyboard → English Text → Natural Language Parser → Intent → Execute
                              ↓
                    (200+ phrases, 50+ synonyms)
```

**Faster. Cleaner. More powerful. Now accessible to everyone.**

Users can type **natural English commands** like "show me system status" or "can you help?". The kernel includes a production-grade English I/O layer that understands 200+ phrase variations, expands synonyms, and generates natural language responses—all while maintaining the steno-native core architecture internally.

---

## Architecture

## Architecture

### The Stroke

Every steno chord produces a 23-bit pattern representing which keys were pressed:

```
Position:  0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15  16  17  18  19  20  21  22
Key:       #  S-  T-  K-  P-  W-  H-  R-  A-  O-  *  -E  -U  -F  -R  -P  -B  -L  -G  -T  -S  -D  -Z
```

### The Dictionary

Strokes map to **concepts**, not text:

**Single-Stroke Briefs:**
| Stroke | Notation | Concept | Action |
|--------|----------|---------|--------|
| `0x42` | `STAT` | STATUS | Display system status |
| `0x400` | `*` | UNDO | Undo last action |
| `0x1A4` | `HELP` | HELP | Show help |
| `0x...` | `SHRO` | SHOW | Display something |

**Multi-Stroke Briefs:**
| Strokes | Notation | Concept | Action |
|---------|----------|---------|--------|
| 2 | `RAOE/PWOOT` | REBOOT | Restart system |
| 2 | `SHUT/TKOUPB` | SHUTDOWN | Power off |
| 2 | `SKROL/UP` | SCROLL_UP | Scroll display up |
| 3 | `TPHU/TPAOEU/-L` | NEW_FILE | Create new file |
| 3 | `KP-U/EUPB/TPO` | CPU_INFO | Show CPU info |

### The Broadcast (1:N)

Intents are **broadcast** to all interested listeners, not just dispatched to a single handler. This mimics the brain's motor control system.

```
Intent ("GRASP") ────┬────▶ Motor Cortex (Move Arm)
                     ├────▶ Visual Cortex (Track Hand)
                     └────▶ Proprioception (Expect Weight)
```

### Sensor Fusion (N:1)

The **Perception Layer** fuses data from all active sensors into a single "World Model".

```
Camera (Hailo-8) ──┐
Lidar (Virtual)  ──┼──▶ Perception Manager ──▶ World Model
Touch Sensors    ──┘
```

### The Flow

```rust
// A stroke comes in from hardware
let stroke = Stroke::from_raw(0x42);

// The engine processes it
if let Some(intent) = steno::process_stroke(stroke) {
    // The executor broadcasts it
    intent::execute(&intent);
}
```

---

## Features

### ✅ Stroke History
64-entry ring buffer with full undo/redo support.

```rust
steno::undo();  // Undo last stroke
steno::redo();  // Redo if possible
```

### ✅ Multi-Stroke Briefs
Real multi-stroke support with prefix matching and timeout:

```rust
// 2-stroke briefs
"RAOE/PWOOT" → REBOOT
"SHUT/TKOUPB" → SHUTDOWN
"RAOE/KAUL" → RECALL

// 3-stroke briefs
"TPHU/TPAOEU/-L" → NEW_FILE
"KP-U/EUPB/TPO" → CPU_INFO
```

- 500ms timeout between strokes
- Prefix matching (waits when partial match exists)
- 20+ built-in multi-stroke entries

### ✅ User-Defined Handlers
Register custom handlers for any concept:

```rust
intent::register_handler(
    concepts::STATUS,
    my_status_handler,
    "custom_status"
);
```

### ✅ Priority Queue
Defer and prioritize intent execution:

```rust
intent::queue_with_priority(
    Intent::new(concepts::SAVE),
    Priority::Critical,
    timestamp
);
```

### ✅ Capability Security
Fine-grained permission control:

```rust
if !has_capability(CapabilityType::System) {
    return Err("Permission denied");
}
```

### ✅ Heads-Up Display (HUD)
Real-time visualization of the stenographic stream and intent execution log.
- **Steno Tape**: Scrolling log of raw strokes (RTFCRE).
- **Intent Stream**: Visual log of recognized semantic actions.
- **Status Bar**: Real-time WPM and stroke statistics.

### ✅ Natural Language Interface ✨ NEW!
Production-grade English I/O layer for universal accessibility:

```rust
// Natural English (200+ phrases)
english::parse("show me system status");
english::parse("can you help?");
english::parse("what's happening?");

// Context-aware conversations
let mut ctx = ConversationContext::new();
ctx.parse("status");           // Execute STATUS
ctx.parse("show it again");    // Repeat from context
ctx.parse("more details");     // Detailed version

// Natural language responses
let response = english::generate_response(&intent, &result);
// Output: "System: CPU 45%, RAM 2.3GB, Uptime 3h 42m"
```

**Features**:
- **200+ Phrase Variations**: "help", "?", "what can you do", "commands", etc.
- **50+ Synonyms**: "quit"→"exit", "info"→"status", "what's"→"what is"
- **Multi-Stage Parsing**: Exact match → Synonym expansion → Keyword extraction
- **Conversation Context**: Stateful understanding of follow-up questions
- **User Mode Adaptation**: Beginner (verbose) → Advanced (concise)
- **Performance**: <30μs overhead per command (negligible!)

### ✅ Dual Input Mode
Power users can still use raw steno:

```rust
// Steno notation (for speed)
steno::process_steno("STAT");    // Direct stroke → intent

// Hybrid mode (mix both)
english::parse("STAT");          // Recognizes steno too!
```

### ✅ Secure Base
Interrupt-safe concurrency primitives and removal of unsafe global state.
- **Deadlock-Free SpinLocks**: Automatically disable interrupts.
- **Safe Interrupts**: Thread-safe handler registration.
- **Overflow Protection**: Hardened filesystem parsers.

### ✅ Real Neural Memory ✨ NEW!
True "Vector Symbolic Architecture" (VSA) memory system:
- **1024-bit Binary Hypervectors**: Replaced inefficient floats with holographic bit patterns.
- **HNSW Indexing**: **O(log N)** graph-based retrieval for scalable performance (replaced linear scan).
- **Dynamic Page Allocation**: Memory grows indefinitely with system RAM (Bump Allocator).
- **Cognitive Algebra**: `Bind`, `Bundle`, and `Permute` operations for semantic reasoning.
- **Robustness**: Information is distributed across 1024 bits; resilient to noise and bit flips.

### ✅ Safe Stack Architecture
- **VMM-Backed Stacks**: Real virtual memory pages for process stacks.
- **Guard Pages**: Unmapped pages at the bottom of every stack to trap overflows instantly.

### ✅ True Memory Isolation
Process isolation via ARM64 VMSA (Virtual Memory System Architecture):
- **UserAddressSpace**: Each process has its own Page Table (TTBR0).
- **Kernel Protection**: Kernel memory is mapped as Privileged-Only (EL1), inaccessible to user mode.
- **Context Switching**: `TTBR0` is updated on every context switch, ensuring complete address space separation.

### ✅ Robust USB Host
Full xHCI Host Controller Driver with RAII Memory Management:
- **Command Ring**: Proper cycle bit management and doorbell ringing.
- **Event Loop**: Asynchronous state machine handling Transfer Events and Command Completion.
- **Memory Safety**: `DmaBuffer` ensures DMA memory is automatically freed when dropped, preventing leaks.
- **Control Transfers**: Setup/Data/Status stages for device configuration.
- **Context Management**: Real Input/Output Contexts and Device Slots.

### ✅ Real Perception ✨ COMPLETE!
Computer Vision pipeline with Hardware Acceleration support:
- **Hailo-8 Driver**: Full YOLO tensor parser with NMS algorithm
- **Tensor Parsing**: Processes 1917 detection boxes → Top 16 objects with hypervectors
- **Sensor Fusion**: Combines data from multiple detectors (Hailo-8 + CPU fallback)
- **Visual Intents**: Automatically generates 1024-bit Hypervectors for detected objects and stores them in Neural Memory. The system "remembers" what it sees.

### ✅ Audio Perception ✨ NEW!
The kernel can "hear" and classify sounds:
- **Feature Extraction**: Zero Crossing Rate (ZCR) + Energy.
- **Acoustic Intents**: Maps sounds (Speech, Noise, Silence) to Semantic Hypervectors.
- **Neural Integration**: Stores acoustic memories alongside visual ones.

### ✅ Multi-Core SMP ✨ COMPLETE!
Production-grade 4-core scheduler with advanced features:
- **Per-Core Run Queues**: Minimizes lock contention
- **4-Level Priority**: Idle, Normal, High, Realtime (< 100μs for steno)
- **Core Affinity**: Pin tasks to specific cores (Core 0 = steno, Core 1 = vision, Core 2 = audio, Core 3 = network)
- **Work Stealing**: Automatic load balancing across cores
- **Power Efficiency**: WFI (Wait For Interrupt) on idle cores

### ✅ Persistent Storage ✨ COMPLETE!
SD Card driver for permanent data:
- **SDHCI Driver**: Full initialization sequence for EMMC2
- **Block I/O**: 512-byte sector read/write
- **SDHC/SDXC**: High-capacity card support
- **Use Cases**: Save dictionaries, neural memory, session logs

### ✅ Networking Stack ✨ COMPLETE!
Full TCP/IP implementation (~1,125 LOC):
- **Ethernet**: DMA ring buffers, zero-copy TX/RX
- **ARP**: Address resolution with caching
- **IPv4**: Routing, checksum verification
- **ICMP**: Ping (echo request/reply)
- **UDP**: Connectionless transport
- **TCP**: Simplified connection-oriented (3-way handshake, data transfer)

### ✅ Framebuffer Console
Text output on HDMI display:

```rust
cprintln!("Intent executed: {}", intent.name);
```

---

## Project Structure

```
intent-kernel/
├── kernel/src/
│   ├── steno/              # Stenographic engine
│   │   ├── stroke.rs       # 23-bit stroke patterns
│   │   ├── dictionary.rs   # Stroke → Intent mapping
│   │   ├── engine.rs       # State machine
│   │   └── history.rs      # Undo/redo buffer
│   ├── english/            # ✨ English I/O Layer
│   │   ├── mod.rs          # Public API
│   │   ├── phrases.rs      # 200+ phrase mappings
│   │   ├── synonyms.rs     # 50+ synonym expansions
│   │   ├── parser.rs       # Multi-stage parser
│   │   ├── responses.rs    # Natural language generation
│   │   └── context.rs      # Conversation state
│   ├── intent/             # Intent execution
│   │   ├── mod.rs          # Core types
│   │   ├── handlers.rs     # User handler registry
│   │   └── queue.rs        # Priority queue
│   ├── perception/         # Adaptive Perception Layer
│   │   ├── mod.rs          # Perception Manager
│   │   ├── vision.rs       # Computer Vision (Hailo/CPU)
│   │   ├── audio.rs        # Audio processing
│   │   └── hud.rs          # Heads-Up Display
│   ├── net/                # ✨ TCP/IP Stack (NEW!)
│   │   ├── mod.rs          # Network core
│   │   ├── arp.rs          # Address resolution
│   │   ├── ipv4.rs         # IPv4 routing
│   │   ├── icmp.rs         # ICMP (ping)
│   │   ├── udp.rs          # UDP transport
│   │   └── tcp.rs          # TCP transport
│   ├── drivers/            # Hardware
│   │   ├── uart.rs         # Serial I/O
│   │   ├── timer.rs        # ARM timer
│   │   ├── gpio.rs         # Pin control
│   │   ├── framebuffer.rs  # VideoCore display
│   │   ├── console.rs      # Text console on framebuffer
│   │   ├── hailo.rs        # ✨ Hailo-8 AI accelerator
│   │   ├── hailo_tensor.rs # ✨ YOLO tensor parser
│   │   ├── sdhci.rs        # ✨ SD card controller
│   │   ├── ethernet.rs     # ✨ Ethernet MAC driver
│   │   └── usb/            # USB Host Controller
│   │       ├── xhci.rs     # xHCI driver
│   │       └── hid.rs      # HID protocol (steno machines)
│   └── kernel/             # Core OS
│       ├── capability.rs   # Security
│       ├── scheduler.rs    # Single-core scheduler
│       ├── smp_scheduler.rs# ✨ Multi-core SMP scheduler
│       └── memory/         # Allocation
│           ├── mod.rs      # Memory subsystem
│           ├── neural.rs   # HDC memory
│           └── hnsw.rs     # HNSW index
├── tests/host/             # 122 unit tests
├── docs/                   # Documentation
└── boot/                   # ARM64 bootloader
```

---

## Status

| Phase | Status | What's Done |
|-------|--------|-------------|
| **1. Foundation** | ✅ | Boot, UART, GPIO, Timer, Memory, Scheduler |
| **2. Steno Engine** | ✅ | Stroke parsing, Dictionary, Engine, RTFCRE |
| **3. Intent System** | ✅ | Handlers, Queue, History, 122 tests |
| **4. Perception** | ✅ | **Complete Hailo-8** (YOLO parser, NMS, hypervectors), HUD |
| **5. Input/Output** | ✅ | **Real xHCI Driver**, HID Boot Protocol, Framebuffer Console |
| **5.5. English Layer** | ✅ | Natural Language I/O (200+ phrases, conversation context) |
| **6. Sensors** | ✅ | **Hailo-8 Tensor Parsing**, Audio Perception (ZCR/Energy), Vision |
| **7. Security** | ✅ | VMM Isolation (TTBR0 Switching, Kernel Protection) |
| **8. Multi-Core** | ✅ ✨ | **SMP Scheduler** (4 cores, priority, affinity, work stealing) |
| **9. Storage** | ✅ ✨ | **SD Card Driver** (SDHCI, block I/O, SDHC/SDXC) |
| **10. Networking** | ✅ ✨ | **TCP/IP Stack** (Ethernet, ARP, IPv4, ICMP, UDP, TCP) |

### Test Coverage

```
122 tests | 7 modules | < 1 second

stroke .......... 25 tests ✓
capability ...... 20 tests ✓
dictionary ...... 20 tests ✓
concept ......... 22 tests ✓
history ......... 12 tests ✓
queue ........... 12 tests ✓
handlers ........ 11 tests ✓
```

---

## Hardware

**Target Platform**: Raspberry Pi 5

| Component | Specification |
|-----------|---------------|
| CPU | ARM Cortex-A76 (4 cores @ 2.4GHz) |
| RAM | 4GB / 8GB LPDDR4X |
| AI | Hailo-8L NPU (optional) |
| Input | Steno machine OR standard keyboard (English mode) |

---

## Philosophy

### 1. Strokes, Not Characters (Internally)
The native input unit is a 23-bit stroke pattern. No character encoding. No Unicode. No string handling **in the kernel core**. However, the English I/O layer provides a natural language interface for universal accessibility.

### 2. Pure Rust
No libc. No C dependencies. Minimal crates. Everything from scratch in safe, idiomatic Rust.

### 3. Green Computing
`wfi` when idle. No busy loops. No wasted cycles.

### 4. Forward Only
We build the future. No backward compatibility with character-based systems.

### 5. Universal Accessibility ✨ NEW!
Steno-native kernel with natural language translation layer. Everyone can use English; power users can use raw strokes.

---

## Documentation

| Document | Description |
|----------|-------------|
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | System design and data flow |
| [ENHANCEMENTS.md](docs/ENHANCEMENTS.md) | ✨ Recent enhancements (Hailo, SMP, Storage, Networking) |
| [ENGLISH_LAYER.md](docs/ENGLISH_LAYER.md) | Natural language I/O system |
| [API.md](docs/API.md) | Complete API reference |
| [ROADMAP.md](docs/ROADMAP.md) | Development phases |
| [BUILDING.md](docs/BUILDING.md) | Build instructions |
| [CONTRIBUTING.md](docs/CONTRIBUTING.md) | How to contribute |

---

## License

MIT License. See [LICENSE](LICENSE) for details.

---

<p align="center">
  <strong>Intent Kernel</strong><br>
  <em>150 years of stenography meets bare-metal computing.</em>
</p>
