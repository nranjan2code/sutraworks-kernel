#!/bin/bash
# kernel/run_test.sh

# QEMU binary
QEMU=qemu-system-aarch64

# QEMU flags
# -M raspi4b: Emulate Raspberry Pi 4 (closest to Pi 5)
# -cpu cortex-a76: Match Pi 5 CPU
# -semihosting: Enable semihosting for exit
# -nographic: No GUI (implies -serial stdio)
# -kernel: The test binary
FLAGS="-M raspi4b -cpu cortex-a76 -m 2G -semihosting -nographic -kernel $1"

# Run QEMU
$QEMU $FLAGS
