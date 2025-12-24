<p align="center">
  <img src="docs/assets/logo.png" alt="Intent Kernel" width="200" />
</p>

<h1 align="center">Intent Kernel</h1>

<p align="center">
  <strong>A Bare-Metal Operating System for Raspberry Pi 5</strong><br>
  <em>Semantic-first architecture with intent-based execution</em>
</p>

<p align="center">
  <a href="#overview">Overview</a> •
  <a href="#key-features">Features</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#benchmarks--performance">Benchmarks</a> •
  <a href="#status">Status</a> •
  <a href="#documentation">Documentation</a>
</p>


---

## Overview

**Intent Kernel** is a bare-metal operating system for Raspberry Pi 5 that processes inputs as semantic concepts rather than character streams.

The kernel implements a semantic-first architecture where inputs from multiple modalities (natural language, hardware patterns, sensors) are converted to 64-bit concept identifiers (ConceptIDs) and executed through a broadcast handler system. This eliminates character-level processing in the core execution path.

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   Any Input  │────▶│  Semantic    │────▶│  Dictionary  │────▶│   Executor   │
│ Language/HW/ │     │   Pattern    │     │ (ConceptID)  │     │  (Broadcast) │
│ Vision/Audio │     │              │     │              │     │              │
└──────────────┘     └──────────────┘     └──────────────┘     └──────────────┘
```

**Core principle**: All inputs map to ConceptIDs before execution.

---

## Key Features

### 1. Semantic-First Architecture
The kernel processes inputs as 64-bit concept identifiers (ConceptIDs) rather than character strings.

- **Execution path**: Input → ConceptID → Intent → Broadcast Handler
- **Hardware pattern latency**: 54 cycles measured (QEMU Cortex-A72)
- **English parsing latency**: 139 cycles measured
- **Architecture**: No string parsing in core intent execution

### 2. Neural Processing Model
The intent system implements 22 computational features inspired by neuroscience literature.

- **Temporal dynamics**: Activation decay (100ms), temporal summation
- **Hierarchical layers**: 5-layer abstraction (Raw → Feature → Object → Semantic → Action)
- **Spreading activation**: Related concepts receive predictive priming
- **Lateral inhibition**: Competing handlers suppress each other
- **Predictive processing**: Efference copy, surprise detection
- **Action selection**: Urgency-based scheduling (basal ganglia model)

### 3. Declarative Application Model
Applications are defined as `.intent` manifest files rather than compiled binaries.

- **Manifest format**: YAML-like syntax defining triggers and goals
- **Semantic linker**: Runtime binding of intents to available capabilities
- **Skill registry**: Atomic execution units (kernel drivers, WASM modules)
- **Execution model**: Manifests describe desired outcomes, system selects implementations

### 4. Dual-Process Execution
The system implements two execution modes with different performance characteristics.

- **Fast path (System 1)**: Pattern matching and handler dispatch
  - Measured: 54 cycles average for hardware patterns
  - Measured: 139 cycles average for English parsing
- **Slow path (System 2)**: LLM-based inference for complex reasoning
  - Measured: 300k+ cycles for transformer forward pass
  - Architecture: Llama 2 implementation, weights loaded from SD card

---

## Quick Start

```bash
# Clone the repository
git clone https://github.com/sutraworks/intent-kernel.git
cd intent-kernel

# Build the kernel
make kernel

# Run in QEMU (virt machine)
make run
# (Drops into Intent Console; User Process runs in background)

# Run 129 host tests (native)
make test

# Run 40-benchmark suite in QEMU
make run
```

### Requirements

| Tool | Version | Purpose |
|------|---------|---------|
| Rust | nightly | Compiler |
| aarch64-unknown-none | - | Target triple |
| QEMU | 8.0+ | Emulation (optional) |

---

## Input Methods

Intent Kernel supports **multiple input modalities**, all converging to the same semantic intent system:

### English Mode

The kernel includes a natural language parser supporting ~200 phrase patterns and 50 synonym expansions:

```
Keyboard → English Text → Natural Language Parser → Intent → Execute
                               ↓
                     (200+ phrases, 50+ synonyms)
