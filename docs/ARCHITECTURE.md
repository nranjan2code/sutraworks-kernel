# Intent Kernel Architecture

## Overview

Intent Kernel is a bare-metal stenographic operating system where **steno strokes are the native semantic unit**.

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Steno Machine  │────▶│  Stroke (23-bit)│────▶│   Dictionary    │────▶│    Executor     │
│   (USB HID)     │     │                 │     │  (Stroke→Intent)│     │                 │
└─────────────────┘     └─────────────────┘     └─────────────────┘     └─────────────────┘
         │                                               │
         │                                               │
┌────────▼────────┐                             ┌────────▼────────┐
│  UART/Keyboard  │────▶ English Text ─────────▶│ Reverse Lookup  │
│  (Fallback)     │      "help" → PH-FPL        │                 │
└─────────────────┘                             └─────────────────┘
```

No characters. No words. No parsing. Pure stroke→intent mapping.

**Dual Input Mode**: Users can input strokes directly (steno notation) or type English commands.
English input is internally converted to steno strokes via reverse dictionary lookup.
The kernel remains steno-native—English is just a convenience layer.

---

## Stenographic Input

### Stroke Representation

A stroke is a 23-bit pattern representing simultaneous key presses on a steno machine:

```rust
pub struct Stroke {
    pub bits: u32,  // 23 bits used
}
```

### Plover Key Layout

The kernel uses the standard Plover layout:

```
Position:  0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15  16  17  18  19  20  21  22
Key:       #  S-  T-  K-  P-  W-  H-  R-  A-  O-  *  -E  -U  -F  -R  -P  -B  -L  -G  -T  -S  -D  -Z
```

### RTFCRE Notation

Strokes are written in RTFCRE (Real-Time Format for Court Reporting Equipment) notation:

| Pattern | RTFCRE | Meaning |
|---------|--------|---------|
| Keys before vowels | Left side | Initial consonants |
| A, O | Vowels | Vowel sounds |
| * | Asterisk | Modifier/toggle |
| E, U | Vowels | Vowel sounds |
| Keys after vowels | Right side | Final consonants |

Examples:
- `KAT` → K- + A- + -T (the word "cat")
- `HELP` → H- + -E + -L + -P
- `STPH` → S- + T- + P- + H- (the brief "in")

---

## Dictionary System

### Structure

```rust
pub struct StenoDictionary {
    entries: BTreeMap<StrokeSequence, DictEntry>,
}

pub struct DictEntry {
    pub strokes: StrokeSequence,
    pub concept: ConceptID,
    pub name: &'static str,
}
```

### Stroke Sequences

Single strokes map to concepts:
```
HELP → concepts::HELP
STAT → concepts::STATUS
```

Multi-stroke sequences:
```
SHUT/TKAOPB → concepts::SHUTDOWN
```

### ConceptID

A 64-bit semantic identifier:

```rust
pub struct ConceptID(pub u64);

// Namespace layout:
// 0x0001_xxxx - System concepts
// 0x0002_xxxx - Display concepts
// 0x0003_xxxx - Memory concepts
// 0x0004_xxxx - Device concepts
// 0x0008_xxxx - User concepts
```

---

## Intent Execution

### Intent Structure

```rust
pub struct Intent {
    pub concept: ConceptID,
    pub data: IntentData,
    pub confidence: u8,
    pub name: &'static str, // Human-readable name (e.g., "SAVE")
}

pub enum IntentData {
    None,
    Number(u64),
    Text(&'static str),
    Stroke(Stroke),
}
```

### Executor

The `IntentExecutor` handles intents with capability checks and user-defined handlers:

```rust
pub struct IntentExecutor {
    display_cap: Option<Capability>,
    memory_cap: Option<Capability>,
    system_cap: Option<Capability>,
    compute_cap: Option<Capability>,
    handlers: HandlerRegistry,  // User-defined handlers
    queue: IntentQueue,          // Deferred execution
}

impl IntentExecutor {
    pub fn execute(&self, intent: &Intent) {
        // Try user handlers first
        if self.handlers.dispatch(intent, has_cap) {
            return;
        }
        // Fall back to built-in handlers
        // ...
    }
    
