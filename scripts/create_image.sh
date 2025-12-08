#!/bin/sh

SD_IMG="sd.img"
ROOT_DIR="build/sd-root"
INIT_BIN="user/init/target/aarch64-unknown-none/release/init"
COUNTER_BIN="user/services/counter/target/aarch64-unknown-none/release/counter"
HELLO_BIN="user/services/hello/target/aarch64-unknown-none/release/hello"

echo "[IMAGE] Creating SD Card Image..."

# 1. Prepare Root Directory
rm -rf "$ROOT_DIR"
mkdir -p "$ROOT_DIR"

if [ -f "$INIT_BIN" ]; then
    cp "$INIT_BIN" "$ROOT_DIR/init"
    echo "  Added: init.elf (as init)"
else
    echo "  Warning: init binary not found at $INIT_BIN"
fi

if [ -f "$COUNTER_BIN" ]; then
    cp "$COUNTER_BIN" "$ROOT_DIR/counter"
    echo "  Added: counter.elf"
else
    echo "  Warning: counter binary not found"
fi

if [ -f "$HELLO_BIN" ]; then
    cp "$HELLO_BIN" "$ROOT_DIR/hello"
    echo "  Added: hello.elf"
else
    echo "  Warning: hello binary not found"
fi

# 1.1 Copy Intent Apps
APPS_DIR="user/apps"
if [ -d "$APPS_DIR" ]; then
    mkdir -p "$ROOT_DIR/apps"
    cp "$APPS_DIR"/*.intent "$ROOT_DIR/apps/" 2>/dev/null
    echo "  Added: Intent Apps from $APPS_DIR"
else
    echo "  Warning: Apps directory not found at $APPS_DIR"
fi

# 2. Create Image (Try mtools first)
if command -v mformat >/dev/null 2>&1; then
    echo "  Using mtools..."
    dd if=/dev/zero of="$SD_IMG" bs=1M count=64 status=none
    mformat -i "$SD_IMG" -F ::
    # mcopy -s "$ROOT_DIR/"* ::
    # Iterate files
    for f in "$ROOT_DIR"/*; do
        b=$(basename "$f")
        mcopy -i "$SD_IMG" "$f" "::$b"
    done
    echo "  Success: Created $SD_IMG using mtools"
    exit 0
fi

# 3. Fallback: macOS hdiutil
# 3. Fallback: macOS newfs_msdos + hdiutil
if command -v newfs_msdos >/dev/null 2>&1; then
    echo "  Using newfs_msdos..."
    # Create 64MB blank image
    rm -f "$SD_IMG"
    dd if=/dev/zero of="$SD_IMG" bs=1m count=64 2>/dev/null
    
    # Attach as raw device
    # Output format: /dev/diskX   GUID_partition_scheme
    # We strip whitespace
    DEV=$(hdiutil attach -nomount "$SD_IMG" | awk '{print $1}' | tr -d '[:space:]')
    
    if [ -z "$DEV" ]; then
        echo "Error: Failed to attach image"
        exit 1
    fi
    
    echo "  Attached to $DEV"
    
    # Format
    newfs_msdos -F 32 -v BOOT "$DEV" >/dev/null
    hdiutil detach "$DEV" >/dev/null
    
    # Mount now (it has FS)
    mkdir -p build/mnt
    hdiutil attach -mountpoint build/mnt "$SD_IMG" >/dev/null
    
    # Copy files
    cp -r "$ROOT_DIR"/* build/mnt/
    
    # Detach
    hdiutil detach build/mnt >/dev/null
    rmdir build/mnt
    
    echo "  Success: Created $SD_IMG using newfs_msdos"
    exit 0
fi

# 4. Last Resort: hdiutil (Legacy)
if command -v hdiutil >/dev/null 2>&1; then

echo "Error: Neither mtools nor hdiutil found. Cannot create image."
exit 1
