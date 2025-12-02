# Intent Kernel Architecture

## Overview

Intent Kernel is a bare-metal stenographic operating system where **steno strokes are the native semantic unit**.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        USER INPUT (Multiple Modes)                       │
│                                                                          │
│  Steno Machine (USB HID) ────▶ Direct Strokes                          │
│  Standard Keyboard       ────▶ Natural English ✨ NEW!                  │
└─────────────────────────────────────────────────────────────────────────┘
                                      ↓
┌─────────────────────────────────────────────────────────────────────────┐
│                    ENGLISH I/O LAYER (Optional Translation)              │
│                                                                          │
│  Input:  English Text → Parser → ConceptID                             │
│          • 200+ phrase variations                                       │
│          • 50+ synonym expansions                                       │
│          • Multi-stage parsing pipeline                                 │
│                                                                          │
│  Output: Intent Result → Template Engine → Natural Language             │
│          • Context-aware responses                                      │
│          • User mode adaptation (Beginner/Advanced)                     │
└─────────────────────────────────────────────────────────────────────────┘
                                      ↓
┌─────────────────────────────────────────────────────────────────────────┐
│                      STENO-NATIVE KERNEL CORE                           │
│                                                                          │
│  Stroke (23-bit) → Dictionary → Intent → Executor                       │
│  Pure semantic processing. No character handling.                       │
└─────────────────────────────────────────────────────────────────────────┘
```

**Kernel Philosophy**: Steno-native core with optional English translation layer.

- **Steno Mode**: Direct stroke → intent (0.1μs, maximum performance)
- **English Mode**: Natural language → intent (~30μs, universal accessibility)
- **Hybrid Mode**: Mix both freely (power users)

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
 
 Steno machine input via USB Host Controller (xHCI 1.2):
 
 ```rust
 pub struct XhciController {
     base_addr: usize,
     dcbaa: Option<NonNull<u8>>,      // Device Context Base Address Array
     cmd_ring: Option<NonNull<u8>>,   // Command Ring
     event_ring: Option<NonNull<u8>>, // Event Ring (Interrupter 0)
 }
 
 pub struct UsbHid {
     // HID Boot Protocol Parser
     pub fn parse_report(&self, report: &KeyboardReport) -> Option<Stroke>;
 }
 ```
 
 **Features**:
 - **Real xHCI Initialization**: Reset, Ring allocation, Interrupt setup.
 - **DMA Memory**: Uses `alloc_dma` for physically contiguous buffers.
 - **Boot Protocol**: Parses standard 8-byte Keyboard Reports.
 - **Key Mapping**: Maps QWERTY keys to Steno layout (Plover standard).

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

## English I/O Layer ✨ NEW!

The English I/O Layer provides natural language accessibility while maintaining the steno-native kernel core.

### Architecture

```rust
pub mod english {
    pub mod phrases;      // 200+ phrase → ConceptID mappings
    pub mod synonyms;     // 50+ synonym expansions
    pub mod parser;       // Multi-stage parsing pipeline
    pub mod responses;    // Natural language generation
    pub mod context;      // Conversation state management
}
```

### Input Pipeline

**Multi-Stage Parsing**:

1. **Normalization**: Lowercase, trim whitespace
2. **Exact Phrase Match**: Check 200+ phrase database
3. **Synonym Expansion**: Expand contractions and synonyms
4. **Keyword Extraction**: Natural language understanding
5. **Steno Fallback**: Try as raw steno notation

**Example**:

```rust
// Stage 2: Exact match
"help" → HELP intent (confidence: 1.0)

// Stage 3: Synonym expansion
"show sys info" → "show system information" → STATUS intent (0.95)

// Stage 4: Keyword extraction
"can you help me?" → extract["help"] → HELP intent (0.9)

// Stage 5: Steno fallback
"STAT" → parse as steno → STATUS intent (1.0)
```

### Output Generation

**Template-Based Responses**:

```rust
pub struct ResponseGenerator {
    pub verbose: bool,  // Adapts to user mode
}

impl ResponseGenerator {
    pub fn generate(&self, intent: &Intent, result: &IntentResult) -> String;
}
```

**Example Responses**:

```
Concise (Advanced): "CPU 45% | RAM 2.3GB | Up 3h"
Verbose (Beginner): "System Status: CPU 45%, Memory 2.3GB/8GB, Uptime 3h 42m"
```

### Conversation Context

**Stateful Understanding**:

```rust
pub struct ConversationContext {
    last_intent: Option<ConceptID>,
    last_result: Option<IntentResult>,
    mode: UserMode,  // Beginner/Intermediate/Advanced
    history: Vec<HistoryEntry>,
}
```

**Features**:
- Follow-up questions: "show it again", "more details"
- Pronoun resolution: "hide it", "show that"
- Auto-upgrade user mode based on usage
- Conversation history (last 10 commands)

### Performance

**Overhead Analysis**:
- Phrase lookup: ~5-10μs (linear search of 200 entries)
- Synonym expansion: ~5μs
- Template generation: ~10-20μs
- **Total**: ~30μs per command

**At 200 WPM** (3.3 commands/sec): 0.0001% CPU

**Steno Bypass**: Power users can bypass English layer entirely (0.1μs direct)

### Integration Example

```rust
use intent_kernel::english;

// Parse natural English
let intent = english::parse("show me system status");

// Execute (kernel core - unchanged)
let result = intent::execute_with_result(&intent.unwrap());

// Generate natural response
let response = english::generate_response(&intent.unwrap(), &result);
println!("{}", response);
// Output: "System: CPU 45%, RAM 2.3GB, Uptime 3h 42m..."
```

### See Also

For complete documentation, see [ENGLISH_LAYER.md](ENGLISH_LAYER.md).

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

- ❌ Deep NLP or LLM-based parsing (English layer uses phrase matching + keyword extraction)
- ❌ Embedding vectors or similarity search (direct lookup only)
- ❌ Traditional shell/terminal
- ❌ POSIX compatibility
- ❌ Backward compatibility with word-based systems

**Note**: English text input is supported as a convenience layer, but internally
all commands are converted to steno strokes. The kernel never parses English—it
only looks up the stroke corresponding to an English word via reverse dictionary.
