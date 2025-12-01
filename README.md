# 🌟 Intent Kernel

> **Where Humans Speak and Silicon Listens**

A world-class, bare-metal operating system kernel for Raspberry Pi 5 — built completely from scratch with zero external dependencies.

```
╔═══════════════════════════════════════════════════════════════════════════════╗
║                                                                               ║
║   ██╗███╗   ██╗████████╗███████╗███╗   ██╗████████╗                           ║
║   ██║████╗  ██║╚══██╔══╝██╔════╝████╗  ██║╚══██╔══╝                           ║
║   ██║██╔██╗ ██║   ██║   █████╗  ██╔██╗ ██║   ██║                              ║
║   ██║██║╚██╗██║   ██║   ██╔══╝  ██║╚██╗██║   ██║                              ║
║   ██║██║ ╚████║   ██║   ███████╗██║ ╚████║   ██║                              ║
║   ╚═╝╚═╝  ╚═══╝   ╚═╝   ╚══════╝╚═╝  ╚═══╝   ╚═╝                              ║
║                                                                               ║
║   ██╗  ██╗███████╗██████╗ ███╗   ██╗███████╗██╗                               ║
║   ██║ ██╔╝██╔════╝██╔══██╗████╗  ██║██╔════╝██║                               ║
║   █████╔╝ █████╗  ██████╔╝██╔██╗ ██║█████╗  ██║                               ║
║   ██╔═██╗ ██╔══╝  ██╔══██╗██║╚██╗██║██╔══╝  ██║                               ║
║   ██║  ██╗███████╗██║  ██║██║ ╚████║███████╗███████╗                          ║
║   ╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝╚══════╝╚══════╝                          ║
║                                                                               ║
║                     The Bridge Between Mind and Machine                       ║
║                                                                               ║
╚═══════════════════════════════════════════════════════════════════════════════╝
```

## 🎯 Vision

In the age of AI, why do we still write code in rigid, unforgiving syntax?

**Intent Kernel** reimagines operating systems from first principles:

- **No files** — just capabilities
- **No processes** — just executing intents  
- **No shell** — just natural language interaction
- **No libraries** — everything built from scratch in pure Rust and ARM64 assembly

Express what you want. Let the silicon figure out how.

> **Next-Gen Update**: Now featuring a **True Reactive Core** (Green Computing) and **Vector Intent Space** (AI Native).

## 🏗️ Architecture

```
┌────────────────────────────────────────────────────────────────┐
│                      HUMAN LAYER                               │
│                  "Show me the temperature"                     │
└───────────────────────────────┬────────────────────────────────┘
                                │
                                ▼
┌────────────────────────────────────────────────────────────────┐
│                    INTENT ENGINE                               │
│   Parse → Understand → Map to Capabilities → Execute           │
└───────────────────────────────┬────────────────────────────────┘
                                │
                                ▼
┌────────────────────────────────────────────────────────────────┐
│                  CAPABILITY LAYER                              │
│   Unforgeable tokens granting fine-grained permissions         │
│   [Memory Cap] [Device Cap] [Display Cap] [Compute Cap]        │
└───────────────────────────────┬────────────────────────────────┘
                                │
                                ▼
┌────────────────────────────────────────────────────────────────┐
│                    KERNEL CORE                                 │
│   Memory Manager │ Scheduler │ IPC │ Interrupt Dispatch        │
└───────────────────────────────┬────────────────────────────────┘
                                │
                                ▼
┌────────────────────────────────────────────────────────────────┐
│                   DRIVER LAYER                                 │
│   UART │ GPIO │ Timer │ GIC-400 │ Mailbox │ RamDisk │ PCIe   │
└───────────────────────────────┬────────────────────────────────┘
                                │
                                ▼
┌────────────────────────────────────────────────────────────────┐
│                  SILICON (BCM2712) + AI HAT                    │
│   ARM Cortex-A76 × 4 │ VideoCore VII │ Hailo-8 (26 TOPS)       │
└────────────────────────────────────────────────────────────────┘
```

## 🎨 Features

### ✅ Implemented

