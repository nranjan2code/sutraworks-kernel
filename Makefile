# ═══════════════════════════════════════════════════════════════════════════════
#  INTENT KERNEL - Build System
# ═══════════════════════════════════════════════════════════════════════════════
#
#  A world-class bare-metal kernel for Raspberry Pi 5
#  No dependencies. No legacy. Just intent and silicon.
#

# ═══════════════════════════════════════════════════════════════════════════════
# CONFIGURATION
# ═══════════════════════════════════════════════════════════════════════════════

# Target: AArch64 bare metal
TARGET := aarch64-unknown-none

# Tools - Use LLVM/Clang (no third-party toolchain needed)
# Clang can cross-compile to any target
CC = clang
AS = clang
LD = rust-lld -flavor gnu
# Use llvm-objcopy from rustup nightly (stable doesn't include it by default)
OBJCOPY = $(shell find ~/.rustup/toolchains -name "llvm-objcopy" 2>/dev/null | head -1)
OBJDUMP = llvm-objdump

# Directories
BOOT_DIR := boot
KERNEL_DIR := kernel
BUILD_DIR := build
TARGET_DIR := $(KERNEL_DIR)/target/$(TARGET)

# Output files
KERNEL_ELF := $(BUILD_DIR)/kernel.elf
KERNEL_IMG := $(BUILD_DIR)/kernel8.img
KERNEL_BIN := $(BUILD_DIR)/kernel.bin
BOOT_OBJ := $(BUILD_DIR)/boot.o

# Linker script
LINKER := $(BOOT_DIR)/linker.ld

# Rust flags
# Note: -C target-feature=-fp-armv8 removed due to Rust deprecation (issue #116344)
# Rust now ties fp-armv8 to NEON and disabling is no longer supported.
RUSTFLAGS := -C link-arg=-T$(LINKER)

# ═══════════════════════════════════════════════════════════════════════════════
# BUILD RULES
# ═══════════════════════════════════════════════════════════════════════════════

.PHONY: all clean kernel boot image install run debug doc

# Default: build everything
all: image

# Create build directory
$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

# Assemble boot code
boot: $(BUILD_DIR) $(BOOT_OBJ)

$(BOOT_OBJ): $(BOOT_DIR)/boot.s
	@echo "╔═══════════════════════════════════════════════════════════════╗"
	@echo "║  Assembling Boot Code                                        ║"
	@echo "╚═══════════════════════════════════════════════════════════════╝"
	$(AS) --target=aarch64-unknown-none -c -o $@ $<

# Build kernel (Rust) - includes boot.o via linker args
kernel: $(BUILD_DIR) $(BOOT_OBJ)
	@echo "╔═══════════════════════════════════════════════════════════════╗"
	@echo "║  Building Intent Kernel (Rust)                               ║"
	@echo "╚═══════════════════════════════════════════════════════════════╝"
	cd $(KERNEL_DIR) && \
	RUSTFLAGS="-C link-arg=-T../$(LINKER) -C link-arg=../$(BOOT_OBJ)" cargo build --release --target $(TARGET)
	cp $(TARGET_DIR)/release/kernel $(KERNEL_ELF)

# Create binary image (skip separate linking, cargo already linked)
$(KERNEL_IMG): kernel
	@echo "╔═══════════════════════════════════════════════════════════════╗"
	@echo "║  Creating Bootable Image                                     ║"
	@echo "╚═══════════════════════════════════════════════════════════════╝"
	$(OBJCOPY) -O binary $(KERNEL_ELF) $@
	@echo ""
	@echo "  ✓ Kernel image created: $@"
	@echo "  ✓ Size: $$(ls -lh $@ | awk '{print $$5}')"
	@echo ""

# Main image target
image: $(KERNEL_IMG)
	@echo "╔═══════════════════════════════════════════════════════════════╗"
	@echo "║            INTENT KERNEL BUILD COMPLETE                      ║"
	@echo "╠═══════════════════════════════════════════════════════════════╣"
	@echo "║  Where humans speak and silicon listens                      ║"
	@echo "╚═══════════════════════════════════════════════════════════════╝"