    pub fn register_handler(&mut self, concept_id: ConceptID, handler: HandlerFn, name: &'static str);
    pub fn queue_intent(&mut self, intent: Intent, timestamp: u64);
}
```

---

## Steno Engine

### State Machine

```rust
pub struct StenoEngine {
    dictionary: StenoDictionary,
    stroke_buffer: StrokeSequence,
    history: StrokeHistory,
    state: EngineState,
    stats: EngineStats,
    timestamp: u64,
}

pub enum EngineState {
    Ready,
    Pending,       // Building multi-stroke sequence
    Error,
}
```

### Stroke History

64-entry ring buffer for undo/redo and context:

```rust
pub struct StrokeHistory {
    entries: [HistoryEntry; 64],
    head: usize,
    count: usize,
    undo_cursor: usize,
}

impl StrokeHistory {
    pub fn push(&mut self, stroke: Stroke, intent: Option<&Intent>, timestamp: u64);
    pub fn undo(&mut self) -> Option<&HistoryEntry>;
    pub fn redo(&mut self) -> Option<&HistoryEntry>;
}
```

### Processing Flow

1. **Receive Stroke**: From USB HID or test input
2. **Accumulate**: Add to pending sequence
3. **Lookup**: Check dictionary for match
4. **Resolve**: If match found, emit Intent; otherwise continue accumulating
5. **Timeout**: If no match after timeout, emit partial or error

### Traits

```rust
pub trait StrokeProducer {
    fn next_stroke(&mut self) -> Option<Stroke>;
}

pub trait IntentConsumer {
    fn consume(&mut self, intent: Intent);
}
```

---

## User-Defined Handlers

### Handler Registry

128 handlers with priority-based dispatch:

```rust
pub struct HandlerRegistry {
    handlers: [HandlerEntry; 128],
    count: usize,
}

pub struct HandlerEntry {
    concept_id: ConceptID,       // 0 = wildcard
    required_cap: Option<CapabilityType>,
    handler: HandlerFn,
    priority: u8,                // Higher runs first
    name: &'static str,
}

pub type HandlerFn = fn(&Intent) -> HandlerResult;
```

### Handler Results

```rust
pub enum HandlerResult {
    Handled,     // Intent was processed
    NotHandled,  // Pass to next handler
    Error(u32),  // Handler failed
}
```

### Registration Example

```rust
intent::register_handler(
    concepts::STATUS,
    my_status_handler,
    "custom_status"
);
```

---

## Intent Queue

### Priority Queue

32-entry queue for deferred intent execution:

```rust
pub struct IntentQueue {
    entries: [QueuedIntent; 32],
    count: usize,
}

pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}
```

### Deadline Support

Intents can have deadlines; expired intents are automatically pruned:

```rust
pub struct QueuedIntent {
    intent: Intent,
    priority: Priority,
    sequence: u64,     // For FIFO within priority
    queued_at: u64,
    deadline: u64,     // 0 = no deadline
}
```

---

## Capability Security

### Model

Every operation requires a capability token:

```rust
pub enum CapabilityType {
    System,    // System operations (shutdown, etc.)
    Memory,    // Memory allocation
    Device,    // Hardware access
    Network,   // Network operations
}

pub struct Capability {
    cap_type: CapabilityType,
    permissions: u32,
}
```

### Checking

```rust
impl IntentExecutor {
    fn handle_shutdown(&self) -> Result<(), &'static str> {
        if !self.capabilities.has(CapabilityType::System) {
            return Err("Permission denied: requires System capability");
        }
        // ... perform shutdown
        Ok(())
    }
}
```

---

## Memory Architecture

### Neural Memory

Memory regions are associated with ConceptIDs:

```rust
pub struct NeuralRegion {
    concept: ConceptID,
    base: usize,
    size: usize,
}