- **Multi-core ARM64 Boot** — EL3→EL2→EL1 transitions, all 4 cores
- **Full Exception Handling** — Sync, IRQ, FIQ, SError vectors
- **PL011 UART Driver** — Serial console with interrupt support
- **GPIO Driver** — All 58 pins, pull config, edge detection
- **System Timer** — ARM Generic Timer with deadlines/stopwatch
- **GIC-400 Interrupts** — Full interrupt controller support
- **VideoCore Mailbox** — GPU communication, power, clocks, temperature
- **Framebuffer** — 4K display, drawing primitives, text rendering
- **Memory Allocator** — Buddy + slab allocator, DMA support
- **Capability System** — Unforgeable tokens, delegation, revocation
- **Intent Engine** — Vector-based semantic understanding (Embeddings + Cosine Similarity)
- **Reactive Core** — Async/Await executor with interrupt-driven sleeping (WFI)
- **Polymorphic Kernel** — Heap ASLR and Pointer Guard (Encrypted Capabilities) using Hardware RNG
- **Virtual Memory** — ARM64 VMSA with 4-level page tables and 4KB granularity (Core VMM)
- **Advanced Exception Handling** — Detailed fault decoding (ESR/FAR) and crash reporting.
- **Process Isolation**: Kernel threads and User Mode (EL0) processes.
- **Preemptive Multitasking**: Round-Robin scheduler with Timer Interrupts.
- **System Calls**: Basic SVC interface for User Mode interaction with **Pointer Validation**.
- [x] **Semantic Memory**: **Neural Allocator** that stores data by meaning (vector embeddings) rather than address.
- [x] **Dynamic Intents**: Support for arbitrary string storage (`remember "..."`).
- [x] **Production Hardening**: Unit tests for core logic and robust error handling.
- [x] **Adaptive Perception Layer**: Hardware-agnostic AI support (Hailo-8 / CPU Fallback).
- **Persistent Storage**: TAR-based RamDisk with Read-Write Overlay (`create`, `edit`, `delete`).
- **PCIe Root Complex**: BCM2712 PCIe driver for hardware enumeration and device discovery.
- **Security Hardening**:
  - **Thread Safety**: Global `Mutex` protection for shared resources.
  - **Capability Enforcement**: Strict checks for sensitive operations.
  - **Input Validation**: Sanitization of user inputs and filenames.
- **Testing Infrastructure**:
  - Custom test framework for bare-metal (`#![custom_test_frameworks]`)
  - QEMU virt machine with semihosting exit (clean termination)
  - 10-second timeout to prevent runaway tests
  - 14 unit tests (memory, capability, intent)
  - `make test-unit` completes in <10 seconds

### 🚧 Roadmap

- [/] Virtual Memory & Process Isolation (v0.2.0)
- [ ] Intent vocabulary expansion with AI integration
- [ ] Persistent storage driver (SD card, NVMe)
- [ ] Network stack (Ethernet, WiFi)
- [ ] GPU compute integration (VideoCore VII shaders)
- [ ] Multi-core task scheduling
- [ ] USB device support
- [ ] Audio driver
- [ ] Camera interface

## 🚀 Quick Start

### Prerequisites

```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add aarch64-unknown-none

# Install ARM toolchain (macOS)
brew install --cask gcc-arm-embedded

# Install ARM toolchain (Ubuntu)
sudo apt install gcc-aarch64-linux-gnu binutils-aarch64-linux-gnu
```

### Build

```bash
# Clone
git clone https://github.com/sutraworks/intent-kernel
cd intent-kernel

# Build
make

# Or step by step:
make boot    # Assemble bootloader
make kernel  # Build Rust kernel
make image   # Create kernel8.img
```

### Deploy to Raspberry Pi 5

1. Format SD card with FAT32 boot partition
2. Copy `build/kernel8.img` to the boot partition
3. Create `config.txt`:
   ```
   arm_64bit=1
   kernel=kernel8.img
   enable_uart=1
   ```
4. Connect USB-to-serial adapter (GPIO 14/15)
5. Power on and connect at 115200 baud

### Run in QEMU

```bash
make run    # Limited - Pi 5 not fully emulated
```

## 📁 Project Structure

```
intent-kernel/
├── boot/
│   ├── boot.s          # ARM64 bootloader (multi-core, exception vectors)
│   └── linker.ld       # Memory map (8GB, kernel at 0x80000)
├── kernel/
│   ├── Cargo.toml      # Rust manifest (no dependencies!)
│   └── src/
│       ├── main.rs     # Kernel entry point
│       ├── arch/       # ARM64 specifics
│       │   └── mod.rs  # Spinlock, barriers, interrupt control
│       ├── drivers/    # Hardware drivers
│       │   ├── mod.rs  # BCM2712 memory map
│       │   ├── uart.rs # Serial console
│       │   ├── gpio.rs # GPIO pins
│       │   ├── timer.rs # System timer
│       │   ├── interrupts.rs # GIC-400
│       │   ├── mailbox.rs    # VideoCore mailbox
│       │   └── framebuffer.rs # Display driver
│       ├── kernel/     # Core subsystems
│       │   ├── mod.rs
│       │   ├── memory.rs     # Buddy/slab allocator
│       │   └── capability.rs # Capability system
│       └── intent/     # Intent engine
│           └── mod.rs  # Parser and executor
├── config/
│   └── config.txt      # Boot configuration
├── docs/
│   ├── ARCHITECTURE.md # System design deep-dive
│   ├── BUILDING.md     # Complete build guide
│   ├── API.md          # Full API reference
│   ├── HARDWARE.md     # BCM2712 hardware reference
│   ├── EXAMPLES.md     # Code examples
│   ├── SECURITY.md     # Capability security model
│   ├── CONTRIBUTING.md # Contribution guidelines
│   └── ROADMAP.md      # Development roadmap
├── Makefile            # Build system
├── CHANGELOG.md        # Version history
└── README.md
```