# ═══════════════════════════════════════════════════════════════════════════════
# DEPLOYMENT
# ═══════════════════════════════════════════════════════════════════════════════

# SD card mount point (macOS default)
SD_MOUNT ?= /Volumes/boot

# Install to SD card
install: image
	@echo "Installing to SD card at $(SD_MOUNT)..."
	@if [ -d "$(SD_MOUNT)" ]; then \
		cp $(KERNEL_IMG) $(SD_MOUNT)/kernel8.img; \
		cp config/config.txt $(SD_MOUNT)/ 2>/dev/null || true; \
		sync; \
		echo "✓ Installed successfully!"; \
	else \
		echo "✗ SD card not found at $(SD_MOUNT)"; \
		exit 1; \
	fi

# ═══════════════════════════════════════════════════════════════════════════════
# EMULATION
# ═══════════════════════════════════════════════════════════════════════════════

# QEMU emulation (using 'virt' machine for best compatibility)
QEMU = qemu-system-aarch64
QEMU_FLAGS = -machine virt -cpu cortex-a72 -smp 4 -m 1G \
             -nographic -serial mon:stdio -semihosting \
             -kernel $(KERNEL_IMG)

run: LINKER := $(BOOT_DIR)/linker_qemu.ld
run: image
	@echo "Starting QEMU (Note: Pi 5 emulation is limited)..."
	$(QEMU) $(QEMU_FLAGS)

debug: LINKER := $(BOOT_DIR)/linker_qemu.ld
debug: image
	@echo "Starting QEMU in debug mode..."
	$(QEMU) $(QEMU_FLAGS) -s -S

# ═══════════════════════════════════════════════════════════════════════════════
# ANALYSIS
# ═══════════════════════════════════════════════════════════════════════════════

# Generate disassembly
disasm: $(KERNEL_ELF)
	@echo "Generating disassembly..."
	$(OBJDUMP) -d $< > $(BUILD_DIR)/kernel.asm
	@echo "✓ Disassembly saved to $(BUILD_DIR)/kernel.asm"

# Show binary info
info: $(KERNEL_ELF)
	@echo "╔═══════════════════════════════════════════════════════════════╗"
	@echo "║  Kernel Information                                          ║"
	@echo "╚═══════════════════════════════════════════════════════════════╝"
	@$(OBJDUMP) -h $<
	@echo ""
	@size $<

# Show symbols
symbols: $(KERNEL_ELF)
	@$(OBJDUMP) -t $< | sort -k1

# ═══════════════════════════════════════════════════════════════════════════════
# DOCUMENTATION
# ═══════════════════════════════════════════════════════════════════════════════

doc:
	@echo "Generating documentation..."
	cd $(KERNEL_DIR) && cargo doc --target $(TARGET) --document-private-items
	@echo "✓ Documentation generated in $(KERNEL_DIR)/target/$(TARGET)/doc/"

# ═══════════════════════════════════════════════════════════════════════════════
# UTILITIES
# ═══════════════════════════════════════════════════════════════════════════════

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	rm -rf $(BUILD_DIR)
	cd $(KERNEL_DIR) && cargo clean
	@echo "✓ Clean complete"

# Format all code
fmt:
	cd $(KERNEL_DIR) && cargo fmt

# Run clippy lints
lint:
	cd $(KERNEL_DIR) && cargo clippy --target $(TARGET)

# Check for issues without building
check:
	cd $(KERNEL_DIR) && cargo check --target $(TARGET)

# ═══════════════════════════════════════════════════════════════════════════════
# TESTING
# ═══════════════════════════════════════════════════════════════════════════════

.PHONY: test test-unit test-host test-integration

# Run all tests
test: test-host

# Run host-based tests (fast, native on Mac)
test-host:
	@echo "╔═══════════════════════════════════════════════════════════════╗"
	@echo "║  Running Host-Based Tests (Native)                           ║"
	@echo "╚═══════════════════════════════════════════════════════════════╝"
	cd tests/host && cargo test -- --test-threads=1
	@echo ""
	@echo "✓ All host tests passed!"