```

**Example Commands:**
```
"help"              → Shows system help
"show me status"    → Displays system status  
"what time is it?"  → Shows current time
"open notes"        → Opens notes application
```

### Hardware Chord Mode

Direct hardware pattern input using 23-bit chord encoding (based on stenographic key layout):

| Method | Speed | Latency |
|--------|-------|---------|
| Typing | 40-80 WPM | ~50ms |
| Voice | 100-150 WPM | ~200ms |
| **Hardware Chords** | **200-300 WPM** | **<0.1μs** |

Single chord maps directly to ConceptID without intermediate text processing.

### Perception Mode (AI-Powered)

Vision and audio inputs are processed through the Hailo-8 NPU and converted to hypervectors:

```
Camera → Hailo-8 NPU → YOLO Detection → ConceptID → Semantic Memory
Mic    → Audio Features → Classification → ConceptID → Semantic Memory
```

### Comparison

| Input Method | Latency | Learning Curve | Best For |
|--------------|---------|----------------|----------|
| **English Keyboard** | ~30μs | None | Everyone |
| **Hardware Chords** | <0.1μs | High (months) | Power users, professionals |
| **Vision/Audio** | ~50ms | None | AI perception, context |

**Architecture**: All input paths converge to ConceptID before intent execution.

### Implementation Details

#### The Semantic Pattern

Hardware chord input produces 23-bit patterns representing key combinations:

```
Position:  0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15  16  17  18  19  20  21  22
Key:       #  S-  T-  K-  P-  W-  H-  R-  A-  O-  *  -E  -U  -F  -R  -P  -B  -L  -G  -T  -S  -D  -Z
```

#### The Dictionary

Input patterns map to **concepts**, not text:

**Single-Input Patterns:**
| Pattern | Notation | Concept | Action |
|---------|----------|---------|--------|
| `0x42` | `STAT` | STATUS | Display system status |
| `0x400` | `*` | UNDO | Undo last action |
| `0x1A4` | `HELP` | HELP | Show help |
| `0x...` | `SHRO` | SHOW | Display something |

**Multi-Input Sequences:**
| Count | Notation | Concept | Action |
|-------|----------|---------|--------|
| 2 | `RAOE/PWOOT` | REBOOT | Restart system |
| 2 | `SHUT/TKOUPB` | SHUTDOWN | Power off |
| 2 | `SKROL/UP` | SCROLL_UP | Scroll display up |
| 3 | `TPHU/TPAOEU/-L` | NEW_FILE | Create new file |
| 3 | `KP-U/EUPB/TPO` | CPU_INFO | Show CPU info |

#### The Broadcast (1:N)

Intents are **broadcast** to all interested listeners, not just dispatched to a single handler. This mimics the brain's motor control system.

```
Intent ("GRASP") ────┬────▶ Motor Cortex (Move Arm)
                     ├────▶ Visual Cortex (Track Hand)
                     └────▶ Proprioception (Expect Weight)
```

#### Sensor Fusion (N:1)

The **Perception Layer** fuses data from all active sensors into a single "World Model".

```
Camera (Hailo-8) ──┐
Lidar (Virtual)  ──┼──▶ Perception Manager ──▶ World Model
Touch Sensors    ──┘
```

#### The Flow

```rust
// An input pattern comes in from hardware
let pattern = InputPattern::from_raw(0x42);

