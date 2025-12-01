# Intent Kernel Architecture

> A deep dive into the technical design of the Intent Kernel

## Overview

Intent Kernel is a capability-based microkernel designed from scratch for the Raspberry Pi 5. It replaces traditional OS abstractions (files, processes, users) with a unified capability model where everything is an unforgeable token.

## Design Principles

### 1. **Zero Trust, Full Capability**
Every operation requires a valid capability. There are no ambient permissions, no superuser, no privilege escalation. If you have the capability, you can do it. If you don't, you can't.

### 2. **Intent Over Instruction**
Users express what they want, not how to do it. The Intent Engine translates natural language into capability-protected operations.

### 3. **No Legacy**
No POSIX, no files, no processes in the traditional sense. We start fresh with abstractions that make sense for modern hardware.

### 4. **Bare Metal Purity**
Zero external dependencies. Every line of code is either ours or the Rust compiler's output. No libc, no external crates, nothing.

## Layer Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           INTENT LAYER                                      │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐              │
│  │ Semantic Engine │  │ Neural Memory   │  │  Async REPL     │              │
│  │  - ConceptID    │  │  - Embeddings   │  │  - Non-blocking │              │
│  │  - Context      │  │  - Cosine Sim   │  │  - Event loop   │              │
│  │  - Hashing      │  │  - Neural Alloc │  │                 │              │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘              │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                          CAPABILITY LAYER                                   │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐              │
│  │ Capability      │  │ Permission      │  │ Revocation      │              │
│  │ Registry        │  │ Checking        │  │ Tree            │              │
│  │  - Mint         │  │  - READ         │  │  - Track parent │              │
│  │  - Derive       │  │  - WRITE        │  │  - Cascade      │              │
│  │  - Lookup       │  │  - EXECUTE      │  │  - Invalidate   │              │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘              │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           KERNEL LAYER                                      │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐              │
│  │ Memory Manager  │  │ Exception       │  │ Async Executor  │              │
│  │  - Buddy alloc  │  │ Handler         │  │  - Tasks/Future │              │
│  │  - Slab cache   │  │  - Sync         │  │  - Wakers       │              │
│  │  - DMA region   │  │  - IRQ          │  │  - WFI Sleep    │              │
│  │  - GPU shared   │  │  - SError       │  │                 │              │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘              │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           DRIVER LAYER                                      │
│  ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐ ┌───────┐ ┌─────────────┐          │
│  │ UART  │ │ GPIO  │ │ Timer │ │ GIC   │ │Mailbox│ │ Framebuffer │          │
│  │PL011  │ │58 pins│ │Generic│ │ -400  │ │VideoCr│ │ Display     │          │
│  └───────┘ └───────┘ └───────┘ └───────┘ └───────┘ └─────────────┘          │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              BOOT LAYER                                     │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐              │
│  │ Exception       │  │ EL Transitions  │  │ Multi-core      │              │
│  │ Vectors         │  │  EL3 → EL2      │  │ Boot            │              │
│  │  - el1_sync     │  │  EL2 → EL1      │  │  - Core 0 main  │              │
│  │  - el1_irq      │  │  - Configure    │  │  - Cores 1-3    │              │
│  │  - el1_fiq      │  │    registers    │  │    secondary    │              │
│  │  - el1_serror   │  │                 │  │                 │              │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘              │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           HARDWARE (BCM2712)                                │
│                                                                             │
│   ARM Cortex-A76 × 4    VideoCore VII    8GB LPDDR4X    GIC-400             │
│   @ 2.4GHz              GPU              RAM             IRQ Controller     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Boot Sequence

```
Power On
    │
    ▼
┌───────────────────────┐
│ GPU loads bootcode.bin│  ← VideoCore firmware (provided by RPi Foundation)
│ from SD card          │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│ GPU loads kernel8.img │  ← Our kernel image
│ to 0x80000            │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│ _start (boot.s)       │  ← Entry point in EL2 or EL3
│  - Check core ID      │
│  - Core 0 continues   │
│  - Cores 1-3 wait     │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│ EL3 → EL2 → EL1       │  ← Exception level transitions
│  - Configure SCRs     │
│  - Set up VBAR        │
│  - Enable features    │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│ Set up stacks         │  ← Each core gets its own stack
│ Clear BSS             │
│ Set exception vectors │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│ kernel_main() [Rust]  │  ← Hand off to Rust
│  - UART init          │
│  - Timer init         │
│  - Memory init        │
│  - Capability init    │
│  - Intent engine      │
└───────────────────────┘
```

