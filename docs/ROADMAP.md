# Project Roadmap

## ✅ Completed Milestones
- [x] **Core Kernel**: Bootloader, UART, GPIO, Mailbox.
- [x] **Memory Management**: Buddy Allocator, Heapless support.
- [x] **Intent Engine**: Semantic understanding, Vector Embeddings, Neural Memory.
- [x] **Adaptive Perception**: Hailo-8 detection, CPU fallback, Vision traits.
- [x] **Persistent Storage**: TAR RamDisk, Read-Write Overlay, Filesystem Intents.

## 🚀 Next Strategic Steps

### Phase 3: Hardware Awakening (Current Focus)
The goal is to move from simulated/stubbed hardware to real physical interaction.

- [x] **PCIe Driver (BCM2712)**: Implement the PCIe Root Complex driver to communicate with the RP1 and Hailo-8.
- [x] **Hailo-8 Driver**: Replace the stub with a real driver that sends `.hef` models and inference requests over PCIe.
- [ ] **Camera Driver**: Implement MIPI CSI-2 / ISP driver to capture real images from the Pi Camera.

### Phase 4: Connectivity & Expansion
- [ ] **Networking**: Ethernet driver (via RP1/PCIe) for remote intent processing.
- [ ] **SD Card Write**: Implement a full FAT32 driver to persist the RamDisk state back to the SD card.
- [ ] **Multi-Core**: Enable SMP to run Perception and Intent Engine on separate cores.

### Phase 5: Cognitive Evolution
- [ ] **LLM Integration**: Run a small quantized LLM (e.g., Llama-3-8B-Quantized) on the CPU/NPU hybrid.
- [ ] **Voice Interface**: Audio I/O for spoken intents.