impl NeuralAllocator {
    pub fn alloc(&mut self, concept: ConceptID, size: usize) -> Option<*mut u8>;
    pub fn retrieve(&self, concept: ConceptID) -> Option<&NeuralRegion>;
}
```

### Allocation

Memory is allocated based on semantic concepts, not raw addresses:

```rust
// Allocate memory for the "status" concept
let ptr = allocator.alloc(concepts::STATUS, 4096)?;
```

---

## Scheduler

### Round-Robin

Simple round-robin scheduling without priority:

```rust
pub struct Scheduler {
    agents: Vec<Agent>,
    current: usize,
}

impl Scheduler {
    pub fn spawn_simple(&mut self, name: &'static str, entry: fn());
    pub fn tick(&mut self);
}
```

### Async Support

The kernel uses async/await for I/O:

```rust
pub async fn steno_loop() {
    loop {
        // Wait for stroke from hardware
        let stroke = steno::wait_stroke().await;
        
        // Process through dictionary
        if let Some(intent) = steno::process_stroke(stroke) {
            intent::execute(&intent);
        }
    }
}
```

---
Perception Layer

The Perception Layer bridges the gap between raw sensor data and semantic intent.

### Perception Manager
Automatically detects available hardware acceleration:
- **Hailo-8**: Uses the NPU for high-speed object detection (26 TOPS).
- **CPU Fallback**: Uses `ColorBlobDetector` (optimized software routine) to detect objects by color when no NPU is present.

### Heads-Up Display (HUD)
A visual interface that renders directly to the framebuffer (no window manager).
- **Steno Tape**: Visualizes the raw stroke stream (RTFCRE).
- **Intent Stream**: Visualizes recognized intents.
- **Status Bar**: Shows system health and statistics.

```rust
pub struct Hud {
    tape_log: TextLog<20>,
    intent_log: TextLog<20>,
}
```

---

## 
## Hardware Drivers

### UART

Debug output via PL011 UART:

```rust
pub fn uart_write(s: &str);
pub fn uart_read() -> Option<u8>;
```

### Timer

ARM generic timer for scheduling:

```rust
pub fn timer_init();
pub fn timer_tick() -> u64;
pub async fn timer_sleep(ms: u64);
```

### GPIO

Pin control for LEDs and sensors:

```rust
pub fn gpio_set(pin: u32, value: bool);
pub fn gpio_get(pin: u32) -> bool;
```

### USB HID ✅

Steno machine input via USB Host Controller:

```rust
pub struct UsbHid {
    controller: XhciController,
    device: HidDevice,
}

impl UsbHid {
    pub fn poll(&mut self) -> Option<Stroke>;
    pub fn supports_nkro(&self) -> bool;
}

impl StrokeProducer for UsbHid {
    fn next_stroke(&mut self) -> Option<Stroke> {
        self.poll()
    }
}
```

Supported devices:
- Georgi (GBoards)
- Uni (Plover HID Protocol)
- Any HID-compliant steno machine

### Framebuffer Console ✅

Text output on HDMI display:

```rust
pub struct Console {
    framebuffer: &'static mut Framebuffer,
    cursor_x: u32,
    cursor_y: u32,
}

impl Console {
    pub fn print(&mut self, s: &str);
    pub fn clear(&mut self);
}
```

Macros: `cprint!()`, `cprintln!()`

---

## Boot Sequence

1. **Reset**: ARM starts at `_start` in `boot.s`
2. **EL2→EL1**: Drop from hypervisor to kernel mode
3. **BSS Clear**: Zero uninitialized memory
4. **Stack Setup**: Initialize kernel stack
5. **Rust Entry**: Jump to `kernel_main()`
6. **Init Drivers**: UART, Timer, GPIO
7. **Init Steno**: Load dictionary, start engine
8. **Main Loop**: Process strokes forever

---

## Non-Goals

These are explicitly NOT part of the architecture:

- ❌ NLP or tokenization (English input uses direct dictionary lookup)
- ❌ Embedding vectors or similarity search
- ❌ Traditional shell/terminal
- ❌ POSIX compatibility
- ❌ Backward compatibility with word-based systems

**Note**: English text input is supported as a convenience layer, but internally
all commands are converted to steno strokes. The kernel never parses English—it
only looks up the stroke corresponding to an English word via reverse dictionary.