## 💡 Using Intents

Once booted, interact naturally:

```
intent> help
╔═══════════════════════════════════════════════════════════╗
║                    INTENT KERNEL HELP                     ║
╠═══════════════════════════════════════════════════════════╣
║  Just say what you want. For example:                     ║
║    show "Hello World"                                     ║
║    what is the temperature                                ║
║    calculate 42 squared                                   ║
╚═══════════════════════════════════════════════════════════╝

intent> show "Hello, World!"
✓ Text displayed

intent> what is the temperature
= 45°C

intent> calculate 42 squared
= 1764

intent> double 100
= 200

intent> remember "my secret data"
✓ Data stored

intent> recall secret
✓ Found Concept (Semantic Match)

intent> status
╔═══════════════════════════════════════════════════════════╗
║                    SYSTEM STATUS                          ║
║  Uptime:        12345 ms                                  ║
║  Core:          0 (EL1)                                   ║
║  Memory Used:   4096 bytes                                ║
║  Capabilities:  5 active, 0 revoked                       ║
║  Temperature:   45°C                                      ║
╚═══════════════════════════════════════════════════════════╝

intent> about
╔═══════════════════════════════════════════════════════════╗
║                   INTENT KERNEL v0.1.0                    ║
║  A capability-based operating system where humans         ║
║  express intent, not instructions.                        ║
║  Created by Sutraworks                                    ║
╚═══════════════════════════════════════════════════════════╝
```

### Intent Categories

| Category | Keywords | Examples |
|----------|----------|----------|
| Display | show, display, print, draw | `show "hello"`, `display temperature` |
| Query | what, how, status, tell me | `what is the time`, `status` |
| Compute | calculate, add, multiply, square | `calculate 5 squared`, `double 42` |
| Store | store, save, remember, keep | `remember "my data"` |
| Retrieve | get, load, fetch, recall, retrieve | `recall data` |
| System | restart, shutdown, clear | `restart`, `clear` |
| Communicate | send, transmit | `send "message"` |

## 🔐 Capability System

Everything in Intent Kernel is protected by capabilities:

```rust
// Capabilities are unforgeable tokens
let mem_cap = mint_root(CapabilityType::Memory, base, size, Permissions::READ_WRITE);

// Derive restricted capabilities
let read_only = derive(&mem_cap, Permissions::READ)?;

// Use capabilities for all operations
memory::read(&mem_cap, offset, &mut buffer)?;

// Revoke entire capability tree
revoke(&mem_cap);
```

## 🛠️ Development

```bash
make check    # Check code without building
make fmt      # Format all code
make lint     # Run clippy lints
make doc      # Generate documentation
make disasm   # Disassemble kernel
make info     # Show binary info
```

## 📊 Technical Specifications

### Hardware Requirements
| Specification | Value |
|--------------|-------|
| Target | Raspberry Pi 5 |
| SoC | BCM2712 |
| CPU | ARM Cortex-A76 × 4 @ 2.4GHz |
| Memory | 8GB LPDDR4X |
| GPU | VideoCore VII |
| Storage | MicroSD (FAT32 boot) |

### Software Architecture
| Component | Details |
|-----------|--------|
| Boot Mode | AArch64 EL2 → EL1 |
| Kernel Load Address | 0x80000 |
| Stack | 0x80000 (grows down) |
| Heap | 0x20_0000 - 0x1_0000_0000 |
| DMA Region | 0x1_0000_0000 - 0x1_1000_0000 |
| GPU Shared Memory | 0x1_1000_0000 - 0x1_2000_0000 |
| Peripheral Base | 0x1_0000_0000 (BCM2712) |
| GIC-400 Base | 0x1_0004_0000 |
| Dependencies | **Zero** |

### Memory Map
```
0x0000_0000_0000 ┌────────────────────┐
                 │    Reserved        │
0x0000_0008_0000 ├────────────────────┤
                 │    Kernel Code     │  ← Entry point
                 │    Kernel Data     │
                 │    Kernel BSS      │
0x0000_0020_0000 ├────────────────────┤
                 │    Heap            │  ← Buddy + Slab allocator
                 │    (grows up)      │
0x0001_0000_0000 ├────────────────────┤
                 │    DMA Buffers     │  ← Cache-coherent
0x0001_1000_0000 ├────────────────────┤
                 │    GPU Shared      │  ← VideoCore communication
0x0001_2000_0000 ├────────────────────┤
                 │    Intent Memory   │  ← Intent engine workspace
0x0002_0000_0000 └────────────────────┘
```