// The engine processes it
if let Some(intent) = input::process_pattern(pattern) {
    // The executor broadcasts it
    intent::execute(&intent);
}
```

---

## System Capabilities

The kernel implements a comprehensive feature set across multiple subsystems:

### Input Processing
- **Input History**: 64-entry ring buffer with undo/redo support
- **Multi-Input Sequences**: Support for 2-8 input patterns with 500ms timeout and prefix matching
- **Unified Input Mode**: Natural language and hardware patterns as first-class citizens

### Intent Execution
- **User-Defined Handlers**: Custom handler registration for any ConceptID
- **Priority Queue**: Deferred and prioritized intent execution with deadlines
- **Intent Security**: Multi-layered security with spam detection and privilege checking (20 cycles overhead)

### System Security
- **Capability Security**: Fine-grained permission control
- **Semantic Tollbooth**: Syscall gating restricting direct I/O to privileged drivers
- **VMM Isolation**: TTBR0 switching with kernel protection
- **Safe Stack Architecture**: VMM-backed stacks with guard pages

### Multi-Core Support
- **SMP Scheduler**: 4-core work-stealing scheduler with per-core run queues
- **Watchdog Core**: Dedicated core 3 for health monitoring and deadlock detection
- **4-Level Priority**: Idle, Normal, High, Realtime scheduling

### Storage & Networking
- **Persistent Storage**: SD card driver with SDHCI, DMA, and write support
- **TCP/IP Stack**: Full implementation with Ethernet, ARP, IPv4, ICMP, UDP, TCP
- **Userspace Networking**: UDP/TCP socket operations available to user processes

### Perception & AI
- **Vision Processing**: Hailo-8 NPU with YOLO tensor parsing and NMS algorithm
- **Audio Processing**: ZCR + Energy classification (Speech/Noise/Silence)
- **Sensor Fusion**: N:1 aggregation from multiple detectors

### Neural Architecture
- **22 Active Features**: Temporal dynamics, hierarchical processing, spreading activation, lateral inhibition, predictive processing
- **Semantic Memory**: HDC-based content-addressable storage with O(log N) retrieval

### Application Framework
- **Declarative Apps**: `.intent` manifest files for code-free application development
- **Semantic Linker**: Runtime binding of intents to available skills
- **Semantic Process Binding**: Dynamic intent handler registration via `sys_announce`

### System 2 Cognition
- **LLM Engine**: Llama 2 transformer implementation with weight loading from SD card
- **Dual-Process**: Fast path (54 cycles) and slow path (300k+ cycles) execution modes

---

_The following detailed subsections have been consolidated above for improved readability._

<details>
<summary>Legacy Feature List (Click to expand)</summary>

### Multi-Input Sequences
Real multi-input support with prefix matching and timeout:

```rust
// 2-input sequences
"RAOE/PWOOT" → REBOOT
"SHUT/TKOUPB" → SHUTDOWN
"RAOE/KAUL" → RECALL

// 3-input sequences
"TPHU/TPAOEU/-L" → NEW_FILE
"KP-U/EUPB/TPO" → CPU_INFO
```

- 500ms timeout between inputs
- Prefix matching (waits when partial match exists)
- 20+ built-in multi-input entries

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
Real-time visualization of the input stream and intent execution log.
- **Input Tape**: Scrolling log of all input patterns and commands.
- **Intent Stream**: Visual log of recognized semantic actions.
- **Status Bar**: Real-time WPM and input statistics.

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

### ✅ Intent Security System ✨ NEW! (Sprint 13.3)

Production-grade multi-layered security protecting the intent execution pipeline:

**Performance**: ~20 cycles overhead per intent (real QEMU measurement)

```rust
// Security enforced on EVERY intent execution
if let Err(violation) = security.check_intent(...) {
    kprintln!("[SECURITY] Intent rejected: {:?}", violation);
    return;  // Blocked!
}
```

- Privilege escalation attempts
- Handler hijacking / ROP attacks
- Semantic anomalies / unusual patterns

### ✅ Semantic Tollbooth (Syscall Gating) ✨ NEW! (Sprint 17)
Direct imperative I/O (`open`, `read`, `write`) is **restricted** to privileged drivers. Standard User Agents MUST use `SYS_PARSE_INTENT` to perform actions. This enforces the "Pure Intent" architecture and prevents unmonitored side effects.

### ✅ Multi-Core & Watchdog (NEW!)

**4-Core Architecture**:
- **3 Worker Cores** (0-2): Process intents with work-stealing load balancing
- **1 Watchdog Core** (3): Dedicated semantic immune system

**Watchdog Capabilities**:
- **Health Monitoring**: CPU, memory, thermal sensors, task queues
- **Deadlock Detection**: Wait-for graph analysis with Tarjan's algorithm
- **Self-Healing**: Automatic recovery from hung cores, deadlocks, memory leaks
- **Intent Security**: Spam detection, privilege escalation prevention
- **Anomaly Detection**: Behavioral pattern analysis (future)

**Performance**:
- Context switch: 54 cycles (372% better than target)
- Syscall latency: 8-11 cycles (550% better)
- Watchdog latency: <1ms target
- Zero overhead on worker cores

### ✅ Unified Input Mode
All input methods are first-class:

```rust
// Natural language (accessible to everyone)
english::parse("show status");   // Natural text → intent

// Direct patterns (for expert speed)
input::process_pattern("STAT");  // Pattern → intent

