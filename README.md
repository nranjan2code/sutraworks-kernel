# Intent Kernel

A bare-metal **stenographic operating system** for Raspberry Pi 5.

## What Is This?

Intent Kernel is an OS where **steno strokes are the native input unit**. Not characters. Not words. Strokes.

A stenographer using a steno machine sends 23-bit stroke patterns directly to the kernel. The kernel maps strokes to **intents** via a dictionary, then executes them.

```
Steno Machine → Stroke (23-bit) → Dictionary → Intent → Executor
```

No parsing. No tokenization. No NLP. Pure stroke→intent mapping.

## Why?

Traditional operating systems treat human input as characters. You type "show status", the shell parses it, looks up a command, runs it.

Intent Kernel skips all that. A single stroke like `STAT` maps directly to the STATUS concept. The kernel knows what you mean instantly.

This is **faster** (no parsing), **cleaner** (no string handling), and **more powerful** (strokes can represent complex concepts atomically).

## Stenographic Architecture

The kernel uses the **Plover steno layout** (23 keys):

```
#  S- T- K- P- W- H- R-  A- O-  *  -E -U  -F -R -P -B -L -G -T -S -D -Z
0  1  2  3  4  5  6  7   8  9  10  11 12  13 14 15 16 17 18 19 20 21 22
```

Each stroke is a 23-bit pattern. The kernel stores strokes as `u32` internally.

### Example Strokes

| Stroke | RTFCRE | Concept | Action |
|--------|--------|---------|--------|
| `HELP` | H-E-L-P | HELP | Show help |
| `STAT` | S-T-A-T | STATUS | System status |
| `PHOR` | P-H-O-R | DISPLAY | Show display |
| `*` | * | UNDO | Undo last action |

### Multi-Stroke Sequences

Some concepts require multiple strokes:

```
SHUT/TKAOPB → SHUTDOWN
```

The engine tracks stroke sequences and matches the longest prefix.

## Building

```bash
# Build the kernel
make kernel

# Build bootable image
make image

# Run in QEMU
make run

# Check for errors
make check
```

### Requirements

- Rust nightly (see `rust-toolchain.toml`)
- `aarch64-unknown-none` target
- QEMU for testing (optional)

## Project Structure

```
kernel/src/
├── steno/           # Stenographic engine
│   ├── stroke.rs    # Stroke struct (23-bit)
│   ├── dictionary.rs # Stroke→Intent mapping
│   ├── engine.rs    # StenoEngine state machine
│   └── history.rs   # Stroke history (undo/redo)
├── intent/          # Intent execution
│   ├── mod.rs       # ConceptID, Intent, IntentExecutor
│   ├── handlers.rs  # User-defined handler registry
│   └── queue.rs     # Intent priority queue
├── drivers/         # Hardware drivers
│   ├── uart.rs      # Debug output
│   ├── timer.rs     # ARM generic timer
│   └── gpio.rs      # GPIO pins
└── kernel/          # Core subsystems
    ├── scheduler.rs # Process scheduler
    ├── capability.rs # Security model
    └── memory/      # Memory management
```

## Core Concepts

### ConceptID

A 64-bit identifier for semantic concepts:

```rust
pub struct ConceptID(pub u64);

// System concepts (0x0001_xxxx)
pub const HELP: ConceptID = ConceptID(0x0001_0001);
pub const STATUS: ConceptID = ConceptID(0x0001_0002);
pub const SHUTDOWN: ConceptID = ConceptID(0x0001_0003);
```

### Intent

The result of stroke processing:

```rust
pub struct Intent {
    pub concept: ConceptID,
    pub data: IntentData,
    pub confidence: u8,
}
```

### Capability

Security tokens that grant permissions:

```rust
pub enum CapabilityType {
    System,    // System-level operations
    Memory,    // Memory allocation
    Device,    // Hardware access
    Network,   // Network operations
}
```

## Usage Example

```rust
// Initialize the steno engine
steno::init();

// Process a stroke from RTFCRE notation
if let Some(intent) = steno::process_steno("HELP") {
    intent::execute(&intent);
}

// Process raw stroke bits from hardware
if let Some(intent) = steno::process_raw(0x000042) {
    intent::execute(&intent);
}

// Register a custom intent handler
intent::register_handler(
    concepts::STATUS,
    |intent| {
        kprintln!("Custom status handler!");
        HandlerResult::Handled
    },
    "my_status"
);

// Queue an intent for later execution
intent::queue_with_priority(
    Intent::new(concepts::SAVE),
    Priority::High,
    timer::uptime_us()
);
```

## Testing

```bash
# Run 122 host-based tests (< 1 second)
make test

# Modules tested:
# - stroke (25 tests)
# - capability (20 tests)
# - dictionary (20 tests)
# - concept (22 tests)
# - history (12 tests)
# - queue (12 tests)
# - handlers (11 tests)
```

## Development Status

| Phase | Status | Description |
|-------|--------|-------------|
| 1. Foundation | ✅ | Boot, UART, GPIO, Memory, Scheduler |
| 2. Steno Engine | ✅ | Stroke processing, Dictionary, Engine |
| 3. Intent Execution | ✅ | Handlers, Queue, History, 122 tests |
| 4. Hardware | 🔄 | USB HID, Framebuffer, Camera |
| 5. Connectivity | ⏳ | Networking, Multi-core, Storage |

## Hardware

**Target**: Raspberry Pi 5

- ARM Cortex-A76 (4 cores)
- 4/8 GB RAM
- Hailo-8L AI accelerator (optional)

**Steno Input**: Any Plover-compatible steno machine via USB HID.

## Philosophy

1. **Strokes, Not Characters**: The native input unit is a steno stroke.
2. **Pure Rust**: No libc, minimal crates, everything from scratch.
3. **Green Computing**: Sleep when idle, no busy loops.
4. **No Backward Compatibility**: We build the future, not preserve the past.

## License

MIT

## Contributing

See [CONTRIBUTING.md](docs/CONTRIBUTING.md).
