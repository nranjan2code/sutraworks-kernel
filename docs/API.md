# API Reference

Complete API documentation for Intent Kernel modules.

---

## Table of Contents

- [Architecture (`arch`)](#architecture-arch)
- [Drivers](#drivers)
  - [UART](#uart)
  - [GPIO](#gpio)
  - [Timer](#timer)
  - [Interrupts](#interrupts)
  - [Mailbox](#mailbox)
  - [Framebuffer](#framebuffer)
- [Kernel Subsystems](#kernel-subsystems)
  - [Memory Allocator](#memory-allocator)
  - [Capability System](#capability-system)
- [Intent Engine](#intent-engine)

---

## Architecture (`arch`)

Low-level CPU and synchronization primitives.

### `SpinLock<T>`

A spinlock providing mutual exclusion with guard-based RAII pattern.

```rust
pub struct SpinLock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}
```

#### Methods

##### `SpinLock::new(data: T) -> Self`

Create a new spinlock protecting the given data.

```rust
let lock = SpinLock::new(0u32);
```

##### `SpinLock::lock(&self) -> SpinLockGuard<T>`

Acquire the lock, spinning until successful. Returns a guard that releases the lock on drop.

```rust
let guard = lock.lock();
*guard = 42;
// Lock released when guard drops
```

##### `SpinLock::try_lock(&self) -> Option<SpinLockGuard<T>>`

Attempt to acquire the lock without spinning. Returns `None` if already locked.

```rust
if let Some(guard) = lock.try_lock() {
    *guard = 42;
}
```

##### `SpinLock::force_unlock(&self)`

Force release the lock. **Unsafe** - only use for error recovery.

```rust
lock.force_unlock();
```

---

### Memory Barriers

#### `dmb()`

Data Memory Barrier - ensures all explicit memory accesses before the DMB complete before any after.

```rust
arch::dmb();
```

#### `dsb()`

Data Synchronization Barrier - ensures all memory accesses complete before continuing.

```rust
arch::dsb();
```

#### `isb()`

Instruction Synchronization Barrier - flushes the pipeline.

```rust
arch::isb();
```

---

### CPU Operations

#### `current_el() -> u64`

Return the current Exception Level (1, 2, or 3).

```rust
let el = arch::current_el();
assert!(el == 1); // Kernel runs at EL1
```

#### `halt() -> !`

Halt the CPU in a low-power state. Never returns.

```rust
arch::halt(); // CPU stops here
```

#### `nop()`

No operation - useful for timing loops.

```rust
arch::nop();
```

---

### Interrupt Control

#### `enable_interrupts()`

Enable CPU interrupts (clear DAIF mask).

```rust
arch::enable_interrupts();
```

#### `disable_interrupts()`

Disable CPU interrupts (set DAIF mask).

```rust
arch::disable_interrupts();
```

#### `interrupts_enabled() -> bool`

Check if interrupts are currently enabled.

```rust
if arch::interrupts_enabled() {
    // Interrupts are enabled
}
```

---

### Early Initialization

#### `early_init()`

Perform early CPU initialization. Called once at boot.

```rust
arch::early_init();
```

---

## Drivers

### UART

PL011 UART driver for serial communication.

#### Module: `drivers::uart`

---

#### `Uart`

```rust
pub struct Uart {
    base: usize,
}
```

##### `Uart::new() -> Self`

Create UART instance using default base address.

```rust
let uart = Uart::new();
```

##### `Uart::init(&self)`

Initialize UART at 115200 baud.

```rust
uart.init();
```

##### `Uart::putc(&self, c: u8)`

Send a single byte.

```rust
uart.putc(b'A');
```

##### `Uart::puts(&self, s: &str)`

Send a string.

```rust
uart.puts("Hello, World!\n");
```

##### `Uart::getc(&self) -> u8`

Receive a byte (blocking).

```rust
let byte = uart.getc();
```

##### `Uart::try_getc(&self) -> Option<u8>`

Try to receive a byte (non-blocking).

```rust
if let Some(byte) = uart.try_getc() {
    // Process byte
}
```

##### `Uart::read_line(&self, buffer: &mut [u8]) -> usize`

Read a line into buffer, returns length.

```rust
let mut buf = [0u8; 256];
let len = uart.read_line(&mut buf);
```

---

#### Global Functions

##### `uart::init()`

Initialize the global UART instance.

##### `uart::puts(s: &str)`

Print string to global UART.

##### `uart::putc(c: u8)`

Print character to global UART.

##### `uart::getc() -> u8`

Read character from global UART.

---

### GPIO

GPIO pin control driver.

#### Module: `drivers::gpio`

---

#### `Gpio`

```rust
pub struct Gpio {
    base: usize,
}
```

##### `Gpio::new() -> Self`

Create GPIO controller instance.

##### `Gpio::set_function(&self, pin: u32, func: GpioFunction)`

Set pin function (input, output, alt0-5).

```rust
gpio.set_function(14, GpioFunction::Alt0); // UART TX
```

##### `Gpio::set_output(&self, pin: u32, high: bool)`

Set output pin state.

```rust
gpio.set_output(42, true); // LED on
```

##### `Gpio::get_input(&self, pin: u32) -> bool`

Read input pin state.

```rust
let pressed = gpio.get_input(17);
```

##### `Gpio::set_pull(&self, pin: u32, pull: GpioPull)`

Set pull-up/down resistor.

```rust
gpio.set_pull(17, GpioPull::Up);
```

---

#### `GpioFunction` Enum

```rust
pub enum GpioFunction {
    Input  = 0b000,
    Output = 0b001,
    Alt0   = 0b100,
    Alt1   = 0b101,
    Alt2   = 0b110,
    Alt3   = 0b111,
    Alt4   = 0b011,
    Alt5   = 0b010,
}
```

---

#### `GpioPull` Enum

```rust
pub enum GpioPull {
    None = 0b00,
    Up   = 0b01,
    Down = 0b10,
}
```

---

#### `Pin` (High-level API)

```rust
pub struct Pin {
    pub number: u32,
}
```

##### `Pin::new(number: u32) -> Self`

Create a pin reference.

##### `Pin::as_output(&self) -> OutputPin`

Configure as output.

##### `Pin::as_input(&self) -> InputPin`

Configure as input.

---

### Timer

ARM Generic Timer driver.

#### Module: `drivers::timer`

---

#### `Timer`

##### `Timer::new() -> Self`

Create timer instance.

##### `Timer::init(&self)`

Initialize the timer subsystem.

##### `Timer::get_counter(&self) -> u64`

Get current counter value.

##### `Timer::get_frequency(&self) -> u64`

Get counter frequency (Hz).

##### `Timer::delay_us(&self, us: u64)`

Delay for microseconds.

```rust
timer.delay_us(1000); // 1ms delay
```

##### `Timer::delay_ms(&self, ms: u64)`

Delay for milliseconds.

```rust
timer.delay_ms(500); // 500ms delay
```

##### `Timer::set_timeout(&self, us: u64)`

Set timer interrupt after microseconds.

##### `Timer::clear_interrupt(&self)`

Clear timer interrupt flag.

---

#### Global Functions

##### `timer::init()`

Initialize global timer.

##### `timer::delay_us(us: u64)`

Global microsecond delay.

##### `timer::delay_ms(ms: u64)`

Global millisecond delay.

##### `timer::uptime_us() -> u64`

Get uptime in microseconds.

---

### Interrupts

GIC-400 interrupt controller driver.

#### Module: `drivers::interrupts`

---

#### `InterruptController`

##### `InterruptController::new() -> Self`

Create interrupt controller instance.

##### `InterruptController::init(&self)`

Initialize GIC-400.

##### `InterruptController::enable(&self, irq: u32)`

Enable specific interrupt.

```rust
ic.enable(153); // Enable UART0 interrupt
```

##### `InterruptController::disable(&self, irq: u32)`

Disable specific interrupt.

##### `InterruptController::acknowledge(&self) -> u32`

Acknowledge interrupt, returns IRQ number.

##### `InterruptController::end(&self, irq: u32)`

Signal end of interrupt handling.

##### `InterruptController::set_priority(&self, irq: u32, priority: u8)`

Set interrupt priority (0=highest, 255=lowest).

##### `InterruptController::set_target(&self, irq: u32, cpu_mask: u8)`

Set which CPUs can handle interrupt.

---

#### Handler Registration

##### `register_handler(irq: u32, handler: fn())`

Register interrupt handler.

```rust
interrupts::register_handler(153, uart_handler);
```

##### `handle_irq()`

Main IRQ dispatch function.

---

### Mailbox

VideoCore mailbox interface.

#### Module: `drivers::mailbox`

---

#### `Mailbox`

##### `Mailbox::new() -> Self`

Create mailbox instance.

##### `Mailbox::call(&self, channel: u8, data: &mut [u32]) -> bool`

Make a mailbox call. Data must be 16-byte aligned.

```rust
let mut data = [0u32; 8];
data[0] = 32; // Buffer size
// ... set up tags
if mailbox.call(8, &mut data) {
    // Success
}
```

---

#### Property Tags

##### `get_arm_memory() -> Option<(u32, u32)>`

Get ARM memory base and size.

```rust
if let Some((base, size)) = mailbox::get_arm_memory() {
    // base = 0, size = RAM size
}
```

##### `get_vc_memory() -> Option<(u32, u32)>`

Get VideoCore memory base and size.

##### `get_board_revision() -> Option<u32>`

Get board revision code.

##### `get_temperature() -> Option<u32>`

Get SoC temperature (millidegrees C).

```rust
if let Some(temp) = mailbox::get_temperature() {
    let celsius = temp / 1000;
}
```

##### `set_power_state(device: u32, on: bool) -> bool`

Set device power state.

---

### Framebuffer

Display driver with text rendering.

#### Module: `drivers::framebuffer`

---

#### `Framebuffer`

##### `Framebuffer::new() -> Self`

Create framebuffer instance.

##### `Framebuffer::init(&self, width: u32, height: u32, depth: u32) -> bool`

Initialize framebuffer with resolution and bit depth.

```rust
fb.init(1920, 1080, 32);
```

##### `Framebuffer::clear(&self, color: u32)`

Clear screen with color.

```rust
fb.clear(0x000000); // Black
```

##### `Framebuffer::put_pixel(&self, x: u32, y: u32, color: u32)`

Draw a single pixel.

##### `Framebuffer::draw_rect(&self, x: u32, y: u32, w: u32, h: u32, color: u32)`

Draw filled rectangle.

##### `Framebuffer::draw_char(&self, x: u32, y: u32, c: char, fg: u32, bg: u32)`

Draw character at position.

##### `Framebuffer::print(&self, s: &str)`

Print string at cursor position.

##### `Framebuffer::println(&self, s: &str)`

Print string with newline.

##### `Framebuffer::set_cursor(&self, x: u32, y: u32)`

Set text cursor position.

##### `Framebuffer::scroll(&self)`

Scroll screen up one line.

---

### RNG

Hardware Random Number Generator (TRNG) driver.

#### Module: `drivers::rng`

---

#### Global Functions

##### `rng::init()`

Initialize the hardware RNG.

##### `rng::next_u32() -> u32`

Get a random 32-bit integer.

##### `rng::next_u64() -> u64`

Get a random 64-bit integer.

##### `rng::fill_bytes(buf: &mut [u8])`

Fill a buffer with random bytes.

---

#### Global Functions

##### `framebuffer::init(width: u32, height: u32)`

Initialize global framebuffer.

##### `framebuffer::print(s: &str)`

Print to global framebuffer.

##### `framebuffer::clear()`

Clear global framebuffer.

---

## Kernel Subsystems

### Memory Allocator

Combined buddy and slab allocator.

#### Module: `kernel::memory`

---

#### `MemoryAllocator`

##### `MemoryAllocator::new(start: usize, size: usize) -> Self`

Create allocator managing memory region.

```rust
let alloc = MemoryAllocator::new(0x200000, 0x1_0000_0000);
```

##### `MemoryAllocator::alloc(&mut self, size: usize) -> Option<*mut u8>`

Allocate memory. Returns aligned pointer or None.

```rust
if let Some(ptr) = alloc.alloc(4096) {
    // Use ptr
}
```

##### `MemoryAllocator::alloc_aligned(&mut self, size: usize, align: usize) -> Option<*mut u8>`

Allocate with specific alignment.

```rust
let ptr = alloc.alloc_aligned(64, 64); // Cache-line aligned
```

##### `MemoryAllocator::free(&mut self, ptr: *mut u8)`

Free previously allocated memory.

```rust
alloc.free(ptr);
```

##### `MemoryAllocator::stats(&self) -> MemoryStats`

Get allocation statistics.

```rust
let stats = alloc.stats();
println!("Used: {} bytes", stats.used);
```

---

#### `MemoryStats`

```rust
pub struct MemoryStats {
    pub total: usize,
    pub used: usize,
    pub free: usize,
    pub allocations: usize,
}
```

---

#### DMA Allocation

##### `alloc_dma(size: usize) -> Option<*mut u8>`

Allocate DMA-coherent memory (non-cached, 16-byte aligned).

```rust
let dma_buf = memory::alloc_dma(4096);
```

---

#### Global Functions

##### `memory::init(seed: u64)`

Initialize global memory allocator.

##### `memory::alloc(size: usize) -> Option<*mut u8>`

Allocate from global allocator.

##### `memory::free(ptr: *mut u8)`

Free to global allocator.

---

### Virtual Memory

ARM64 Virtual Memory System Architecture (VMSA) management.

#### Module: `kernel::memory::paging`

---

#### `VMM`

Virtual Memory Manager for creating and managing address spaces.

##### `VMM::new() -> Option<Self>`

Create a new VMM instance with a fresh root page table (Level 0).

```rust
if let Some(vmm) = VMM::new() {
    // VMM created
}
```

##### `VMM::map_page(&mut self, virt: u64, phys: u64, flags: EntryFlags) -> Result<(), &'static str>`

Map a 4KB page.

```rust
vmm.map_page(0x1000, 0x2000, EntryFlags::VALID | EntryFlags::AP_RW_EL1)?;
```

##### `VMM::identity_map(&mut self, start: u64, end: u64, flags: EntryFlags) -> Result<(), &'static str>`

Identity map a physical memory range (Virtual Address = Physical Address).

```rust
vmm.identity_map(0x80000, 0x90000, EntryFlags::ATTR_NORMAL)?;
```

---

#### `EntryFlags`

Bitflags for Page Table Entry attributes.

- `VALID`: Entry is valid
- `TABLE`: Entry points to next level table (or is a page at L3)
- `ATTR_DEVICE`: Device-nGnRnE memory (uncached, ordered)
- `ATTR_NORMAL`: Normal memory (cacheable)
- `AP_RW_EL1`: Read-Write, EL1 only
- `AP_RW_USER`: Read-Write, EL1 & EL0
- `AP_RO_EL1`: Read-Only, EL1 only
- `AP_RO_USER`: Read-Only, EL1 & EL0
- `PXN`: Privileged Execute Never
- `UXN`: Unprivileged Execute Never

---

#### Global Functions

##### `paging::init()`

Initialize the kernel VMM, setup MAIR/TCR, and enable the MMU.

---

### Exception Handling

ARM64 Exception handling and crash reporting.

#### Module: `kernel::exception`

---

#### `ExceptionFrame`

Represents the register state saved on the stack during an exception.

```rust
#[repr(C)]
pub struct ExceptionFrame {
    pub x: [u64; 30],
    pub x30: u64,
    pub elr: u64,
    pub spsr: u64,
    pub esr: u64,
    pub far: u64,
}
```

##### `ExceptionFrame::dump(&self)`

Prints a human-readable dump of the registers to the kernel console.

---

#### `ExceptionClass`

Enum representing the class of exception (decoded from `ESR_EL1.EC`).

- `DataAbortLower/Same`: Memory access violation.
- `InstrAbortLower/Same`: Instruction fetch violation.
- `SVC`: System call.
- `TrappedWFI`: Trapped WFI/WFE instruction.
- `SError`: System Error (asynchronous).

---

#### `DataFaultStatusCode`

Enum representing the specific cause of a Data Abort (decoded from `ESR_EL1.ISS`).

- `TranslationLevelX`: Page not mapped at level X.
- `PermissionLevelX`: Permission violation (e.g. write to RO) at level X.
- `AccessFlagLevelX`: Access flag not set.
- `AlignmentFault`: Unaligned access.

---

### Process Management

Process isolation and scheduling.

#### Module: `kernel::process`

---

#### `Process`

The Process Control Block (PCB).

```rust
pub struct Process {
    pub id: ProcessId,
    pub state: ProcessState,
    pub context: Context,
    pub vmm: Option<VMM>,
    pub kernel_stack: Vec<u8>,
}
```

##### `Process::new_kernel(entry: fn()) -> Self`

Creates a new kernel thread. It shares the kernel address space (TTBR1) but has its own stack and context.

---

#### `ProcessId`

Unique identifier for a process.

```rust
pub struct ProcessId(pub u64);
```

---

#### Module: `kernel::scheduler`

---

#### `Scheduler`

Round-robin process scheduler.

##### `Scheduler::add_process(&mut self, process: Process)`

Adds a process to the scheduling queue.

##### `Scheduler::schedule(&mut self) -> Option<(*mut Context, *const Context)>`

Selects the next process to run. Returns a pair of pointers: `(prev_ctx, next_ctx)`.
The caller must pass these to `arch::switch_to`.

---

### Capability System

Capability-based access control.

#### Module: `kernel::capability`

---

#### `Capability`

```rust
pub struct Capability {
    pub id: u64,
    pub resource: ResourceType,
    pub permissions: Permissions,
    pub owner: u64,
}
```

---

#### `ResourceType` Enum

```rust
pub enum ResourceType {
    Memory { base: usize, size: usize },
    Device { device_id: u32 },
    Interrupt { irq: u32 },
    Port { port: u16 },
    File { inode: u64 },
    Process { pid: u32 },
    Intent { intent_id: u64 },
}
```

---

#### `Permissions`

```rust
pub struct Permissions(u32);

impl Permissions {
    pub const READ: Permissions = Permissions(0x01);
    pub const WRITE: Permissions = Permissions(0x02);
    pub const EXECUTE: Permissions = Permissions(0x04);
    pub const GRANT: Permissions = Permissions(0x08);
    pub const REVOKE: Permissions = Permissions(0x10);
    pub const DERIVE: Permissions = Permissions(0x20);
}
```

---

#### `CapabilityTable`

##### `CapabilityTable::new() -> Self`

Create empty capability table.

##### `CapabilityTable::create(&mut self, resource: ResourceType, perms: Permissions, owner: u64) -> Option<Capability>`

Create new capability.

```rust
let cap = table.create(
    ResourceType::Memory { base: 0x1000, size: 0x1000 },
    Permissions::READ | Permissions::WRITE,
    0
)?;
```

##### `CapabilityTable::derive(&mut self, parent: &Capability, new_perms: Permissions) -> Option<Capability>`

Derive child capability with reduced permissions.

```rust
let child = table.derive(&cap, Permissions::READ)?;
```

##### `CapabilityTable::revoke(&mut self, cap: &Capability) -> bool`

Revoke capability and all derivatives.

```rust
table.revoke(&cap);
```

##### `CapabilityTable::validate(&self, cap: &Capability, required: Permissions) -> bool`

Check if capability has required permissions.

```rust
if table.validate(&cap, Permissions::WRITE) {
    // Allowed to write
}
```

##### `CapabilityTable::lookup(&self, id: u64) -> Option<&Capability>`

Find capability by ID.

---

#### Global Functions

##### `capability::init()`

Initialize global capability system.

##### `capability::init_security(seed: u64)`

Initialize capability security features (Pointer Guard).

##### `capability::create_root_capability(resource: ResourceType) -> Capability`

Create root capability for resource.

##### `capability::require(cap: &Capability, perms: Permissions) -> bool`

Validate capability has permissions.

---

## Steno Engine

### Module: `steno`

#### Constants

##### `MULTI_STROKE_TIMEOUT_US`

Timeout for multi-stroke sequences (500,000 μs = 500ms).

##### `MAX_BUFFER_STROKES`

Maximum strokes that can be buffered (8).

#### Global Functions

##### `steno::init()`

Initialize the stenographic engine with default dictionary.

##### `steno::process_stroke(stroke: Stroke) -> Option<Intent>`

Process a stroke and return intent if matched.

##### `steno::process_steno(steno: &str) -> Option<Intent>`

Process from RTFCRE notation.

```rust
if let Some(intent) = steno::process_steno("HELP") {
    intent::execute(&intent);
}
```

##### `steno::process_raw(bits: u32) -> Option<Intent>`

Process from raw stroke bits (hardware input).

##### `steno::stats() -> EngineStats`

Get engine statistics.

##### `steno::history_len() -> usize`

Get number of strokes in history.

##### `steno::redo() -> Option<Intent>`

Redo the last undone action.

##### `steno::flush_buffer()`

Force-flush the stroke buffer (for external timeout triggers).

##### `steno::stroke_buffer() -> &StrokeSequence`

Get the current stroke buffer (for debugging/display).

---

### Multi-Stroke Dictionary

#### Module: `steno::dictionary`

#### `StrokeSequence`

A sequence of up to 8 strokes for multi-stroke entries.

##### `StrokeSequence::from_steno(steno: &str) -> Self`

Parse a slash-separated steno notation.

```rust
let seq = StrokeSequence::from_steno("RAOE/PWOOT");
assert_eq!(seq.len(), 2);
```

##### `StrokeSequence::matches(&self, other: &StrokeSequence) -> bool`

Exact match comparison.

##### `StrokeSequence::starts_with(&self, prefix: &StrokeSequence) -> bool`

Check if this sequence starts with the given prefix.

---

#### `MultiStrokeDictionary`

Dictionary for multi-stroke entries (128 max).

##### `MultiStrokeDictionary::lookup(&self, sequence: &StrokeSequence) -> Option<&MultiStrokeEntry>`

Exact lookup of a multi-stroke sequence.

##### `MultiStrokeDictionary::check_prefix(&self, sequence: &StrokeSequence) -> (bool, bool)`

Check if sequence matches or could match any entry.

Returns `(has_exact_match, has_prefix_match)` tuple.

```rust
// After typing "RAOE"
let (exact, prefix) = dict.check_prefix(&buffer);
assert_eq!(exact, false);   // "RAOE" alone isn't complete
assert_eq!(prefix, true);   // But "RAOE/PWOOT" exists
```

##### `MultiStrokeDictionary::sequence_to_intent(&self, sequence: &StrokeSequence) -> Option<Intent>`

Convert a matched sequence to an Intent.

---

#### `MultiStrokeEntry`

```rust
pub struct MultiStrokeEntry {
    pub sequence: StrokeSequence,
    pub concept_id: ConceptID,
    pub name: &'static str,
}
```

##### `MultiStrokeEntry::from_steno(steno: &str, concept_id: ConceptID, name: &str) -> Self`

Create a multi-stroke entry from slash-separated notation.

```rust
MultiStrokeEntry::from_steno("RAOE/PWOOT", ConceptID(0x0000_0003), "REBOOT")
```

---

### Stroke History

#### Module: `steno::history`

#### `StrokeHistory`

64-entry ring buffer for stroke history with undo/redo.

##### `StrokeHistory::new() -> Self`

Create empty history.

##### `StrokeHistory::push(&mut self, stroke: Stroke, intent: Option<&Intent>, timestamp: u64)`

Push a new stroke to history.

##### `StrokeHistory::last(&self) -> Option<&HistoryEntry>`

Get the most recent entry.

##### `StrokeHistory::at(&self, offset: usize) -> Option<&HistoryEntry>`

Get entry at offset from most recent (0 = most recent).

##### `StrokeHistory::undo(&mut self) -> Option<&HistoryEntry>`

Mark the most recent non-undone entry as undone.

##### `StrokeHistory::redo(&mut self) -> Option<&HistoryEntry>`

Redo the most recently undone entry.

##### `StrokeHistory::len(&self) -> usize`

Get number of entries.

##### `StrokeHistory::recent(&self, max: usize) -> RecentStrokesIter`

Iterate over recent strokes (most recent first).

---

#### `HistoryEntry`

```rust
pub struct HistoryEntry {
    pub stroke: Stroke,
    pub intent_id: Option<u64>,
    pub timestamp: u64,
    pub undone: bool,
}
```

---

## Intent Engine

Natural language intent processing.

#### Module: `intent`

---

#### `IntentEngine`

##### `IntentEngine::new() -> Self`

Create intent engine.

##### `IntentEngine::process(&mut self, input: &str) -> IntentResult`

Process natural language intent.

```rust
let result = engine.process("show memory status");
```

##### `IntentEngine::register_handler(&mut self, pattern: &str, handler: fn(&str) -> IntentResult)`

Register custom intent handler.

---

#### `IntentResult`

```rust
pub enum IntentResult {
    Success(String),
    Error(String),
    NeedMoreInfo(String),
    Capability(Capability),
}
```

---

#### `Intent` Enum

```rust
pub enum Intent {
    // System
    Status,
    Shutdown,
    Reboot,
    Help,
    
    // Memory
    MemoryStats,
    Allocate { size: usize },
    Free { address: usize },
    
    // GPIO
    SetPin { pin: u32, high: bool },
    ReadPin { pin: u32 },
    
    // Display
    Clear,
    Print { text: String },
    SetColor { fg: u32, bg: u32 },
    
    // Unknown
    Unknown(String),
}
```

---

#### Parsing

##### `parse_intent(input: &str) -> Intent`

Parse natural language to intent.

```rust
let intent = parse_intent("turn on LED 42");
// Returns Intent::SetPin { pin: 42, high: true }
```

---

#### REPL

##### `run()`

Start interactive REPL.

```rust
intent::run(); // Starts "intent>" prompt
```

---

### Intent Handlers

#### Module: `intent::handlers`

#### `HandlerRegistry`

128-handler registry with priority dispatch.

##### `HandlerRegistry::new() -> Self`

Create empty registry.

##### `HandlerRegistry::register(&mut self, concept_id: ConceptID, handler: HandlerFn, name: &'static str) -> bool`

Register a handler for a specific concept.

```rust
registry.register(concepts::STATUS, my_handler, "custom_status");
```

##### `HandlerRegistry::register_with_options(&mut self, concept_id: ConceptID, handler: HandlerFn, name: &'static str, priority: u8, required_cap: Option<CapabilityType>) -> bool`

Register with full options.

##### `HandlerRegistry::register_wildcard(&mut self, handler: HandlerFn, name: &'static str, priority: u8) -> bool`

Register a wildcard handler (receives all intents).

##### `HandlerRegistry::unregister(&mut self, name: &'static str) -> bool`

Unregister a handler by name.

##### `HandlerRegistry::dispatch(&mut self, intent: &Intent, has_cap: impl Fn(CapabilityType) -> bool) -> bool`

Dispatch an intent to registered handlers.

---

#### `HandlerResult`

```rust
pub enum HandlerResult {
    Handled,       // Intent was handled
    NotHandled,    // Pass to next handler
    Error(u32),    // Handler failed
}
```

---

#### Global Functions

##### `intent::register_handler(concept_id: ConceptID, handler: HandlerFn, name: &'static str) -> bool`

Register a user-defined handler.

##### `intent::unregister_handler(name: &'static str) -> bool`

Unregister a handler.

---

### Intent Queue

#### Module: `intent::queue`

#### `IntentQueue`

32-entry priority queue for deferred execution.

##### `IntentQueue::new() -> Self`

Create empty queue.

##### `IntentQueue::push(&mut self, intent: Intent, timestamp: u64) -> bool`

Push with default priority.

##### `IntentQueue::push_with_priority(&mut self, intent: Intent, priority: Priority, timestamp: u64, deadline: u64) -> bool`

Push with specific priority and deadline.

##### `IntentQueue::pop(&mut self) -> Option<QueuedIntent>`

Pop highest priority intent.

##### `IntentQueue::peek(&self) -> Option<&QueuedIntent>`

Peek at highest priority without removing.

##### `IntentQueue::remove_expired(&mut self, now: u64) -> usize`

Remove expired intents.

---

#### `Priority`

```rust
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}
```

---

#### `QueuedIntent`

```rust
pub struct QueuedIntent {
    pub intent: Intent,
    pub priority: Priority,
    pub sequence: u64,
    pub queued_at: u64,
    pub deadline: u64,
}
```

---

#### Global Functions

##### `intent::queue(intent: Intent, timestamp: u64) -> bool`

Queue an intent for deferred execution.

##### `intent::queue_with_priority(intent: Intent, priority: Priority, timestamp: u64) -> bool`

Queue with specific priority.

##### `intent::process_queue() -> bool`

Process next queued intent.

##### `intent::queue_len() -> usize`

Get number of queued intents.

---

## Error Handling

### `KernelError` Enum

```rust
pub enum KernelError {
    OutOfMemory,
    InvalidCapability,
    PermissionDenied,
    DeviceError(u32),
    InvalidAddress,
    NotFound,
}
```

### Panic Handler

The kernel provides a panic handler that prints diagnostics and halts:

```rust
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Prints location and message via UART
    // Then halts CPU
}
```

---

## Type Aliases

```rust
pub type Result<T> = core::result::Result<T, KernelError>;
pub type Address = usize;
pub type PhysAddr = usize;
pub type VirtAddr = usize;
```

---

## Constants

### Memory

```rust
pub const PAGE_SIZE: usize = 4096;
pub const KERNEL_BASE: usize = 0x80000;
pub const HEAP_START: usize = 0x200000;
pub const HEAP_SIZE: usize = 0x1_0000_0000; // 4GB
```

### Peripherals

```rust
pub const PERIPHERAL_BASE: usize = 0x1_0000_0000;
pub const GPIO_BASE: usize = PERIPHERAL_BASE + 0x200000;
pub const UART0_BASE: usize = PERIPHERAL_BASE + 0x201000;
pub const GIC_BASE: usize = PERIPHERAL_BASE + 0x40000;
```

---

## Console

Framebuffer-based text console.

#### Module: `drivers::console`

---

#### `Console`

##### `Console::new(width: u32, height: u32) -> Self`

Create a new console with given dimensions.

##### `Console::init()`

Initialize the global console (call after framebuffer init).

##### `Console::write_str(&mut self, s: &str)`

Write a string to the console at the current cursor position.

##### `Console::set_colors(&mut self, fg: Color, bg: Color)`

Set foreground and background colors.

---

#### Global Functions

##### `console::init()`

Initialize the global console.

##### `console::print(args: fmt::Arguments)`

Print formatted text to console.

##### `console::println(args: fmt::Arguments)`

Print formatted text with newline.

##### `console::clear()`

Clear the console screen.

---

#### Macros

##### `cprint!(fmt, args...)`

Print to the framebuffer console.

```rust
cprint!("Value: {}", 42);
```

##### `cprintln!(fmt, args...)`

Print to the framebuffer console with newline.

```rust
cprintln!("Intent: {}", intent.name);
```

---

## Steno English Bridge

Process English text as steno strokes (reverse dictionary lookup).

#### Module: `steno`

---

#### Functions

##### `steno::process_english(text: &str) -> Option<Intent>`

Process an English word/command by looking up its corresponding steno stroke
and processing that stroke through the engine.

```rust
// User types "help"
if let Some(intent) = steno::process_english("help") {
    // Found stroke PH-FPL for "HELP"
    // intent.concept_id == concepts::HELP
    intent::execute(&intent);
}
```

**How it works**:
1. Look up English word in dictionary (reverse lookup by name)
2. If found, get the corresponding stroke
3. Process that stroke through the steno engine
4. Return the resulting intent

This allows non-steno users to interact with the kernel while maintaining
the semantic-first architecture internally.

---

#### Dictionary Functions

##### `StenoDictionary::lookup_by_name(&self, name: &str) -> Option<Stroke>`

Reverse lookup: find the stroke that produces a given English name.

```rust
if let Some(stroke) = dictionary.lookup_by_name("HELP") {
    // stroke represents PH-FPL
}
```

Case-insensitive matching.

---

*API Reference v0.4.0 - Intent Kernel (Phase 5 Complete)*

---

## `kernel::syscall`
System Call Interface for User Mode processes.

### Enums
- `SyscallNumber`: Defines syscall numbers (Yield=1, Print=2, Sleep=3).

### Functions
- `dispatcher(num: u64, arg0: u64, arg1: u64, arg2: u64) -> u64`: Handles system calls.

### System Call ABI
- **Instruction**: `svc #0`
- **Register x8**: Syscall Number
- **Register x0-x7**: Arguments
- **Return Value**: x0

| Syscall | Number | Arg0 | Arg1 | Arg2 | Description |
|---------|--------|------|------|------|-------------|
| Yield   | 1      | -    | -    | -    | Yield CPU to next task |
| Print   | 2      | Ptr  | Len  | -    | Print string to UART |
| Sleep   | 3      | MS   | -    | -    | Sleep for N milliseconds |