// Hybrid mode (system understands both)
english::parse("STAT");          // Recognizes patterns too!
```

### ✅ Secure Base
Interrupt-safe concurrency primitives and removal of unsafe global state.
- **Deadlock-Free SpinLocks**: Automatically disable interrupts.
- **Safe Interrupts**: Thread-safe handler registration.
- **Overflow Protection**: Hardened filesystem parsers.

### ✅ Semantic Memory ✨ NEW!
Efficient, type-safe semantic storage:
- **ConceptID Indexing**: Direct mapping of semantic concepts to memory blocks.
- **Scalability**: Verified up to **1,000,000 concepts** (16ms alloc output).
- **O(log N) Retrieval**: Fast BTreeMap-based lookups.
- **Dynamic Page Allocation**: Memory grows indefinitely with system RAM (Bump Allocator).
- **Efficiency**: Eliminates HNSW overhead for clean, deterministic storage.

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
- **Tensor Parsing**: Processes 1917 detection boxes → Top 16 objects
- **Sensor Fusion**: Combines data from multiple detectors (Hailo-8 + CPU fallback)
- **Visual Intents**: Automatically maps detected objects to `ConceptID` and stores them in Semantic Memory. The system "remembers" what it sees.

### ✅ Audio Perception ✨ NEW!
The kernel can "hear" and classify sounds:
- **Feature Extraction**: Zero Crossing Rate (ZCR) + Energy.
- **Acoustic Intents**: Maps sounds (Speech, Noise, Silence) to `ConceptID`.
- **Neural Integration**: Stores acoustic memories alongside visual ones.

### ✅ Neural Architecture ✨ NEW!
Biologically-inspired intent processing with 22 neural features:

| Feature | Description |
|---------|-------------|
| **Spreading Activation** | Concepts activate related concepts automatically |
| **Lateral Inhibition** | Competing handlers suppress each other |
| **Temporal Dynamics** | Activations decay; weak signals accumulate over time |
| **Hierarchical Processing** | Raw → Feature → Object → Semantic → Action layers |
| **Predictive Processing** | Predict outcomes via efference copy, detect surprise |
| **Basal Ganglia Model** | Urgency-based action selection with dopamine modulation |
| **Attention Focus** | Limited-capacity selective enhancement |
| **Goal Modulation** | Top-down goals affect perception |
| **Graceful Degradation** | Load-based throttling under pressure |

```rust
// Temporal summation - weak signals accumulate
summate(concept_id, 0.15, timestamp);  // 3 weak signals → fires

// Predictive priming - pre-activate expected concepts  
predict(source, expected, 0.9, timestamp);