## Capability System

### Capability Structure
```rust
struct Capability {
    id: u64,           // Unique identifier
    generation: u64,   // For revocation detection
    cap_type: u8,      // Memory, Device, Display, etc.
    permissions: u32,  // READ, WRITE, EXECUTE, DELEGATE, REVOKE
    resource: u64,     // Address or device ID
    size: u64,         // Resource size/range
}
```

### Capability Types
| Type | ID | Description |
|------|-----|-------------|
| Null | 0 | Invalid capability |
| Memory | 1 | Access to memory region |
| Device | 2 | Access to I/O device |
| Interrupt | 3 | Handle interrupts |
| Timer | 4 | Use system timer |
| Display | 5 | Access framebuffer |
| Compute | 6 | GPU compute access |
| Network | 7 | Network interface |
| Storage | 8 | Storage device |
| Input | 9 | Input devices |
| Intent | 10 | Intent interpreter |
| CapControl | 11 | Mint/revoke capabilities |
| System | 12 | System control |

### Permission Flags
```
Bit 0: READ      - Can read from resource
Bit 1: WRITE     - Can write to resource
Bit 2: EXECUTE   - Can execute resource
Bit 3: DELETE    - Can delete resource
Bit 4: SHARE     - Can share with others
Bit 5: DELEGATE  - Can create derived capabilities
Bit 6: REVOKE    - Can revoke derived capabilities
```

### Capability Derivation
```
Root Capability (ALL permissions)
        │
        ├─► Derived Cap 1 (READ + WRITE)
        │         │
        │         └─► Derived Cap 1a (READ only)
        │
        └─► Derived Cap 2 (READ + EXECUTE)
                  │
                  └─► [Cannot derive further - no DELEGATE]
```

When a capability is revoked, all derived capabilities are automatically invalidated.

## Memory Architecture

### Physical Memory Layout (8GB)
```
0x0000_0000_0000 ─────────────────── Reserved (VideoCore)
                 │   512KB
0x0000_0008_0000 ─────────────────── Kernel Load Address
                 │   ~2MB           Kernel code, data, BSS
0x0000_0020_0000 ─────────────────── Heap Start (Randomized Base)
                 │   ~4GB           Buddy allocator pool
                 │                  (Actual start = Base + Random Offset)
0x0001_0000_0000 ─────────────────── DMA Region
                 │   256MB          DMA-safe allocations
0x0001_1000_0000 ─────────────────── GPU Shared
                 │   256MB          VideoCore communication
0x0001_2000_0000 ─────────────────── Intent Engine Memory
                 │   256MB          Intent workspace
0x0002_0000_0000 ─────────────────── End of mapped region
```

### Virtual Memory Layout (Planned v0.2.0)
The kernel will transition to a "Higher Half" design using ARM64 VMSA:

**User Space (TTBR0)**: `0x0000_0000_0000_0000` - `0x0000_FFFF_FFFF_FFFF`
- Application Code/Data
- User Stack
- Shared Libraries (Capabilities)

**Kernel Space (TTBR1)**: `0xFFFF_0000_0000_0000` - `0xFFFF_FFFF_FFFF_FFFF`
- `0xFFFF_0000_0000_0000`: Physical Memory Map (Identity or Offset)
- `0xFFFF_8000_0000_0000`: Kernel Code/Data
- `0xFFFF_FFFF_F000_0000`: MMIO Peripherals (mapped as Device-nGnRnE)

### Allocator Design

**Buddy Allocator** (for large allocations):
- Minimum block: 16 bytes
- Maximum block: 16MB (order 20)
- Coalescing on free
- O(log n) allocation

**Slab Allocator** (for small, fixed-size objects):
- Slab sizes: 16, 32, 64, 128, 256, 512, 1024, 2048 bytes
- Page-based slabs
- Free list per size class
- O(1) allocation

### Neural Allocator (Semantic Memory)
A content-addressable memory system that sits alongside the traditional allocators.

**Semantic Block**:
- `embedding`: 64-byte vector representing the content's meaning.
- `access_count`: For "forgetting" (LRU-like eviction).
- `data`: The actual stored information.

**Allocation Strategy**:
1.  **Store**: `alloc(size, embedding)` -> Finds a free block and tags it with the vector.
2.  **Retrieve**: `retrieve(query_vector)` -> Scans blocks for highest Cosine Similarity (>70%).

## Intent Engine

