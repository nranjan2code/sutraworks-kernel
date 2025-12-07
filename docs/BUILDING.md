# Building Intent Kernel

Complete guide to building Intent Kernel from source.

## Prerequisites

### Rust Toolchain

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add AArch64 bare metal target
rustup target add aarch64-unknown-none

# Install nightly (required for some features)
rustup install nightly
rustup default nightly

# Add required components
rustup component add rust-src llvm-tools-preview
```

### ARM Cross-Compiler

**macOS:**
```bash
# Using Homebrew
brew install --cask gcc-arm-embedded

# Or download directly from ARM
# https://developer.arm.com/downloads/-/arm-gnu-toolchain-downloads
```

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install gcc-aarch64-linux-gnu binutils-aarch64-linux-gnu
```

**Windows (WSL2 recommended):**
```bash
# In WSL2 Ubuntu
sudo apt install gcc-aarch64-linux-gnu binutils-aarch64-linux-gnu
```

**Arch Linux:**
```bash
sudo pacman -S aarch64-linux-gnu-gcc aarch64-linux-gnu-binutils
```

### QEMU (Optional, for emulation)

**macOS:**
```bash
brew install qemu
```

**Ubuntu/Debian:**
```bash
sudo apt install qemu-system-aarch64
```

## Building

### Quick Build

```bash
# Clone repository
git clone https://github.com/sutraworks/intent-kernel
cd intent-kernel

# Build everything
make

# Output: build/kernel8.img
```

### Step-by-Step Build

```bash
# 1. Assemble bootloader
make boot
# Output: build/boot.o

# 2. Build Rust kernel
make kernel
# Output: kernel/target/aarch64-unknown-none/release/libintent_kernel.a

# 3. Link everything
make image
# Output: build/kernel8.img
```

### Build Options

```bash
# Release build (default, optimized)
make

# Debug build
PROFILE=debug make

# Clean build
make clean && make

# Check code without building
make check

# Format code
make fmt

# Run clippy lints
make lint
```

## Build Artifacts

After successful build:

```
build/
├── boot.o          # Assembled bootloader
├── kernel.elf      # Linked ELF binary (with debug symbols)
└── kernel8.img     # Raw binary for Pi 5

kernel/target/aarch64-unknown-none/release/
└── libintent_kernel.a    # Rust static library
```

## Deployment

### To SD Card

1. **Prepare SD Card:**
   - Use Raspberry Pi Imager or similar tool
   - Create a FAT32 partition labeled "boot"

2. **Copy Files:**
   ```bash
   # macOS (adjust mount point as needed)
   make install SD_MOUNT=/Volumes/boot
   
   # Or manually
   cp build/kernel8.img /Volumes/boot/
   cp config/config.txt /Volumes/boot/
   ```