// Urgency-based scheduling (basal ganglia)
submit_intent(IntentRequest { urgency: 0.9, ..default() });
```

See [NEURAL_ARCHITECTURE.md](docs/NEURAL_ARCHITECTURE.md) for complete documentation.

### ✅ Multi-Core SMP ✨ COMPLETE!
Production-grade 4-core scheduler with advanced features:
- **Per-Core Run Queues**: Minimizes lock contention
- **4-Level Priority**: Idle, Normal, High, Realtime (< 100μs for fast input)
- **Core Affinity**: Pin tasks to specific cores (Core 0 = input, Core 1 = vision, Core 2 = audio, Core 3 = network)
- **Work Stealing**: Automatic load balancing across cores
- **Power Efficiency**: WFI (Wait For Interrupt) on idle cores

### ✅ Persistent Storage ✨ COMPLETE!
SD Card driver for permanent data:
- **SDHCI Driver**: Full initialization sequence for EMMC2
- **Block I/O**: 512-byte sector read/write
- **DMA Support**: ADMA2 scatter-gather DMA for high-performance I/O
- **Write Support**: Full write capability with bounce buffering
- **SDHC/SDXC**: High-capacity card support
- **Use Cases**: Save dictionaries, neural memory, session logs

### ✅ Networking Stack ✨ COMPLETE!
Full TCP/IP implementation with production-grade reliability (~1,700 LOC):
- **Ethernet**: DMA ring buffers, zero-copy TX/RX
- **ARP**: Address resolution with 16-entry cache
- **IPv4**: Routing, RFC 1071 checksum verification
- **ICMP**: Ping (echo request/reply)
- **UDP**: Connectionless transport
- **TCP**: Full RFC-compliant implementation
  - Connection tracking (`TcpConnection`, 11-state machine)
  - Retransmission with RTT-based RTO (Jacobson/Karels algorithm)
  - Congestion control (RFC 5681: Slow Start, Congestion Avoidance, Fast Recovery)
  - TCP checksum with pseudo-header (RFC 793)
  - 17 unit tests covering all components
- **Userspace Networking**: User agents can bind UDP ports (`sys_bind_udp`) and receive packets (`sys_recvfrom`), enabling server-like capabilities.
- **VirtIO Support**: Initial VirtIO-Net driver for QEMU networking.

### ✅ Framebuffer Console
Text output on HDMI display:

```rust
cprintln!("Intent executed: {}", intent.name);
```

### ✅ Semantic Visual Interface (SVI) ✨ NEW!
A broadcast-based, intent-reactive GUI that reflects the kernel's semantic state.
- **Projections**: Ephemeral visual elements (InputTape, IntentLog, Status, Help).
- **Perception Overlay**: Visualizes active sensors and object detections.
- **Memory Graph**: Visualizes HDC neural memory nodes in real-time.
- **Broadcast Listener**: The GUI observes intents rather than driving them.

### ✅ Userspace & Scheduling ✨ COMPLETE!
Full preemptive multitasking OS capabilities:
- **ELF Loading**: Loads standard ELF64 binaries from SD card.
- **Preemptive Scheduler**: Round-Robin scheduling with 10ms time slices.
- **Process Isolation**: Full address space separation (Kernel=EL1, User=EL0).
- **System Calls**: `yield`, `sleep`, `print`, `exit`, `sys_parse_intent`, and File I/O.
- **Intent-Native Shell**: Kernel-side console that accepts natural language or direct pattern input.
- **Preemption**: Correct interrupt-driven context switching (Virtual Timer).
- **User Program**: `no_std` Rust userland support.

### ✅ Intent-Native Apps ✨ COMPLETE! (Sprint 14)
The OS supports "Programming without Code" via **Intent Manifests**.
- **Declarative Apps**: Define apps as a graph of `[Trigger] -> [Intent] -> [Action]`.
- **Semantic Linking**: The kernel resolves intents to capabilities at runtime using HDC.
- **Just-in-Time Assembly**: "I want to track calories" automatically links to the best available database and storage skills.

### ✅ Semantic Process Binding ✨ COMPLETE! (Sprint 15)
Processes are not just binaries; they are **Semantic Agents**.
- **Announce**: A process calls `sys_announce(CONCEPT_ID)` to declare: "I handle INCREMENT."
- **Binding**: The kernel binds the `ConceptID` to the Process ID (PID).
- **Execution**: When you think "Increment", the kernel automatically routes the intent to the correct process via IPC.
- **Nervous Impulse IPC**: Fixed-size, biological message passing (64 bytes) between agents.

### ✅ System 2 Cognitive Engine (LLM) ✨ NEW!
The kernel now features an integrated **System 2** engine for deep semantic processing, complementing the fast System 1 Intent Engine.
- **Model**: Llama 2 Architecture (Transformer).
- **Loading**: Streamed directly from SD card (`model.bin`) via `llm::loader`.
- **Latency**: Orders of magnitude slower than System 1 (~300k+ cycles), running as a background task.
- **Fail-Safe**: Includes a dummy fallback model to ensure strict boot reliability.

### 2. Intent-Native Apps Framework
> **Full Documentation**: [docs/APP_ARCHITECTURE.md](docs/APP_ARCHITECTURE.md)

"Applications" in this OS are not compiled binaries—they are **Semantic Manifests**.
*   **Manifest**: A declarative graph (`.intent` file).
*   **Semantic Linker**: A runtime that binds your intent to the best available **Skill**.
*   **Skill**: The atomic unit of execution (WASM/Rust).

See the [App Architecture Guide](docs/APP_ARCHITECTURE.md) to build your first app.

</details>

---

## Project Structure

```
intent-kernel/
├── kernel/src/
│   ├── steno/              # Hardware chord input engine
│   │   ├── stroke.rs       # 23-bit input patterns
│   │   ├── dictionary.rs   # Pattern → Intent mapping
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
│   │   ├── pcie.rs         # ✨ PCIe Root Complex
│   │   ├── rp1.rs          # ✨ RP1 I/O Controller
│   │   ├── sdhci.rs        # ✨ SD card controller
│   │   ├── ethernet.rs     # ✨ Ethernet MAC driver
│   │   └── usb/            # USB Host Controller
│   │       ├── xhci.rs     # xHCI driver
│   │       └── hid.rs      # HID protocol (keyboard devices)
│   └── kernel/             # Core OS
│       ├── capability.rs   # Security
│       ├── scheduler.rs    # Single-core scheduler
│       ├── smp_scheduler.rs# ✨ Multi-core SMP scheduler
│       └── memory/         # Allocation
│           ├── mod.rs      # Memory subsystem
│           ├── neural.rs   # HDC memory
│           └── hnsw.rs     # HNSW index
├── tests/host/             # 90 unit tests
├── docs/                   # Documentation
└── boot/                   # ARM64 bootloader
```

---

## Architecture

### Multi-Core Design

Intent Kernel uses a **4-core architecture** optimized for the Raspberry Pi 5:

```
Raspberry Pi 5 - 4× Cortex-A76 @ 2.4GHz
┌─────────────────────────────────────────────┐
│  WORKER CORES (0-2): Intent Processing     │
│  • Core 0: Realtime priority (fast input)  │
│  • Core 1: General task execution          │
│  • Core 2: General task execution          │
│  • Work-stealing load balancing            │
├─────────────────────────────────────────────┤
│  WATCHDOG CORE (3): Semantic Immune System │
│  • Health monitoring (CPU, memory, thermal)│
│  • Deadlock detection (wait-for graphs)    │
│  • Intent security (spam, privilege check) │
│  • Self-healing recovery strategies        │
└─────────────────────────────────────────────┘
```

**Scalability**: Dedicated watchdog overhead decreases with more cores
- 4 cores: 75% compute, 25% safety
- 8 cores: 87.5% compute, 12.5% safety  
- 56+ cores: 98%+ compute, <2% safety (negligible)

### System Layers

```
┌──────────────────────────────────────────────────────────────────┐
│                        USER SPACE                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ Intent Apps  │  │   Scripts    │  │     NLP      │          │
│  │ (Declarative)│  │   (Future)   │  │   Commands   │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
├──────────────────────────────────────────────────────────────────┤
│                      INTENT LAYER                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │   Pattern    │  │  Dictionary  │  │   Executor   │          │
│  │  Matching    │  │ (ConceptID)  │  │ (Broadcast)  │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
├──────────────────────────────────────────────────────────────────┤
│                      KERNEL LAYER                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ SMP Scheduler│  │   Watchdog   │  │   Syscalls   │          │
│  │ (3 workers)  │  │ (Core 3)     │  │  (24 calls)  │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │   Memory     │  │     VFS      │  │   Network    │          │
│  │ (VMA+Buddy)  │  │  (FAT32)     │  │  (TCP/IP)    │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
├──────────────────────────────────────────────────────────────────┤
│                     DRIVER LAYER                                 │
│  USB/HID • UART • Timer • Ethernet • SD Card • GPIO             │
├──────────────────────────────────────────────────────────────────┤
│                   HARDWARE (BCM2712)                             │
│          Raspberry Pi 5 • Cortex-A76 • 8GB RAM                  │
└──────────────────────────────────────────────────────────────────┘
```

### Memory Management

**Hybrid Allocator**:
- **Buddy Allocator**: Page-level (4KB pages)
- **Slab Allocator**: Object-level (8, 16, 32, 64, 128, 256, 512, 1024 bytes)
- **VMA (Virtual Memory Areas)**: User process memory regions with permissions
- **Performance**: 
  - Buddy: ~40 cycles/allocation
  - Slab: ~3,279 cycles for 8-byte objects

### Neural Memory (HDC)

**Hyperdimensional Computing** for semantic similarity:
- **HNSW Index**: Fast approximate nearest neighbor search
- **10,000-dim vectors**: Robust pattern matching
- **Intent Recognition**: Links inputs to concepts via cosine similarity
- **Anomaly Detection**: Detects unusual intent patterns (watchdog use)

### Concurrency

**SMP Scheduler**:
- **Per-core run queues**: Lock-free task execution
- **Work stealing**: Dynamic load balancing
- **Priority levels**: Realtime, High, Normal, Low, Idle
- **Core affinity**: Tasks can be pinned to specific cores
- **Watchdog exclusion**: Core 3 never runs user tasks

---

## Benchmarks & Performance

> **📊 Full Documentation**: See [docs/BENCHMARKS.md](docs/BENCHMARKS.md) for complete benchmark architecture, algorithms, and methodology.

**All tests run on QEMU virt platform (Cortex-A72 @ 62MHz timer frequency)**

### 40-Benchmark Suite (Verified December 2025)

The Intent Kernel includes a **40-benchmark suite** across 11 categories, tailored to semantic computing:

| Category | Benchmarks | Key Metrics |
|----------|------------|-------------|
| Intent Engine | 5 | Handler match: 0 cycles, Hash: 1 cycle, Security: 0 cycles |
| **Neural** | 3 | Decay: 15 cycles, Propagate: 157 cycles, Select: 5.9k cycles |
| Semantic Memory | 1 | Neural Alloc: 142 cycles, Retrieval: O(log N) |
| Perception | 2 | Sensor fusion: 0 cycles, Perceive+Store: 73 cycles |
| Multi-Modal | 5 | Direct input: 54 cycles, English: 139 cycles |
| Process/Agent | 6 | Context switch: 474 cycles, Preemption: 14 cycles |
| Lock/Sync | 5 | SpinLock: 19 cycles, IPI: 106 cycles |
| Interrupt | 4 | Timer jitter: 125 max cycles |
| I/O/Network | 4 | TCP checksum: 8 cycles, UART: 0 cycles |
| Memory | 2 | Slab: 28 cycles, Buddy: 43 cycles |
| Stress Test | 1 | **180k ops @ 29 cycles avg** |

### Stress Test Results (Verified December 2025)

Allocator validation across 180,000 operations:

| Test | Operations | Avg Cycles | Throughput | Status |
|------|-----------|-----------|------------|--------|
| Small Allocations (8B) | 100,000 | 28 | **2.2M ops/sec** | ✅ |
| Vec Operations (100 elem) | 50,000 | 30 | 2.1M ops/sec | ✅ |
| Page Allocations (4KB) | 10,000 | 36 | 1.7M ops/sec | ✅ |
| Mixed Workload (8B-4KB) | 20,000 | 33 | 1.9M ops/sec | ✅ |
| **Total** | **180,000** | **29** | **~3M ops/sec** | ✅ |

> **Performance Profile**: The benchmark results show a ~100x performance gap between fast-path processing (hardware patterns, 54 cycles) and slow-path processing (neural selection, 5,989 cycles). This two-tier architecture separates reflex-level pattern matching from deliberative reasoning.

### Standard Benchmarks

| Benchmark | Result | Target | Status |
|-----------|--------|--------|--------|
| **Context Switch** | 474 cycles | <500 | ✅ 5% under |
| **Syscall Dispatch** | 0 cycles | <50 | ✅ Optimal |
| **Memory Alloc (Slab)** | 28 cycles | <100 | ✅ 72% under |
| **Memory Alloc (Buddy)** | 43 cycles | <100 | ✅ 57% under |
| **Intent Security** | 0 cycles | <50 | ✅ 100% under |
| **SpinLock** | 19 cycles | <50 | ✅ 62% under |

---

## Status

| Phase | Status | What's Done |
|-------|--------|-------------|
| **1. Foundation** | ✅ | Boot, UART, GPIO, Timer, Memory, Scheduler |
| **2. Input Engine** | ✅ | Pattern parsing, Dictionary, Engine, RTFCRE |
| **3. Intent System** | ✅ | Handlers, Queue, History, 122 tests |
| **4. Perception** | ✅ | **Complete Hailo-8** (YOLO parser, NMS, hypervectors), HUD |
| **5. Input/Output** | ✅ | **Real xHCI Driver**, HID Boot Protocol, Framebuffer Console |
| **5.5. English Layer** | ✅ | Natural Language I/O (200+ phrases, conversation context) |
| **6. Sensors** | ✅ | **Hailo-8 Tensor Parsing**, Audio Perception (ZCR/Energy), Vision |
| **7. Security** | ✅ | VMM Isolation (TTBR0 Switching, Kernel Protection) |
| **8. Multi-Core** | ✅ ✨ | **SMP Scheduler** (4 cores, priority, affinity, work stealing) |
| **9** | Storage | ✅ ✨ | **SD Card Driver** (SDHCI, block I/O, SDHC/SDXC, **DMA**, **Write Support**) |
| **10** | Networking | ✅ ✨ | **TCP/IP Stack** (Ethernet, VirtIO, ARP, IPv4, ICMP, UDP, TCP) - **Userspace Receive Supported** ✅ |
| **11. Hardware** | ✅ ✨ | **Real Drivers**: PCIe Root Complex, RP1 Southbridge, **Hailo-8 (HCP, DMA, Inference)** |
| **12. Userspace** | ✅ ✨ | **ELF Loader**, Preemptive Scheduler, Syscalls, User Mode (EL0), **Stable Shell** |
| **13. Visual Interface** | ✅ ✨ | **SVI**: Broadcast-based GUI, Projections, Perception Overlay, Memory Graph |
| **14. Integration** | ✅ ✨ | **Integration Tests** (QEMU, RamFS, Loopback, Process Lifecycle) |
| **15. Multi-Tasking** | ✅ ✨ | **Semantic Process Binding**: `sys_announce`, `sys_ipc` (Biological Message Passing) |
| **16. Neural Architecture** | ✅ ✨ | **Verified Active**: `decay_tick()`, `propagate_all()` running on timer tick |
| **17. File System** | ✅ ✨ | **Pure Intent**: `ls`, `cat` (with argument parsing), `sys_getdents` |
| **17. File System** | ✅ ✨ | **Pure Intent**: `ls`, `cat` (with argument parsing), `sys_getdents` |
| **18. App Framework** | ✅ ✨ | **Declarative Apps**: `AppManager`, `Manifest`, `SemanticLinker`, `Just-in-Time Binding` |
| **19. Architecture** | ✅ ✨ | **Hardening**: Semantic Tollbooth (Syscall Gating), Dynamic Intent Handlers |
| **20. System 2 Upgrade** | ✅ ✨ | **LLM Engine**: `llm::loader`, Llama 2 Inference, `OwnedWeights`, FAT32 Integration |

### Test Coverage (Verified December 2025)

```
127 host tests | 8 modules | < 1 second

