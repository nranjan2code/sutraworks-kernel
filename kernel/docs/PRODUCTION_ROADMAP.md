# Intent Kernel: Production Implementation Roadmap

## Executive Summary
This document provides a realistic, prioritized roadmap for completing the Intent Kernel to production quality. Total estimated effort: **6-9 months** of full-time kernel development work.

---

## 🎯 Phase 1: Core Functionality (Weeks 1-8)

### 1.1 USB/HID Input Stack ✅ 70% → 🎯 100%
**Current State**: Event ring processing works, port enumeration works, basic command submission exists.

**Remaining Work** (2-3 weeks):
1. **Control Transfer Engine** (400 LOC)
   - Complete Setup/Data/Status stage handling
   - Synchronous transfer waiting (poll event ring until completion)
   - Error handling and retry logic

2. **HID Report Parser** (300 LOC)
   - Parse HID descriptors
   - Identify Report IDs
   - Extract 23-bit steno chords from reports
   - Handle boot protocol fallback

3. **Device Enumeration** (200 LOC)
   - Get Device Descriptor (8 bytes, then full)
   - Set Configuration
   - Get HID Descriptor
   - Set Protocol (Boot vs Report)

**Deliverable**: Real steno machine input working in kernel

---

### 1.2 Filesystem Layer (Weeks 3-6)
**Current State**: Ramdisk stub only

**Required Work** (3 weeks):
1. **VFS Layer** (800 LOC)
   - File descriptor table
   - open(), read(), write(), close(), lseek()
   - Directory operations (opendir, readdir)
   - Mount point management

2. **FAT32 Driver** (1200 LOC)
   - Boot sector parsing
   - FAT table traversal
   - Directory entry parsing
   - Cluster chain following
   - File read/write

3. **Integration** (300 LOC)
   - Wire SDHCI to VFS
   - Block cache (LRU)
   - Sync/flush operations

**Deliverable**: Can read dictionary files from SD card, save session logs

---

### 1.3 Syscall Interface (Weeks 5-8)
**Current State**: 3 hardcoded syscalls in assembly

**Required Work** (2 weeks):
1. **Syscall Dispatch** (400 LOC)
   - Syscall table (vector of function pointers)
   - Argument marshaling
   - Return value handling
   - Error propagation

2. **Core Syscalls** (1000 LOC)
   ```
   File I/O:     open, read, write, close, lseek, stat, fstat
   Process:      fork, exec, exit, wait, getpid
   Memory:       mmap, munmap, brk
   IPC:          pipe, socketpair
   Network:      socket, bind, connect, listen, accept, send, recv
   ```

3. **Pointer Validation** (200 LOC)
   - VMA (Virtual Memory Area) tracking per process
   - validate_user_ptr() with VMA lookup
   - Copy-in/copy-out helpers (copy_from_user, copy_to_user)

**Deliverable**: User processes can do file I/O, spawn children, network I/O

---

## 🚀 Phase 2: Advanced Drivers (Weeks 9-16)

### 2.1 TCP/IP Stack Hardening (Weeks 9-11)
**Current State**: Basic 3-way handshake, no retransmission

**Required Work** (3 weeks):
1. **TCP Retransmission** (600 LOC)
   - Retransmission timer (RTO calculation)
   - Send buffer management
   - ACK processing
   - Duplicate ACK detection

2. **Congestion Control** (400 LOC)
   - Slow start
   - Congestion avoidance
   - Fast retransmit/fast recovery
   - CWND management

3. **Socket API** (500 LOC)
   - Non-blocking I/O
   - select()/poll() support
   - SO_REUSEADDR, SO_KEEPALIVE options
   - Proper error codes (ECONNREFUSED, ETIMEDOUT, etc.)

**Deliverable**: Robust networking that survives packet loss and congestion

---

### 2.2 SD Card Driver Completion (Weeks 10-12)
**Current State**: Read-only, no DMA

**Required Work** (2 weeks):
1. **Write Support** (300 LOC)
   - CMD24 (WRITE_SINGLE_BLOCK)
   - CMD25 (WRITE_MULTIPLE_BLOCK)
   - Write protection checking
   - Verify writes

2. **DMA Engine** (400 LOC)
   - Set up ADMA2 descriptors
   - Interrupt-driven completion
   - Error recovery (CRC errors, timeouts)

**Deliverable**: Fast, reliable SD card I/O

---