3. **Required Boot Files:**
   Download from [Raspberry Pi firmware](https://github.com/raspberrypi/firmware/tree/master/boot):
   - `bootcode.bin`
   - `start4.elf`
   - `fixup4.dat`

4. **Final SD Card Contents:**
   ```
   /boot/
   ├── bootcode.bin      # From RPi firmware
   ├── start4.elf        # From RPi firmware
   ├── fixup4.dat        # From RPi firmware
   ├── config.txt        # From our config/
   └── kernel8.img       # Our kernel
   ```

### Serial Connection

Connect a USB-to-TTL serial adapter:

| Pi 5 GPIO | Serial Adapter |
|-----------|----------------|
| Pin 6 (GND) | GND |
| Pin 8 (GPIO14/TX) | RX |
| Pin 10 (GPIO15/RX) | TX |

**⚠️ IMPORTANT:** Pi 5 uses 3.3V logic. Do NOT connect 5V serial adapters directly!

Connect with:
```bash
# macOS
screen /dev/tty.usbserial-* 115200

# Linux
screen /dev/ttyUSB0 115200

# Or use minicom, picocom, etc.
```

## Emulation

### QEMU (Limited Support)

QEMU doesn't fully emulate Raspberry Pi 5 (BCM2712), but can be used for basic testing:

```bash
# Run in QEMU
make run

# Run with GDB server (for debugging)
make debug

# Connect GDB
aarch64-none-elf-gdb build/kernel.elf
(gdb) target remote localhost:1234
(gdb) break kernel_main
(gdb) continue
```

**Limitations:**
- No BCM2712 peripherals emulation
- No VideoCore VII
- Some Pi 5 specific features won't work

## Troubleshooting

### "aarch64-none-elf-as: command not found"

The ARM toolchain isn't installed or not in PATH:
```bash
# Check installation
which aarch64-none-elf-as

# macOS: ensure Homebrew bin is in PATH
export PATH="/opt/homebrew/bin:$PATH"

# Or use the Linux toolchain name
export CROSS_COMPILE=aarch64-linux-gnu-
```

### "error[E0463]: can't find crate for `core`"

Missing Rust target:
```bash
rustup target add aarch64-unknown-none
```

### Build succeeds but kernel doesn't boot

1. **Check serial output** - Connect serial first, then power on
2. **Verify config.txt** - Must have `arm_64bit=1` and `kernel=kernel8.img`
3. **Check firmware files** - All boot files present on SD card?
4. **Try a simpler test** - Use a known-working kernel image first

### Rust features errors

Ensure nightly toolchain:
```bash
rustup default nightly
# Or use +nightly flag
cargo +nightly build
```

## Testing

### Run Unit Tests

```bash
# Run host tests (127 tests, runs on your machine)
make test

# Run 40-benchmark suite in QEMU
make run

# Output: Tests complete in <10 seconds

### Running the User Shell

The **User Mode Shell** launches automatically after boot (following any benchmarks). It runs as an isolated process (EL0).

**Commands:**
- `help`: Triggers kernel's `handle_help` intent.
- `echo <msg>`: Example of text processing.
- `status`: Show system status (via Kernel Intent).
- `panic`: Trigger a user-mode panic (tests fault handling).

**Note**: The shell is "Intent-Native". It sends your text directly to the Kernel's `EnglishParser`. You can type "show status" or "what time is it" just like you would on the main console.

```bash
make run
# Wait for boot...
# shell> help
```
```

### Running Benchmarks

The kernel includes a **40-benchmark suite** that runs automatically at boot:

```bash
# Run benchmarks (output appears in console)
make run

# Benchmarks complete before "INTENT KERNEL READY"
```

**Categories:** Intent Engine (5), Semantic Memory (1), Perception (2), Multi-Modal (5), Process (6), Lock (5), Interrupt (4), I/O (4), Memory (2), Stress Test (1).

> **📊 Full Documentation**: See [BENCHMARKS.md](BENCHMARKS.md) for architecture, algorithms, and methodology.

### Test Architecture

The testing infrastructure uses:
- **Host tests**: Run on your development machine using `make test`
- **QEMU benchmarks**: Full kernel boot tests in emulator with 40 benchmarks
- **Custom allocator bypass**: `#[cfg(not(test))]` on `#[global_allocator]` allows host tests to use standard library allocator
- **Custom test framework**: `#![custom_test_frameworks]` for bare-metal QEMU tests

### Current Tests (127 host tests)

| Module | Tests | Description |
|--------|-------|-------------|
| Audio | 3 | Noise, silence, speech detection |
| Handlers | 12 | Registry, dispatch, capabilities |
| History | 11 | Undo/redo, ring buffer |
| Matrix | 2 | LSH property, determinism |
| Queue | 12 | Priority, deadline, FIFO |
| Capability | 20 | Permissions, revocation, derive |
| Concept | 22 | Hashing, categories, display |
| Dictionary | 20 | Lookup, entries, steno sequences |
| Stroke | 25 | Parsing, RTFCRE, key order |

### Host Test Fix

The kernel's custom `KernelAllocator` requires bare-metal initialization. For host tests:
- Added `#[cfg(not(test))]` to `#[global_allocator]`
- Tests now use standard library allocator
- All 18 TCP tests pass on host

### Limitations

Full kernel tests require the complete boot sequence. Options:

1. **Host tests** (Default): `cargo test --lib` (most code)
2. **QEMU tests**: `make test-integration` (boot sequence)
3. **Hardware tests**: Run on actual Raspberry Pi 5

## Development Workflow

### Adding a New Driver

1. Create `kernel/src/drivers/mydriver.rs`
2. Add `pub mod mydriver;` to `kernel/src/drivers/mod.rs`
3. Implement driver following existing patterns
4. Test with `make && make install`

### Adding a New Intent Category

1. Add variant to `IntentCategory` enum in `kernel/src/intent/mod.rs`
2. Add detection keywords in `detect_category()`
3. Add execution handler in `IntentEngine::execute()`
4. Test interactively via serial console

### Debugging

```bash
# Generate disassembly
make disasm
# Output: build/kernel.asm

# Show symbol table
make symbols

# Show section sizes
make info
```

## Continuous Integration

Example GitHub Actions workflow:

```yaml
name: Build Intent Kernel

on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-action@stable
        with:
          targets: aarch64-unknown-none
          
      - name: Install ARM toolchain
        run: sudo apt-get install -y gcc-aarch64-linux-gnu
        
      - name: Build
        run: make
        
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: kernel8.img
          path: build/kernel8.img
```

---

*Last updated: December 2025*