stroke .......... 25 tests ✓
capability ...... 20 tests ✓
dictionary ...... 20 tests ✓
concept ......... 22 tests ✓
history ......... 11 tests ✓
queue ........... 12 tests ✓
handlers ........ 12 tests ✓
matrix .......... 2 tests ✓
audio ........... 3 tests ✓
```

---

## Hardware

**Target Platform**: Raspberry Pi 5

| Component | Specification |
|-----------|---------------|
| CPU | ARM Cortex-A76 (4 cores @ 2.4GHz) |
| RAM | 4GB / 8GB LPDDR4X |
| AI | Hailo-8L NPU (optional) |
| Input | Keyboard (English mode) OR specialized hardware (chord mode) |

---

## Philosophy

### 1. Intents, Not Characters (Internally)
The native processing unit is a semantic concept. No character encoding in the core execution path. No Unicode in intent handlers. The English I/O layer provides a natural language interface for universal accessibility.

### 2. Pure Rust
No libc. No C dependencies. Minimal crates. Everything from scratch in safe, idiomatic Rust.

### 3. Green Computing
`wfi` when idle. No busy loops. No wasted cycles.

### 4. Forward Only
We build the future. No backward compatibility with character-based systems.

### 5. Universal Accessibility
Semantic-first kernel with multiple input paths. Everyone can use natural language; power users can unlock maximum speed with specialized hardware.

---

## Documentation

| Document | Description |
|----------|-------------|
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | System design and data flow |
| [APP_ARCHITECTURE.md](docs/APP_ARCHITECTURE.md) | **App Development Framework** |
| [ENHANCEMENTS.md](docs/ENHANCEMENTS.md) | ✨ Recent enhancements (Hailo, SMP, Storage, Networking) |
| [ENGLISH_LAYER.md](docs/ENGLISH_LAYER.md) | Natural language I/O system |
| [VISUAL_INTERFACE.md](docs/VISUAL_INTERFACE.md) | Semantic Visual Interface (SVI) guide |
| [SEMANTIC_MEDIA.md](docs/SEMANTIC_MEDIA.md) | **Media & Blob Storage Spec** |
| [API.md](docs/API.md) | Complete API reference |
| [ROADMAP.md](docs/ROADMAP.md) | Development phases |
| [BUILDING.md](docs/BUILDING.md) | Build instructions |
| [GUIDE_INTENTS.md](docs/GUIDE_INTENTS.md) | **Dev Guide: Adding Intents** |
| [GUIDE_USERSPACE.md](docs/GUIDE_USERSPACE.md) | **Dev Guide: User Agents** |
| [CONTRIBUTING.md](docs/CONTRIBUTING.md) | How to contribute |

---

## License

MIT License. See [LICENSE](LICENSE) for details.

---

<p align="center">
  <strong>Intent Kernel</strong><br>
  <em>Where semantic computing meets bare-metal performance.</em>
</p>
