# Intent Kernel Architecture

## Overview

Intent Kernel is a bare-metal stenographic operating system where **steno strokes are the native semantic unit**.

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Steno Machine  │────▶│  Stroke (23-bit)│────▶│   Dictionary    │────▶│    Executor     │
│   (USB HID)     │     │                 │     │  (Stroke→Intent)│     │                 │
└─────────────────┘     └─────────────────┘     └─────────────────┘     └─────────────────┘
```

No characters. No words. No parsing. Pure stroke→intent mapping.

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
}

pub enum IntentData {
    None,
    Number(u64),
    Text(&'static str),
    Stroke(Stroke),
}
```

### Executor

The `IntentExecutor` handles intents with capability checks:

```rust
pub struct IntentExecutor {
    capabilities: CapabilitySet,
}

impl IntentExecutor {
    pub fn execute(&self, intent: &Intent) -> Result<(), &'static str> {
        match intent.concept {
            concepts::HELP => self.handle_help(),
            concepts::STATUS => self.handle_status(),
            concepts::SHUTDOWN => self.handle_shutdown(),
            _ => self.handle_unknown(intent),
        }
    }
}
```

---

## Steno Engine

### State Machine

```rust
pub struct StenoEngine {
    dictionary: StenoDictionary,
    state: EngineState,
    pending: StrokeSequence,
    stats: EngineStats,
}

pub enum EngineState {
    Idle,
    Accumulating,  // Building multi-stroke sequence
    Error,
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

### USB HID (Planned)

Steno machine input via USB:

```rust
pub trait StenoHID {
    fn poll(&mut self) -> Option<Stroke>;
}
```

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

- ❌ Character/word parsing
- ❌ NLP or tokenization
- ❌ Embedding vectors or similarity search
- ❌ Traditional shell/terminal
- ❌ POSIX compatibility
- ❌ Backward compatibility with word-based systems
