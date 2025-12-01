#!/bin/bash
# kernel/run_test.sh

# QEMU binary
QEMU=qemu-system-aarch64

# Timeout in seconds
TIMEOUT=10

# Use 'virt' machine which properly supports semihosting exit
# raspi4b does NOT support semihosting properly
FLAGS="-M virt -cpu cortex-a72 -m 1G -semihosting -nographic -kernel $1"

echo "=== INTENT KERNEL UNIT TESTS ==="
echo "Timeout: ${TIMEOUT}s"
echo ""

# Run QEMU in background and capture PID
$QEMU $FLAGS &
QEMU_PID=$!

# Wait with timeout
COUNTER=0
while kill -0 $QEMU_PID 2>/dev/null; do
    sleep 1
    COUNTER=$((COUNTER + 1))
    if [ $COUNTER -ge $TIMEOUT ]; then
        echo ""
        echo "ERROR: Test timed out after ${TIMEOUT}s - killing QEMU"
        kill -9 $QEMU_PID 2>/dev/null
        exit 1
    fi
done

# Get exit status
wait $QEMU_PID
EXIT_CODE=$?

if [ $EXIT_CODE -eq 0 ] || [ $EXIT_CODE -eq 33 ]; then
    # 33 = 0x21 = (0x10 << 1) | 1 which is QEMU success for semihosting
    echo ""
    echo "=== TESTS COMPLETED ==="
    exit 0
else
    echo ""
    echo "ERROR: QEMU exited with code $EXIT_CODE"
    exit $EXIT_CODE
fi