## 🌟 Philosophy

> "The best interface is no interface."

Traditional operating systems are artifacts of a time when humans had to speak machine. They use:
- **Files** — because storage was precious
- **Processes** — because multitasking was hard
- **Shell commands** — because keyboards were all we had

**Intent Kernel** starts fresh. In an era of AI and natural interaction, we ask:

*What if the OS understood us instead of us learning it?*

## � API Reference

### Architecture Module (`arch`)
```rust
arch::core_id()           // Get current CPU core (0-3)
arch::exception_level()   // Get current EL (1-3)
arch::irq_enable()        // Enable interrupts
arch::irq_disable()       // Disable interrupts
arch::wfi()               // Wait for interrupt (low power)
arch::wfe()               // Wait for event
arch::dmb()               // Data memory barrier
arch::dsb()               // Data synchronization barrier
arch::SpinLock::new()     // Create spinlock for multicore
```

### Driver Modules
```rust
// UART (Serial Console)
drivers::uart::early_init()        // Initialize UART
drivers::uart::send(byte)          // Send byte
drivers::uart::receive()           // Receive byte (blocking)
drivers::uart::read_byte_async()   // Async read (yields to executor)
drivers::uart::read_line(&mut buf) // Read line of input
kprintln!("Hello {}", name);       // Print macro

// GPIO
drivers::gpio::init()              // Initialize GPIO
drivers::gpio::activity_led(on)    // Control activity LED
drivers::gpio::set_output(pin, high)
drivers::gpio::read_input(pin)

// Timer
drivers::timer::init()             // Initialize timer
drivers::timer::uptime_ms()        // Milliseconds since boot
drivers::timer::delay_ms(100)      // Delay milliseconds
drivers::timer::Deadline::from_now_ms(1000)
drivers::timer::Stopwatch::start()

// Interrupts (GIC-400)
drivers::interrupts::init()        // Initialize GIC
drivers::interrupts::enable(irq)   // Enable interrupt
drivers::interrupts::disable(irq)  // Disable interrupt

// Mailbox (GPU Communication)
drivers::mailbox::init()
drivers::mailbox::get_temperature()     // Returns millidegrees C
drivers::mailbox::get_board_info()      // Board model, revision, memory
drivers::mailbox::get_clock_rate(id)    // Get clock frequency

// Framebuffer
drivers::framebuffer::init(1920, 1080, 32)
drivers::framebuffer::clear(Color::BLACK)
drivers::framebuffer::draw_text(x, y, "Hello", Color::WHITE)
```

### Kernel Modules
```rust
// Memory Allocator
kernel::memory::init()             // Initialize allocator
kernel::memory::stats()            // Get allocation stats
kernel::memory::heap_available()   // Available heap bytes

// Capability System
kernel::capability::init()
kernel::capability::mint_root(type, resource, size, perms)
kernel::capability::derive(&cap, new_perms)
kernel::capability::revoke(&cap)
kernel::capability::validate(&cap)
```
## 📚 Documentation

| Document | Description |
|----------|-------------|
| [Architecture](docs/ARCHITECTURE.md) | System design, boot sequence, memory layout |
| [Building](docs/BUILDING.md) | Prerequisites, build commands, deployment |
| [API Reference](docs/API.md) | Complete module and function documentation |
| [Hardware](docs/HARDWARE.md) | BCM2712 registers, GPIO pinout, peripherals |
| [Examples](docs/EXAMPLES.md) | Practical code examples for all subsystems |
| [Security](docs/SECURITY.md) | Capability-based security model explained |
| [Contributing](docs/CONTRIBUTING.md) | Code style, PR process, guidelines |
| [Roadmap](docs/ROADMAP.md) | Development phases and future plans |
| [Changelog](CHANGELOG.md) | Version history and release notes |
## �📜 License

MIT License — Because knowledge should be free.

## 🤝 Contributing

This project exists at the intersection of systems programming and AI. Contributions welcome in:

- Expanding the intent vocabulary
- Adding new drivers
- Improving the capability model
- GPU compute integration
- Documentation and examples

## 🙏 Acknowledgments

Built with passion at **Sutraworks**.

*"The future doesn't need programmers. It needs people with intent."*

---

<p align="center">
  <strong>Intent Kernel</strong><br>
  Where Humans Speak and Silicon Listens<br>
  <sub>Made with ❤️ for Raspberry Pi 5</sub>
</p>