### 2.3 Hailo-8 AI Accelerator (Weeks 13-16)
**Current State**: Device enumeration only

**Required Work** (4 weeks):
1. **HCP Protocol** (500 LOC)
   - Command descriptors
   - Command/response queue
   - Firmware handshake
   - State machine

2. **DMA Engine** (400 LOC)
   - Input buffer management (image data)
   - Output buffer management (tensor data)
   - Scatter-gather descriptor setup

3. **Model Management** (300 LOC)
   - Load HEF files from filesystem
   - Send model to device (compilation)
   - Context switching

4. **Inference Pipeline** (200 LOC)
   - Image preprocessing
   - Job submission
   - Tensor retrieval
   - Integration with hailo_tensor.rs

**Deliverable**: Real-time object detection from camera

---

## 🔒 Phase 3: Security & Hardening (Weeks 17-20)

### 3.1 Memory Safety (Weeks 17-18)
1. **VMA Management** (400 LOC)
   - Per-process VMA tree
   - mmap()/munmap() implementation
   - Page fault handler integration
   - SIGSEGV delivery

2. **Capability Enforcement** (300 LOC)
   - Check capabilities before privileged operations
   - Remove Permissions::ALL shortcuts
   - Add cap_get/cap_set/cap_drop syscalls

**Deliverable**: Proper memory isolation, no kernel panics from user pointers

---

### 3.2 Error Recovery (Weeks 19-20)
1. **Driver Watchdogs** (200 LOC)
   - USB: Reset on hang
   - SD: Retry on CRC error
   - Network: Re-init on fatal error

2. **Graceful Degradation** (200 LOC)
   - CPU fallback if Hailo fails
   - Serial console if framebuffer fails
   - Continue if SD card unplugged

**Deliverable**: System stays up despite hardware failures

---

## 🧪 Phase 4: Testing & Validation (Weeks 21-24)

### 4.1 Unit Tests (Week 21)
- 500+ tests covering all subsystems
- Mock hardware for CI/CD

### 4.2 Integration Tests (Week 22)
- End-to-end scenarios
- Stress tests (memory pressure, high I/O load)

### 4.3 Hardware Validation (Weeks 23-24)
- Test on real Raspberry Pi 5
- Steno machine compatibility matrix
- Performance benchmarking

**Deliverable**: Shipping-quality OS

---

## 📊 Current Status Summary

| Component | Completeness | Blocking Issues | Est. Work |
|-----------|--------------|-----------------|-----------|
| Boot & Init | ✅ 100% | None | Done |
| Memory Allocator | ✅ 95% | Minor leaks in stack allocator | 1 day |
| Scheduler | ✅ 90% | Not tested on real multi-core | 3 days |
| VMM/Paging | 🟡 70% | No demand paging, no page fault handler | 1 week |
| Steno Engine | ✅ 100% | None | Done |
| Intent System | ✅ 100% | None | Done |
| HDC Memory | ✅ 95% | None | Done |
| USB/xHCI | 🟡 70% | No control transfers, no HID parsing | 2-3 weeks |
| Networking | 🟡 40% | No retransmission, no congestion control | 3 weeks |
| SD Card | 🟡 60% | No write, no DMA | 2 weeks |
| Hailo-8 | 🔴 10% | No HCP, no DMA, no inference | 4 weeks |
| Filesystem | 🔴 5% | No VFS, no FAT32 | 3 weeks |
| Syscalls | 🔴 10% | Only 3 syscalls, no validation | 2 weeks |
| Security | 🔴 20% | Major holes in validation | 2 weeks |

**Overall Completeness: ~55%**

---

## 🎯 Recommended Focus Order

Given your goal of a **production-grade OS**, I recommend this order:

1. **USB/HID** (immediate: makes the system actually usable)
2. **Filesystem** (week 3: needed for loading dictionaries/models)
3. **Syscalls** (week 5: needed for user processes)
4. **Security** (week 8: prevent exploits before public demo)
5. **TCP/IP** (week 9: networking is less critical for steno use case)
6. **Hailo-8** (week 13: cool but not blocking)

---

## 💬 Next Steps

I'm ready to implement any of these. Tell me which component to tackle first, and I'll build it to production quality with:
- Complete implementation (no stubs)
- Full error handling
- Inline documentation
- Unit tests
- Integration with existing code

**What's the priority?** I recommend starting with **USB/HID** so you can actually type on a steno machine.