# Run unit tests (QEMU)
test-unit: $(BUILD_DIR)
	@echo "╔═══════════════════════════════════════════════════════════════╗"
	@echo "║  Running Unit Tests (QEMU)                                   ║"
	@echo "╚═══════════════════════════════════════════════════════════════╝"
	clang --target=aarch64-none-elf -c -o $(BUILD_DIR)/boot_test.o $(BOOT_DIR)/boot_test.s
	cd $(KERNEL_DIR) && \
	RUSTFLAGS="-C link-arg=-T../boot/linker_test.ld -C link-arg=../$(BUILD_DIR)/boot_test.o" cargo test --target $(TARGET) --test kernel_tests
	@echo ""
	@echo "✓ All unit tests passed!"

# Run integration tests (future)
test-integration:
	@echo "╔═══════════════════════════════════════════════════════════════╗"
	@echo "║  Running Integration Tests (QEMU)                            ║"
	@echo "╚═══════════════════════════════════════════════════════════════╝"
	cd $(KERNEL_DIR) && \
	RUSTFLAGS="-C link-arg=-Ttest_linker.ld" cargo test --target $(TARGET) --test integration_tests
	@echo ""
	@echo "✓ All integration tests passed!"

# ═══════════════════════════════════════════════════════════════════════════════
# TOOLCHAIN SETUP
# ═══════════════════════════════════════════════════════════════════════════════

# Install required Rust target
setup:
	@echo "Setting up toolchain..."
	rustup target add $(TARGET)
	rustup component add rust-src llvm-tools-preview
	@echo ""
	@echo "You also need aarch64-none-elf toolchain:"
	@echo "  macOS: brew install --cask gcc-arm-embedded"
	@echo "  Ubuntu: sudo apt install gcc-aarch64-linux-gnu"
	@echo ""

# ═══════════════════════════════════════════════════════════════════════════════
# HELP
# ═══════════════════════════════════════════════════════════════════════════════

help:
	@echo "╔═══════════════════════════════════════════════════════════════╗"
	@echo "║  INTENT KERNEL - Build System                                ║"
	@echo "╠═══════════════════════════════════════════════════════════════╣"
	@echo "║                                                               ║"
	@echo "║  Build Commands:                                              ║"
	@echo "║    make          - Build kernel image                         ║"
	@echo "║    make boot     - Assemble boot code only                    ║"
	@echo "║    make kernel   - Build Rust kernel only                     ║"
	@echo "║    make clean    - Remove build artifacts                     ║"
	@echo "║                                                               ║"
	@echo "║  Deployment:                                                  ║"
	@echo "║    make install  - Copy to SD card                            ║"
	@echo "║    make run      - Run in QEMU                                ║"
	@echo "║    make debug    - Run in QEMU with GDB server                ║"
	@echo "║                                                               ║"
	@echo "║  Analysis:                                                    ║"
	@echo "║    make disasm   - Generate disassembly                       ║"
	@echo "║    make info     - Show binary information                    ║"
	@echo "║    make symbols  - List symbols                               ║"
	@echo "║    make doc      - Generate documentation                     ║"
	@echo "║                                                               ║"
	@echo "║  Development:                                                 ║"
	@echo "║    make setup    - Install required toolchain                 ║"
	@echo "║    make fmt      - Format code                                ║"
	@echo "║    make lint     - Run clippy lints                           ║"
	@echo "║    make check    - Check without building                     ║"
	@echo "║                                                               ║"
	@echo "║  Testing:                                                     ║"
	@echo "║    make test      - Run all tests (host-based)                ║"
	@echo "║    make test-host - Run host-based tests (fast, native)       ║"
	@echo "║    make test-unit - Run unit tests (QEMU)                     ║"
	@echo "║                                                               ║"
	@echo "╚═══════════════════════════════════════════════════════════════╝"