### Parsing Pipeline (Vector-Native)
```
"show the temperature"
         │
         ▼
┌─────────────────┐
│ Tokenize        │  → ["show", "the", "temperature"]
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Vector Lookup   │  → Look up 64-dim embedding in Static Knowledge Base
│                 │     (Simulating an Embedding Model)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Neural Memory   │  → Calculate Cosine Similarity against Concept Vectors
│                 │     Score = (A . B) / (||A|| * ||B||)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Build Intent    │  → Intent { concept: Display, confidence: >70% }
└─────────────────┘
```

### Execution Flow
```
Intent
   │
   ▼
┌─────────────────────────┐
│ Check Intent Capability │  ← Must have Intent cap
└───────────┬─────────────┘
            │ has cap
            ▼
┌─────────────────────────┐
│ Map to Resource Cap     │  ← Display intent → Display cap
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ Validate Resource Cap   │  ← Not revoked? Right permissions?
└───────────┬─────────────┘
            │ valid
            ▼
┌─────────────────────────┐
│ Execute Operation       │  ← Call into driver layer
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│ Return Result           │  ← Success, Value, Error, etc.
└─────────────────────────┘
```

## Driver Model

Each driver follows a consistent pattern:

```rust
// Private state (Thread-Safe)
static DRIVER: Mutex<Driver> = Mutex::new(Driver::new(BASE_ADDRESS));

// Public interface
pub fn init() { ... }
pub fn operation(...) -> Result { ... }

// Register access helpers
impl Driver {
    fn read(&self, offset: usize) -> u32 {
        unsafe { arch::read32(self.base + offset) }
    }
    
    fn write(&self, offset: usize, value: u32) {
        unsafe { arch::write32(self.base + offset, value) }
    }
}
```

## Exception Handling

### Exception Vector Table
```
┌──────────────────┬──────────────────┬──────────────────┬──────────────────┐
│     Sync         │      IRQ         │      FIQ         │    SError        │
├──────────────────┼──────────────────┼──────────────────┼──────────────────┤
│ Current EL SP0   │ Current EL SP0   │ Current EL SP0   │ Current EL SP0   │
├──────────────────┼──────────────────┼──────────────────┼──────────────────┤
│ Current EL SPx   │ Current EL SPx   │ Current EL SPx   │ Current EL SPx   │
├──────────────────┼──────────────────┼──────────────────┼──────────────────┤
│ Lower EL AArch64 │ Lower EL AArch64 │ Lower EL AArch64 │ Lower EL AArch64 │
├──────────────────┼──────────────────┼──────────────────┼──────────────────┤
│ Lower EL AArch32 │ Lower EL AArch32 │ Lower EL AArch32 │ Lower EL AArch32 │
└──────────────────┴──────────────────┴──────────────────┴──────────────────┘
```

### Exception Flow
1. **Vector Table (`boot.s`)**: Catches exception, saves context (`SAVE_ALL`), calls Rust handler.
2. **Rust Handler (`kernel::exception`)**:
   - Decodes `ESR_EL1` to determine exception class (Data Abort, SVC, etc.).
   - For Data Aborts, decodes `ISS` to identify Translation vs Permission faults.
   - Dumps register state (`ExceptionFrame`) if fatal.
3. **Recovery or Halt**: Currently halts on fatal exceptions; future will kill faulting process.

### Exception Frame
```rust
#[repr(C)]
pub struct ExceptionFrame {
    pub x: [u64; 30],       // General purpose registers x0-x29
    pub x30: u64,           // Link register
    pub elr: u64,           // Exception link register (return address)
    pub spsr: u64,          // Saved program status register
    pub esr: u64,           // Exception syndrome register
    pub far: u64,           // Fault address register
}
```

## Process Model
The kernel implements a **Preemptive Multitasking** system supporting both Kernel Threads (EL1) and User Processes (EL0).

#### Process Control Block (PCB)
Each process is represented by a `Process` struct containing:
- **ID**: Unique `ProcessId`.
- **State**: `Ready`, `Running`, `Blocked`, `Terminated`.
- **Context**: Saved registers (`x19`-`x29`, `sp`, `lr`) and `TTBR0` (Page Table Base).
- **Stacks**:
  - `kernel_stack`: Used when running in EL1 (syscalls/interrupts).
  - `user_stack`: Used when running in EL0 (User Mode).

#### Scheduling & Preemption
- **Scheduler**: A Round-Robin scheduler manages a queue of `Ready` processes.
- **Preemption**: The ARM Generic Timer fires periodic interrupts (e.g., every 10ms). The IRQ handler calls `scheduler::tick()`, which may trigger a context switch.
- **Context Switch**: The `switch_to` assembly function saves the current context and restores the next. It handles `TTBR0` switching for address space isolation.

