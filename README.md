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

### Intent Kernel (English Mode)
```
Keyboard → English Word → Reverse Lookup → Stroke → Intent → Execute
```

**Faster. Cleaner. More powerful.**

Users who don't know steno can type English commands. The kernel internally
converts them to strokes, maintaining the steno-native architecture.

---

## Architecture

### The Stroke

Every steno chord produces a 23-bit pattern representing which keys were pressed:

```
Position:  0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15  16  17  18  19  20  21  22
Key:       #  S-  T-  K-  P-  W-  H-  R-  A-  O-  *  -E  -U  -F  -R  -P  -B  -L  -G  -T  -S  -D  -Z
```

### The Dictionary

Strokes map to **concepts**, not text:

| Stroke | Notation | Concept | Action |
|--------|----------|---------|--------|
| `0x42` | `STAT` | STATUS | Display system status |
| `0x400` | `*` | UNDO | Undo last action |
| `0x1A4` | `HELP` | HELP | Show help |
| `0x...` | `SHRO` | SHOW | Display something |

### The Intent

```rust
pub struct Intent {
    pub concept_id: ConceptID,  // What to do
    pub confidence: f32,         // How certain
    pub data: IntentData,        // Parameters
}
```

### The Flow

```rust
// A stroke comes in from hardware
let stroke = Stroke::from_raw(0x42);

// The engine processes it
if let Some(intent) = steno::process_stroke(stroke) {
    // The executor acts on it
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

### ✅ Dual Input Mode
Use steno strokes OR English text:

```rust
// Steno notation
steno::process_steno("PH-FPL");  // HELP stroke

// English text (reverse lookup)
steno::process_english("help");   // Finds PH-FPL, executes HELP intent
```

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
│   ├── intent/             # Intent execution
│   │   ├── mod.rs          # Core types
│   │   ├── handlers.rs     # User handler registry
│   │   └── queue.rs        # Priority queue
│   ├── perception/         # Adaptive Perception Layer
│   │   ├── mod.rs          # Perception Manager
│   │   ├── vision.rs       # Computer Vision (Hailo/CPU)
│   │   └── hud.rs          # Heads-Up Display
│   ├── drivers/            # Hardware
│   │   ├── uart.rs         # Serial I/O
│   │   ├── timer.rs        # ARM timer
│   │   ├── gpio.rs         # Pin control
│   │   ├── framebuffer.rs  # VideoCore display
│   │   ├── console.rs      # Text console on framebuffer
│   │   └── usb/            # USB Host Controller
│   │       ├── xhci.rs     # xHCI driver
│   │       └── hid.rs      # HID protocol (steno machines)
│   └── kernel/             # Core OS
│       ├── capability.rs   # Security
│       ├── scheduler.rs    # Process management
│       └── memory/         # Allocation
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
| **4. Perception** | ✅ | Hailo-8 detection, Heads-Up Display (HUD) |
| **5. Input/Output** | ✅ | USB HID, Framebuffer Console, English Input |
| **6. Sensors** | 🔄 | Camera Driver (In Progress) |
| **7. Connectivity** | ⏳ | Networking, Multi-core |

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
| Input | Any Plover-compatible steno machine |

---

## Philosophy

### 1. Strokes, Not Characters
The native input unit is a 23-bit stroke pattern. No character encoding. No Unicode. No string handling.

### 2. Pure Rust
No libc. No C dependencies. Minimal crates. Everything from scratch in safe, idiomatic Rust.

### 3. Green Computing
`wfi` when idle. No busy loops. No wasted cycles.

### 4. Forward Only
We build the future. No backward compatibility with character-based systems.

---

## Documentation

| Document | Description |
|----------|-------------|
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | System design and data flow |
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