#### User Mode & System Calls
- **EL0 Transition**: The kernel switches to User Mode using `eret` after setting up `SPSR_EL1` (Mode 0) and `SP_EL0`.
- **System Calls**: User processes communicate with the kernel via the `svc` (Supervisor Call) instruction.
- **Handler**: The `handle_svc` exception handler dispatches syscalls based on the number in `x8`. Arguments are passed in `x0`-`x7`.
  - `Yield` (1): Voluntarily give up CPU.
  - `Print` (2): Output text to UART.
  - `Sleep` (3): Sleep for N milliseconds.
 
 ## Adaptive Perception Layer
 
 The **Adaptive Perception Layer** acts as the "eyes and ears" of the Intent Kernel, abstracting the complexity of AI hardware accelerators.
 
 ### Design Philosophy
 1. **Hardware Agnostic**: The kernel should not care if it's running on a CPU or a 26 TOPS accelerator.
 2. **Graceful Degradation**: If the accelerator is missing, the system falls back to CPU execution without crashing.
 3. **Unified API**: All vision tasks use the `ObjectDetector` trait.
 
 ### Architecture
 ```
 ┌─────────────────────────┐
 │    Intent Engine        │
 │ "What do you see?"      │
 └───────────┬─────────────┘
             │
             ▼
 ┌─────────────────────────┐
 │   Perception Manager    │  ← Decides which backend to use
 └───────────┬─────────────┘
             │
      ┌──────┴──────┐
      ▼             ▼
 ┌──────────┐  ┌──────────┐
 │ Hailo-8  │  │   CPU    │
 │ Driver   │  │ Fallback │
 └──────────┘  └──────────┘
      │             │
      ▼             ▼
   Hardware      Software
 Accelerator      Model
 ```
 
 ### Hailo-8 Integration
 - **Driver**: A stub driver (`drivers/hailo.rs`) currently simulates the device.
 - **Protocol**: Future implementation will use PCIe to send `.hef` (Hailo Executable Format) models and receive inference buffers.
 - **Performance**: 26 TOPS (INT8) allows for real-time object detection (YOLOv8) at 30+ FPS, compared to <1 FPS on CPU.
 
 ## PCIe Subsystem (Hardware Awakening)
 
 The **PCIe Subsystem** enables the kernel to discover and interact with high-speed peripherals like the Hailo-8 AI Accelerator and the RP1 Southbridge.
 
 ### Architecture
 1.  **Root Complex**: The BCM2712 PCIe controller is initialized to manage the bus.
 2.  **ECAM (Enhanced Configuration Access Mechanism)**: A memory-mapped region allows the kernel to read/write configuration space registers for every device (Bus/Device/Function).
 3.  **Enumeration**:
     - The kernel scans Bus 0..255.
     - Reads Vendor/Device IDs.
     - Identifies known devices (e.g., Hailo-8: `1e60:2864`).
 4.  **Driver Binding**: Once a device is found, the corresponding driver (e.g., `drivers/hailo.rs`) is probed and initialized.
 
 ## Persistent Storage (RamDisk)
 
 The **Persistent Storage** subsystem provides a read-write filesystem that persists in RAM during a session.
 
 ### Architecture
 1.  **Bootloader Loading**: The Pi 5 bootloader loads a `fs.tar` file into memory at `0x2000_0000` (configured via `ramfsfile`).
 2.  **Base Layer (Read-Only)**: The kernel parses this TAR image to provide initial files (models, configs).
 3.  **Overlay Layer (Read-Write)**: A `RamDiskFS` wrapper maintains an in-memory list of created, modified, or deleted files.
     - **Writes**: Go to the overlay.
     - **Reads**: Check overlay first, then fall back to base TAR.
     - **Deletes**: Marked with tombstones in the overlay.
 
 ### Usage
 - **Intents**: `create`, `edit`, `delete`, `ls`, `cat`.
 - **Persistence**: Changes persist until reboot. To save permanently, the `fs.tar` on the SD card must be updated (future work).
 
 ## Future Directions

1. **Scheduler**: Priority-based scheduling across all 4 cores
2. **IPC**: Capability-based inter-process communication
3. **Persistence**: Save capability state across reboots
4. **Networking**: TCP/IP stack with capability protection
5. **AI Integration**: LLM-powered intent understanding
6. **GPU Compute**: VideoCore VII shader execution

---

*This document describes Intent Kernel v0.2 as of December 2025.*
